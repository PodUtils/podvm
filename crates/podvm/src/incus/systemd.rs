//! PodUtils/podrunner#313 ask 1 (TODO/machine.md T-1993): the init of the
//! image (systemd) as PID 1 of an incus instance that stays.
//!
//! With `--podvm-init=image` the instance has no `lxc.init.cmd`, so the
//! init of the image boots. podvm then waits for `systemctl
//! is-system-running --wait` by the exec API, again until PID 1 has its
//! bus, within [`BOOT_BOUND`], and
//! starts `guest-hold --run --detach` by the exec API as one more process:
//! it runs the command of `/.podvm/cmd` with its log and its exit code as
//! with podvm's own init, relays the channel, and asks PID 1 to power the
//! instance off when the command ends.

use std::time::{Duration, Instant};

use pod_incus::{Client, Exec};

/// The bound of the boot of the init of the image, where
/// [`BOOT_BOUND_VAR`] sets no other.
pub const BOOT_BOUND: Duration = Duration::from_secs(180);

/// PodUtils/podrunner#349 ask 1 (TODO/machine.md T-2083): the variable
/// that sets the bound of the boot, in seconds.
pub const BOOT_BOUND_VAR: &str = "PODVM_INIT_BOOT_TIMEOUT";

/// The bound of the boot: [`BOOT_BOUND_VAR`], else [`BOOT_BOUND`]. A value
/// that is not a number of seconds from 1 to 3600 is an error, not the
/// default.
pub fn boot_bound() -> Result<Duration, String> {
    bound_of(std::env::var_os(BOOT_BOUND_VAR).as_deref())
}

/// [`boot_bound`] for the value `v` of the variable: the rule
/// `BOOT_TIMEOUT` of [`crate::number`] (TODO/gate.md T-1947), after the
/// white space at each end of the text.
pub(crate) fn bound_of(v: Option<&std::ffi::OsStr>) -> Result<Duration, String> {
    let Some(v) = v else {
        return Ok(BOOT_BOUND);
    };
    let text = v.to_string_lossy();
    crate::number::check(&crate::number::BOOT_TIMEOUT, text.trim()).map(Duration::from_secs)
}

/// What podvm does after one `systemctl is-system-running --wait`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// The init is up: start the command.
    Run,
    /// The init is not up yet: ask again. Before PID 1 opens its bus,
    /// systemctl gives no state and "Failed to connect to bus" (experiment
    /// 434 clause 19), and `--wait` does not wait for the bus.
    Again,
    /// The init stays down: the error.
    Fail,
}

/// The step for one answer of `systemctl is-system-running`; the boot of
/// an init in a PID namespace (TODO/enter.md T-2002) and in a gVisor
/// sandbox (T-2007) read it too.
pub(crate) fn step(state: &str, stderr: &str) -> Step {
    match state {
        "running" | "degraded" => Step::Run,
        // PodUtils/podrunner#349 (T-2083): "offline" before PID 1 made
        // /run/systemd/system or opened its bus, and "unknown" where
        // systemctl cannot read the state yet. `--wait` waits only in
        // "initializing" and "starting", so these come back at once.
        "initializing" | "starting" | "offline" | "unknown" => Step::Again,
        // systemd 262 says "Failed to connect to system scope bus via local
        // transport" (TODO/enter.md T-2002); older ones "... to bus".
        "" if stderr.contains("Failed to connect to") => Step::Again,
        _ => Step::Fail,
    }
}

/// One answer to the poll of the state of systemd.
#[derive(Debug)]
pub(crate) enum Answer {
    /// systemctl ran: its first line, its error output and its exit code.
    Out {
        state: String,
        err: String,
        exit: i32,
    },
    /// The exec API refused; `running` is false where the instance is not
    /// in the state Running (or podvm cannot read its state).
    ExecErr { err: String, running: bool },
    /// No answer before the bound.
    NoReply,
}

impl Answer {
    /// The answer in one short text, for the record of the polls.
    fn text(&self) -> String {
        match self {
            Answer::Out { state, err, exit } => {
                let first = err.lines().next().unwrap_or("");
                if first.is_empty() {
                    format!("{state:?} (exit {exit})")
                } else {
                    format!("{state:?} (exit {exit}: {first})")
                }
            }
            Answer::ExecErr { err, running: true } => format!("exec refused ({err})"),
            Answer::ExecErr {
                err,
                running: false,
            } => {
                format!("exec refused ({err}), the instance is not running")
            }
            Answer::NoReply => "no answer".to_string(),
        }
    }
}

/// The decision after one poll.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Next {
    /// systemd is up; `degraded` is true for the state "degraded".
    Up { degraded: bool },
    /// Poll again.
    Again,
    /// A final state: the line. The instance goes.
    Fail(String),
    /// The bound passed with no final state: the line. The instance stays
    /// for a look (PodUtils/podrunner#349 ask 3).
    Bound(String),
}

/// The record of the polls of one boot: each run of equal answers once,
/// with its count.
#[derive(Debug, Default)]
pub(crate) struct Polls(Vec<(String, u32)>);

/// The most runs of equal answers that [`Polls::line`] shows: the first
/// and the last half.
const POLLS_SHOWN: usize = 12;

impl Polls {
    fn push(&mut self, text: String) {
        match self.0.last_mut() {
            Some((last, n)) if *last == text => *n += 1,
            _ => self.0.push((text, 1)),
        }
    }

    /// The polls in order, as `"offline" (exit 1) x5, "starting" (exit 1)`.
    pub(crate) fn line(&self) -> String {
        let one = |(t, n): &(String, u32)| {
            if *n == 1 {
                t.clone()
            } else {
                format!("{t} x{n}")
            }
        };
        let all = &self.0;
        if all.len() <= POLLS_SHOWN {
            return all.iter().map(one).collect::<Vec<_>>().join(", ");
        }
        let half = POLLS_SHOWN / 2;
        let head: Vec<String> = all[..half].iter().map(one).collect();
        let tail: Vec<String> = all[all.len() - half..].iter().map(one).collect();
        format!(
            "{}, ({} more), {}",
            head.join(", "),
            all.len() - POLLS_SHOWN,
            tail.join(", ")
        )
    }
}

/// The wait for the init of one instance: the end and the polls.
#[derive(Debug)]
pub(crate) struct Waiter {
    end: Instant,
    bound: Duration,
    polls: Polls,
}

impl Waiter {
    pub(crate) fn new(start: Instant, bound: Duration) -> Waiter {
        Waiter {
            end: start + bound,
            bound,
            polls: Polls::default(),
        }
    }

    /// The time that is left at `now`.
    pub(crate) fn left(&self, now: Instant) -> Duration {
        self.end.saturating_duration_since(now)
    }

    /// The decision for the answer `a` at `now`. "offline", "initializing",
    /// "starting", "unknown", the bus and a refused exec of a running
    /// instance are not final before the bound.
    pub(crate) fn next(&mut self, a: Answer, now: Instant) -> Next {
        let text = a.text();
        self.polls.push(text.clone());
        let again = match &a {
            Answer::Out { state, err, .. } => match step(state, err) {
                Step::Run => {
                    return Next::Up {
                        degraded: state == "degraded",
                    }
                }
                Step::Fail => {
                    return Next::Fail(format!(
                        "systemctl is-system-running gives {text}; the polls: {}",
                        self.polls.line()
                    ))
                }
                Step::Again => true,
            },
            Answer::ExecErr { running: false, .. } => {
                return Next::Fail(format!(
                    "systemctl is-system-running: {text}; the polls: {}",
                    self.polls.line()
                ))
            }
            Answer::ExecErr { running: true, .. } | Answer::NoReply => true,
        };
        if again && now >= self.end {
            return Next::Bound(format!(
                "the init of the image did not finish its boot in {} s; the polls: {}",
                self.bound.as_secs(),
                self.polls.line()
            ));
        }
        Next::Again
    }
}

/// Why the init of the image is not up.
#[derive(Debug)]
pub struct Down {
    /// The line, with the polls.
    pub line: String,
    /// True where the instance stays for a look: the bound passed with no
    /// final state (PodUtils/podrunner#349 ask 3).
    pub keep: bool,
}

impl Down {
    fn gone(line: String) -> Down {
        Down { line, keep: false }
    }
}

/// The time between two polls.
const POLL_GAP: Duration = Duration::from_millis(500);

/// One poll of `systemctl is-system-running --wait`, at most `left` long.
async fn poll(c: &Client, name: &str, left: Duration) -> Answer {
    let wait = Exec::new(["systemctl", "is-system-running", "--wait"]);
    match tokio::time::timeout(left, c.exec_recorded(name, &wait)).await {
        Err(_) => Answer::NoReply,
        Ok(Ok(out)) => Answer::Out {
            state: String::from_utf8_lossy(&out.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string(),
            err: String::from_utf8_lossy(&out.stderr).trim().to_string(),
            exit: out.exit_code,
        },
        Ok(Err(e)) => {
            // Before the instance is ready, incusd can refuse the exec; a
            // refusal of an instance that is not running is final.
            let running = matches!(c.instance_state(name).await, Ok(s) if s.status == "Running");
            Answer::ExecErr {
                err: e.to_string(),
                running,
            }
        }
    }
}

/// Wait for the init of the image, then start the command. An error names
/// what happened: the state of systemd with each poll, or the bound, the
/// polls and the name of PID 1.
pub async fn boot_then_run(c: &Client, name: &str) -> Result<(), Down> {
    let bound = boot_bound().map_err(|e| Down::gone(format!("--podvm-init=image: {e}")))?;
    let mut w = Waiter::new(Instant::now(), bound);
    loop {
        let left = w.left(Instant::now());
        let a = poll(c, name, left).await;
        match w.next(a, Instant::now()) {
            Next::Up { degraded } => {
                if degraded {
                    // PodUtils/podrunner#349 ask 2: degraded is up, with
                    // the failed units in one line.
                    let units = failed_units(c, name).await;
                    eprintln!(
                        "podvm: --podvm-init=image: systemd of {name} is degraded{}",
                        if units.is_empty() {
                            "; systemctl names no failed unit"
                        } else {
                            &units
                        }
                    );
                }
                break;
            }
            Next::Again => tokio::time::sleep(POLL_GAP.min(w.left(Instant::now()))).await,
            Next::Fail(line) => {
                return Err(Down::gone(format!(
                    "--podvm-init=image: the init of the image is not up: {line}{}",
                    failed_units(c, name).await
                )))
            }
            Next::Bound(line) => {
                return Err(Down {
                    line: format!(
                        "--podvm-init=image: {line}; PID 1 is {}; an image with no systemd has \
                         no such init{}",
                        pid1(c, name).await,
                        failed_units(c, name).await
                    ),
                    keep: true,
                })
            }
        }
    }
    let run = Exec::new(["/.podvm/podvm", "guest-hold", "--run", "--detach"]);
    let r = c
        .exec_recorded(name, &run)
        .await
        .map_err(|e| Down::gone(format!("--podvm-init=image: the start of the command: {e}")))?;
    if r.exit_code != 0 {
        return Err(Down::gone(format!(
            "--podvm-init=image: the start of the command gave exit {}: {}",
            r.exit_code,
            String::from_utf8_lossy(&r.stderr).trim()
        )));
    }
    Ok(())
}

/// PodUtils/podrunner#325 ask 2: the failed units of systemd for the line
/// of a boot that did not come up, as `; failed units: a, b`; nothing
/// where systemctl gives none or does not answer.
async fn failed_units(c: &Client, name: &str) -> String {
    let list = Exec::new(["systemctl", "--failed", "--no-legend", "--plain"]);
    match c.exec_recorded(name, &list).await {
        Ok(o) => units_line(&String::from_utf8_lossy(&o.stdout)),
        Err(_) => String::new(),
    }
}

/// The first word of each line of `systemctl --failed --no-legend
/// --plain`, at most 20, as `; failed units: a, b`.
pub(crate) fn units_line(text: &str) -> String {
    let units: Vec<&str> = text
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .take(20)
        .collect();
    if units.is_empty() {
        String::new()
    } else {
        format!("; failed units: {}", units.join(", "))
    }
}

/// The name of PID 1 of the instance, for the error of the bound.
async fn pid1(c: &Client, name: &str) -> String {
    let comm = Exec::new(["cat", "/proc/1/comm"]);
    match c.exec_recorded(name, &comm).await {
        Ok(o) if o.exit_code == 0 => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Ok(o) => format!("- (cat gives exit {})", o.exit_code),
        Err(e) => format!("- ({e})"),
    }
}

#[cfg(test)]
mod tests;
