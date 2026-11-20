//! TODO/machine.md T-2008: an init of the image that is booted before its
//! command is known.
//!
//! A guest of the machine tier whose state is saved boots the init of the
//! image once (`guest-init --hold FILE`), and each run from that state
//! gives its command later (`guest-init --attach FILE -- CMD`). Two
//! processes in place of the one supervisor of [`super::boot`]:
//!
//! * `--hold` starts the supervisor as a process of its own, waits until
//!   the boot of the init is complete or failed, and ends. The supervisor
//!   stays as the parent of the init, with the console of the init, and
//!   writes the state to FILE: the pid of the init, or the line of the
//!   failure;
//! * `--attach` reads FILE, joins the namespaces of the init, runs the
//!   command, and asks the init for its stop. A failed boot gives its line
//!   and 125 here, as a run with no saved state does.

use std::path::Path;
use std::time::{Duration, Instant};

use super::boot::{self, Spec};
use super::join;

/// The state of a held init, as FILE holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Held {
    /// The init is up: its pid in the PID namespace of the supervisor,
    /// whether it is systemd, and whether it has a cgroup namespace.
    Up {
        pid: i32,
        systemd: bool,
        cgroup: bool,
    },
    /// The boot failed, with the line of the failure.
    Failed(String),
}

impl Held {
    pub fn text(&self) -> String {
        match self {
            Held::Up {
                pid,
                systemd,
                cgroup,
            } => format!("up {pid} {} {}\n", u8::from(*systemd), u8::from(*cgroup)),
            Held::Failed(why) => format!("failed\n{why}\n"),
        }
    }

    /// Read the text of a state file. An error is what is wrong with it:
    /// no command runs from a state that podvm cannot read.
    pub fn parse(text: &str) -> Result<Held, String> {
        let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
        let f: Vec<&str> = first.split(' ').collect();
        let flag = |s: &str| match s {
            "0" => Some(false),
            "1" => Some(true),
            _ => None,
        };
        match f.as_slice() {
            ["up", pid, systemd, cgroup] => {
                let pid = pid.parse::<i32>().ok().filter(|p| *p > 1);
                match (pid, flag(systemd), flag(cgroup)) {
                    (Some(pid), Some(systemd), Some(cgroup)) => Ok(Held::Up {
                        pid,
                        systemd,
                        cgroup,
                    }),
                    _ => Err(format!("the line {first:?} names no init")),
                }
            }
            ["failed"] => Ok(Held::Failed(rest.trim_end().to_string())),
            _ => Err(format!("the line {first:?} is no state of an init")),
        }
    }

    pub fn read(file: &Path) -> Result<Held, String> {
        let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
        Held::parse(&text)
    }
}

/// Write the state under a name of its own, then give it the name `file`:
/// a reader finds the whole text or no file.
fn write(file: &Path, held: &Held) -> std::io::Result<()> {
    let part = file.with_extension("part");
    std::fs::write(&part, held.text())?;
    std::fs::rename(&part, file)
}

/// `guest-init --hold FILE`: 0 where the init is up, 1 where its boot
/// failed (FILE then has the line), 125 where no supervisor started.
pub fn hold(spec: &Spec, file: &Path) -> i32 {
    let _ = std::fs::remove_file(file);
    // The supervisor is a process of its own: this one ends after the
    // boot, and the init must keep its parent.
    match boot::fork_here() {
        n if n < 0 => boot::refuse(&format!(
            "the supervisor of the held init: fork: {}",
            std::io::Error::last_os_error()
        )),
        0 => {
            let (held, init) = match boot::start(spec) {
                Ok(b) => {
                    let held = Held::Up {
                        pid: b.init,
                        systemd: b.systemd,
                        cgroup: b.cgroup,
                    };
                    (held, Some(b))
                }
                Err(why) => (Held::Failed(why), None),
            };
            if let Err(e) = write(file, &held) {
                eprintln!(
                    "podvm: --podvm-init=image: the state file {}: {e} (TODO/machine.md T-2008)",
                    file.display()
                );
                boot::exit_now(125);
            }
            match init {
                // The supervisor stays: the init ends when its parent does.
                Some(b) => {
                    boot::wait_end(b.init);
                    boot::exit_now(0)
                }
                None => boot::exit_now(1),
            }
        }
        supervisor => wait_state(file, supervisor),
    }
}

/// Wait for the state file of the supervisor `pid`. The file comes when
/// the boot is complete or failed; the bound is the one of the boot, and a
/// little more for the write. A supervisor that ended with no file is a
/// failure at once.
fn wait_state(file: &Path, pid: i32) -> i32 {
    let end = Instant::now() + crate::incus::systemd::BOOT_BOUND + Duration::from_secs(30);
    let mut gone = false;
    loop {
        match Held::read(file) {
            Ok(Held::Up { .. }) => return 0,
            Ok(Held::Failed(_)) => return 1,
            Err(e) if gone || Instant::now() >= end => {
                return boot::refuse(&format!("the held init gave no state: {e}"))
            }
            // One more read after the end of the supervisor: its write
            // comes before its end.
            Err(_) => gone = boot::ended(pid).is_some(),
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// `guest-init --attach FILE -- CMD`: the exit code of the command in the
/// held init, or 125 with one line.
pub fn attach(spec: &Spec, file: &Path) -> i32 {
    let (pid, systemd, cgroup) = match Held::read(file) {
        Ok(Held::Up {
            pid,
            systemd,
            cgroup,
        }) => (pid, systemd, cgroup),
        Ok(Held::Failed(why)) => return boot::refuse(&why),
        Err(e) => return boot::refuse(&format!("the state of the held init: {e}")),
    };
    // The PID namespace of the init for each child of this process, then
    // the other namespaces in the child ([`join`]).
    if let Err(e) = join::enter_pid_namespace(pid) {
        return boot::refuse(&format!("the PID namespace of the held init {pid}: {e}"));
    }
    let ns = match join::Namespaces::of(pid, cgroup) {
        Ok(ns) => ns,
        Err(e) => return boot::refuse(&format!("the namespaces of the held init {pid}: {e}")),
    };
    let code = join::run_command(&ns, spec);
    stop(pid, systemd, spec.discard);
    code
}

/// Ask the held init for its stop and wait for its end, within the bound
/// of [`boot`]; its supervisor reaps it. With `discard` nothing of the run
/// stays, and the guest ends at once.
fn stop(pid: i32, systemd: bool, discard: bool) {
    if discard || !boot::signal(pid, boot::stop_signal(systemd)) {
        return;
    }
    let end = Instant::now() + boot::BOUND_OF_STOP;
    while Instant::now() < end {
        if !boot::signal(pid, 0) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    boot::signal(pid, boot::KILL);
}

#[cfg(test)]
mod tests {
    use super::Held;

    /// The state file round-trips, and each text that is no state is an
    /// error: no command runs from it.
    #[test]
    fn the_state_of_a_held_init_is_read_or_refused() {
        for held in [
            Held::Up {
                pid: 42,
                systemd: true,
                cgroup: false,
            },
            Held::Up {
                pid: 7,
                systemd: false,
                cgroup: true,
            },
            Held::Failed("the init of the image ended during its boot\nline 2".to_string()),
            Held::Failed(String::new()),
        ] {
            assert_eq!(Held::parse(&held.text()), Ok(held.clone()), "{held:?}");
        }
        for bad in [
            "",
            "up",
            "up 42 1",
            "up 0 1 1\n",
            "up 1 1 1\n",
            "up -3 1 1\n",
            "up 42 2 1\n",
            "up 42 1 yes\n",
            "up x 1 1\n",
            "up 99999999999999 1 1\n",
            "down 42 1 1\n",
            "failed now\n",
            "\u{0}\u{0}",
        ] {
            assert!(Held::parse(bad).is_err(), "{bad:?} was read as a state");
        }
    }
}
