//! `run --podvm-tier=machine --podvm-init=image` with the init of the
//! image (systemd) as PID 1 of the guest itself (TODO/machine.md T-1994,
//! steps 2 and 3; PodUtils/podrunner#313 ask 2, #316 ask 2).
//!
//! On the root disk of the image, `podvm guest-root --init=SECS` makes the
//! root, writes the unit of the run ([`UNIT`]) and its masks into it, and
//! execs the image's `/sbin/init`. systemd starts the unit after
//! `basic.target`. The unit runs `podvm guest-ready`, which waits for
//! `systemctl is-system-running` to give `running` or `degraded` and then
//! execs the init script of the run (`/podvm-init`): the command runs in
//! the unit, with the markers, the ports and the exit code of each other
//! guest (step 3: the exit marker on the serial line).
//!
//! One bound, from the exec of the init ([`crate::incus::systemd::boot_bound`],
//! `PODVM_INIT_BOOT_TIMEOUT`), holds three faults, each with one line and
//! exit 125 before the command:
//!
//! * the unit of the run does not start: a watchdog that `guest-root`
//!   forks before the exec of the init ends the run at the bound;
//! * the init does not get ready: `guest-ready` ends the run at the bound,
//!   with the last state of systemd;
//! * the init is in a state that does not come back (`maintenance`,
//!   `stopping`): `guest-ready` ends the run at once.
//!
//! An init that ends is a panic of the kernel: the guest resets, and the
//! host names a guest that ended before its exit code.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::incus::systemd::{step, Step};

/// The verb that the unit of the run starts.
pub const READY_VERB: &str = "guest-ready";
/// The unit of the run.
pub const UNIT: &str = "podvm-run.service";
/// The file that `guest-ready` makes at its start, for the watchdog.
const STARTED: &str = "/podvm/started";
/// The bound of the boot in the guest: the uptime of its end, and the
/// seconds of the bound.
const DEADLINE: &str = "/podvm/init-deadline";
/// The units of a console that the guest keeps off: a login prompt on the
/// serial line of the markers, and the questions of a first boot.
pub const MASKS: [&str; 6] = [
    "serial-getty@ttyS0.service",
    "serial-getty@ttyS1.service",
    "serial-getty@hvc0.service",
    "console-getty.service",
    "getty@tty1.service",
    "systemd-firstboot.service",
];
const ETC: &str = "/etc/systemd/system";
/// The flag of a guest whose state podvm saves (TODO/machine.md T-2008):
/// a fault before the job gives the marker of a job that the guest did
/// not take ([`super::job::BAD`]), so that the host boots a guest with no
/// state, in place of the end of the run.
pub const HELD_FLAG: &str = "--held";

/// The unit of the run: after `basic.target` (the default dependencies),
/// wanted by `multi-user.target`, with the serial line of the markers as
/// its output. `held`: the guest of a saved state ([`HELD_FLAG`]).
pub fn unit(nonce: &str, held: bool) -> String {
    let flag = if held {
        format!(" {HELD_FLAG}")
    } else {
        String::new()
    };
    format!(
        "[Unit]\nDescription=The command of the run of podvm (TODO/machine.md T-1994)\n\n\
         [Service]\nType=exec\nExecStart=/podvm/podvm {READY_VERB} {nonce} /podvm-init{flag}\n\
         StandardInput=null\nStandardOutput=tty\nStandardError=inherit\nTTYPath=/dev/ttyS0\n\
         TTYReset=no\nTTYVHangup=no\nTimeoutStartSec=infinity\nRestart=no\n"
    )
}

/// One file of the root that the init reads.
#[derive(Debug, PartialEq, Eq)]
pub enum Entry {
    File(String, String),
    Link(String, String),
}

/// Each file that `guest-root` writes before the exec of the init: the
/// unit, its want, the masks, and the status of the boot off the console.
pub fn entries(nonce: &str, held: bool) -> Vec<Entry> {
    let mut v = vec![
        Entry::File(format!("{ETC}/{UNIT}"), unit(nonce, held)),
        Entry::Link(
            format!("{ETC}/multi-user.target.wants/{UNIT}"),
            format!("../{UNIT}"),
        ),
        Entry::File(
            "/etc/systemd/system.conf.d/podvm.conf".to_string(),
            "[Manager]\nShowStatus=no\n".to_string(),
        ),
    ];
    v.extend(
        MASKS
            .iter()
            .map(|m| Entry::Link(format!("{ETC}/{m}"), "/dev/null".to_string())),
    );
    v
}

/// The seconds since the boot (`/proc/uptime`).
fn uptime() -> Result<f64, String> {
    let t = std::fs::read_to_string("/proc/uptime").map_err(|e| format!("/proc/uptime: {e}"))?;
    t.split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("/proc/uptime: {t:?}"))
}

/// The text of the deadline file: the uptime of the end, and the bound.
fn deadline_text(deadline: f64, bound: u64) -> String {
    format!("{deadline} {bound}\n")
}

fn read_deadline(text: &str) -> Option<(f64, u64)> {
    let mut w = text.split_whitespace();
    Some((w.next()?.parse().ok()?, w.next()?.parse().ok()?))
}

unsafe extern "C" {
    fn fork() -> i32;
    fn _exit(code: i32) -> !;
}

/// In `guest-root`, after the root moved: write [`entries`] and the
/// deadline of the boot, and fork the watchdog of the unit.
pub fn prepare(root: &Path, nonce: &str, bound: u64, held: bool) -> Result<(), String> {
    for e in entries(nonce, held) {
        let (path, made) = match &e {
            Entry::File(p, text) => (p, write_at(root, p, |at| std::fs::write(at, text))),
            Entry::Link(p, to) => (
                p,
                write_at(root, p, |at| {
                    let _ = std::fs::remove_file(at);
                    std::os::unix::fs::symlink(to, at)
                }),
            ),
        };
        made.map_err(|err| format!("{path}: {err}"))?;
    }
    let deadline = uptime()? + bound as f64;
    std::fs::write(
        root.join(DEADLINE.trim_start_matches('/')),
        deadline_text(deadline, bound),
    )
    .map_err(|e| format!("{DEADLINE}: {e}"))?;
    // SAFETY: fork(2) of a process with one thread; the child only reads
    // files, writes to the console and ends.
    match unsafe { fork() } {
        -1 => Err(format!(
            "fork of the watchdog: {}",
            std::io::Error::last_os_error()
        )),
        0 => watchdog(nonce, deadline, bound, held),
        _ => Ok(()),
    }
}

fn write_at(
    root: &Path,
    p: &str,
    f: impl FnOnce(&Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    let at = root.join(p.trim_start_matches('/'));
    if let Some(d) = at.parent() {
        std::fs::create_dir_all(d)?;
    }
    f(&at)
}

/// The child of `guest-root`: the end of the run where the unit did not
/// start by the deadline.
fn watchdog(nonce: &str, deadline: f64, bound: u64, held: bool) -> ! {
    loop {
        if Path::new(STARTED).exists() {
            // SAFETY: the end of a forked child.
            unsafe { _exit(0) };
        }
        match uptime() {
            Ok(now) if now < deadline => std::thread::sleep(Duration::from_millis(500)),
            _ => fail(
                nonce,
                held,
                &format!(
                    "the init of the image did not start the unit {UNIT} of the run in {bound} s"
                ),
            ),
        }
    }
}

/// The end of a guest before its command: for a guest of a saved state
/// (`held`) first the marker of a job that it did not take, which the
/// host reads as a state that does not serve; then the markers of a run
/// that ends with 125 and the reset.
fn fail(nonce: &str, held: bool, why: &str) -> ! {
    if held {
        bad(nonce, why);
    }
    super::disk::end_run(nonce, &line(why))
}

/// The marker of a job that the guest of a state did not take, with
/// `why`: the host then boots a guest with no state.
pub(crate) fn bad(nonce: &str, why: &str) {
    use std::io::Write;
    let mut o = std::io::stdout();
    let why = why.replace('\n', " ");
    o.write_all(format!("\n{}{nonce} {why}\n", super::job::BAD).as_bytes())
        .and_then(|()| o.flush())
        .unwrap_or(());
}

/// The line of a run that ends before its command.
fn line(why: &str) -> String {
    format!("podvm: machine tier: --podvm-init=image: {why} (TODO/machine.md T-1994)\n")
}

/// One `systemctl is-system-running` (with `--wait` where `wait`), killed
/// at `end` (an uptime): its first line and its error output, or `None`.
fn ask(wait: bool, end: f64) -> Result<Option<(String, String)>, String> {
    let mut c = Command::new("systemctl");
    c.arg("is-system-running");
    if wait {
        c.arg("--wait");
    }
    let mut child = c
        .env("PATH", crate::initns::join::PATH)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("systemctl is-system-running: {e}"))?;
    loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            let o = child.wait_with_output().map_err(|e| e.to_string())?;
            let out = String::from_utf8_lossy(&o.stdout);
            return Ok(Some((
                out.lines().next().unwrap_or("").trim().to_string(),
                String::from_utf8_lossy(&o.stderr).trim().to_string(),
            )));
        }
        if uptime()? >= end {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Wait until systemd is up, by the deadline: `Err` is the reason.
fn wait_ready() -> Result<(), String> {
    let text = std::fs::read_to_string(DEADLINE).map_err(|e| format!("{DEADLINE}: {e}"))?;
    let (end, bound) = read_deadline(&text).ok_or_else(|| format!("{DEADLINE}: {text:?}"))?;
    let (mut wait, mut last) = (true, String::from("no answer"));
    loop {
        if let Some((state, err)) = ask(wait, end)? {
            last = format!("systemctl is-system-running gives {state:?}: {err}");
            if wait && crate::initns::join::no_wait_flag(&err) {
                wait = false;
                continue;
            }
            match step(&state, &err) {
                Step::Run => return Ok(()),
                Step::Again => {}
                Step::Fail => return Err(format!("the init of the image is not up: {last}")),
            }
        }
        if uptime()? >= end {
            // `--wait` gave no answer by the bound: the state now.
            if let Ok(Some((state, err))) = ask(false, end + 5.0) {
                last = format!("systemctl is-system-running gives {state:?}: {err}");
            }
            return Err(format!(
                "the init of the image did not finish its boot in {bound} s \
                 (PODVM_INIT_BOOT_TIMEOUT): {last}"
            ));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// `podvm guest-ready NONCE SCRIPT`, the unit of the run: it does not
/// return where the exec of the script works.
pub fn ready_main(args: &[String]) -> i32 {
    use std::os::unix::process::CommandExt;
    let (nonce, script, held) = match args {
        [n, s] => (n, s, false),
        [n, s, f] if f == HELD_FLAG => (n, s, true),
        _ => {
            eprintln!("podvm {READY_VERB}: the arguments are NONCE SCRIPT [{HELD_FLAG}]");
            return 125;
        }
    };
    if let Err(e) = std::fs::write(STARTED, b"") {
        // The watchdog ends the run at the bound; say why first.
        eprintln!("podvm {READY_VERB}: {STARTED}: {e}");
    }
    let why = match wait_ready().and_then(|()| console_in()) {
        Ok(console) => format!(
            "the init script of the run: {}",
            Command::new("/bin/sh").arg(script).stdin(console).exec()
        ),
        Err(e) => e,
    };
    fail(nonce, held, &why)
}

/// The serial line of the markers as the stdin of the init script, as the
/// kernel gives it to the script of a guest with no init of the image:
/// `guest-signals` (T-2041) reads the signals of the host there, and
/// `guest-job` the job of a run (T-2008). The unit itself has no stdin.
pub fn console_in() -> Result<std::fs::File, String> {
    std::fs::File::open(CONSOLE).map_err(|e| format!("{CONSOLE}: {e}"))
}

/// The first serial port: the markers, the job and the signals.
pub const CONSOLE: &str = "/dev/ttyS0";

/// Host side: the path in `rootfs` of the file that the image path `p`
/// names, with each link of each component resolved inside the image (an
/// absolute target is a path of the image); `None` where a component is
/// missing or the links loop.
pub fn resolve(rootfs: &Path, p: &str) -> Option<PathBuf> {
    let mut done = String::new();
    let mut todo: Vec<String> = p.split('/').rev().map(String::from).collect();
    let mut links = 0;
    while let Some(part) = todo.pop() {
        match part.as_str() {
            "" | "." => continue,
            ".." => {
                done = crate::incus::walk::normal(&format!("{done}/.."));
                if done == "/" {
                    done.clear();
                }
                continue;
            }
            _ => {}
        }
        let next = format!("{done}/{part}");
        let md = std::fs::symlink_metadata(rootfs.join(next.trim_start_matches('/'))).ok()?;
        if md.file_type().is_symlink() {
            links += 1;
            if links > crate::incus::walk::MAX_LINKS {
                return None;
            }
            let t = std::fs::read_link(rootfs.join(next.trim_start_matches('/'))).ok()?;
            let t = t.to_string_lossy().into_owned();
            if t.starts_with('/') {
                done.clear();
            }
            todo.extend(t.split('/').rev().map(String::from));
        } else {
            done = next;
        }
    }
    Some(rootfs.join(done.trim_start_matches('/')))
}

/// Host side: the bound of the boot in seconds where the run boots
/// systemd as PID 1 of the guest: an init of the image (`init_image`), a
/// run that takes this route (`route`: the root disk, where the saved
/// state of T-2008 does not serve it; call C27) and an image whose init is
/// systemd. A bad bound is an error for each run of an init of the image.
pub fn of(rootfs: &Path, init_image: bool, route: bool) -> Result<Option<u64>, String> {
    if !init_image {
        return Ok(None);
    }
    let bound = crate::incus::systemd::boot_bound()?.as_secs();
    Ok((route && systemd_init(rootfs)).then_some(bound))
}

/// Host side: whether the init of the image (`/sbin/init`) is systemd.
pub fn systemd_init(rootfs: &Path) -> bool {
    resolve(rootfs, crate::initns::INIT)
        .is_some_and(|p| p.file_name().is_some_and(|n| n == "systemd") && p.is_file())
}

/// Host side: the environment of the command without the variables of
/// podvm's own entry, as the init of the image of T-2002 gives it.
pub fn image_env(env: &[String]) -> Vec<String> {
    env.iter()
        .filter(|e| {
            let name = e.split('=').next().unwrap_or("");
            !crate::initns::join::internal(name)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
