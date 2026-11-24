//! A command of the incus tier with its websockets (TODO/machine.md T-2080;
//! OR-B39-01, OR-B39-03, OR-B39-05 and OR-B39-06 of the eighth outside
//! pass). `run` in the foreground and `exec` use it.
//!
//! Before, both used the exec of incus with `record-output`: podvm gave the
//! output only at the end (stderr after stdout), dropped the stdin of
//! `exec`, and a payload that ended with 126 or 127 came back as "incus
//! refused the call (400): Command not found", exit 125, with its output
//! lost: incusd 6.0 ends such an operation as a failure, and its wait
//! answers with an error, not with the operation (measured on 2026-11-24).
//! A SIGINT, SIGTERM or SIGHUP ended podvm with its default action and left
//! the instance of `run --rm` running.
//!
//! Now:
//!
//! * stdout and stderr go to podvm's own as they come; stdin goes in;
//! * the exit code of an operation that failed with "Command not found" or
//!   "Command not executable" is its `metadata.return` ([`code_of_failure`]),
//!   read with `GET /1.0/operations/ID` after the wait;
//! * podvm blocks the signals of `pod_enter::WAIT_FORWARDED` and reads them
//!   from a signalfd ([`Signals`]). The first one goes to the command by the
//!   control socket ("forwarded SIGTERM to the payload in the instance");
//!   after a second one, or [`GRACE`] with no end, SIGKILL goes to it. The
//!   caller of `run` then removes the instance. The exit code is the one of
//!   the command, else 128 + N of the first signal.

use std::io::Write;
use std::os::fd::{AsRawFd, RawFd};
use std::time::Duration;

use pod_incus::{Client, Exec};
use podvm_probe::sys as k;
use serde_json::Value;

/// The time that the command has to end after the first signal.
pub const GRACE: Duration = Duration::from_secs(10);
/// The bound of the wait for the end of the operation after the streams
/// ended (or after SIGKILL).
pub const END_WAIT: Duration = Duration::from_secs(30);
const KILL_WAIT: Duration = Duration::from_secs(5);

/// The words of incusd 6.0 for an operation of exec whose command ended
/// with 127 and 126 (`ErrExecCommandNotFound`, `ErrExecCommandNotExecutable`).
pub const NOT_FOUND: &str = "Command not found";
pub const NOT_EXECUTABLE: &str = "Command not executable";

/// The name of the signal `n`.
pub fn sig_name(n: i32) -> &'static str {
    crate::machine::guest::signals::SET
        .iter()
        .find(|(s, _)| *s == n)
        .map_or("a signal", |(_, name)| *name)
}

/// The exit code of an operation of exec that failed with the error
/// `err`, where incusd gives its `return` in `ret`: `None` for another
/// failure. incusd gives these two words only for 127 and 126, so a
/// missing or odd `return` takes the code of the words.
pub fn code_of_failure(err: &str, ret: Option<i64>) -> Option<i32> {
    let by_words = match err.trim() {
        NOT_FOUND => 127,
        NOT_EXECUTABLE => 126,
        _ => return None,
    };
    Some(match ret {
        Some(r) if (0..=255).contains(&r) => r as i32,
        _ => by_words,
    })
}

/// The signalfd.
struct SigFd(RawFd);

impl AsRawFd for SigFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

impl Drop for SigFd {
    fn drop(&mut self) {
        let _ = k::close(self.0 as i64);
    }
}

/// The signals of a run or an exec that go to its command.
pub struct Signals {
    fd: Option<tokio::io::unix::AsyncFd<SigFd>>,
    /// A test gives its signals here: each after its pause.
    script: std::collections::VecDeque<(Duration, i32)>,
}

impl Signals {
    /// Block the set in this thread and read it from a signalfd. Call it in
    /// the runtime of one thread, before each other thread of the run
    /// starts: a thread takes the mask of its maker. Where a step is
    /// refused, the mask comes back, a line says so, and a signal ends
    /// podvm as before.
    pub fn open(verb: &str) -> Signals {
        let mut old = 0u64;
        let fd = podvm_enter::block_forwarded(&mut old)
            .and_then(|_| k::signalfd_proxied())
            .map_err(|e| e.name().to_string())
            .and_then(|fd| {
                // F_SETFL, O_NONBLOCK: the reactor reads it until EAGAIN.
                let set = k::fcntl(fd, 4, 0o4000).map(drop);
                let sfd = SigFd(fd as RawFd);
                set.map_err(|e| e.name().to_string())?;
                // SAFETY: SigFd owns the descriptor and closes it only in
                // its Drop, after the AsyncFd that holds it deregistered it.
                unsafe {
                    tokio::io::unix::AsyncFd::register_with_interest(
                        sfd,
                        tokio::io::Interest::READABLE,
                    )
                }
                .map_err(|e| e.to_string())
            });
        match fd {
            Ok(fd) => Signals {
                fd: Some(fd),
                script: Default::default(),
            },
            Err(why) => {
                let _ = k::sigrestore(old);
                eprintln!(
                    "podvm {verb}: incus tier: no signal goes to the command ({why}): a SIGTERM \
                     ends podvm and the instance stays (TODO/machine.md T-2080)"
                );
                Signals {
                    fd: None,
                    script: Default::default(),
                }
            }
        }
    }

    /// Signals of a test: no mask, no signalfd.
    #[cfg(test)]
    pub fn scripted(script: &[(Duration, i32)]) -> Signals {
        Signals {
            fd: None,
            script: script.iter().copied().collect(),
        }
    }

    /// The next signal; it never ends where none can come.
    pub async fn next(&mut self) -> i32 {
        if let Some((pause, s)) = self.script.pop_front() {
            tokio::time::sleep(pause).await;
            return s;
        }
        let Some(fd) = &self.fd else {
            return std::future::pending().await;
        };
        loop {
            let Ok(mut ready) = fd.readable().await else {
                return std::future::pending().await;
            };
            let mut buf = [0u8; 128];
            match k::read(fd.get_ref().0 as i64, &mut buf) {
                Ok(n) if n >= 4 => return i32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]),
                Err(e) if e == k::EWOULDBLOCK => ready.clear_ready(),
                _ => return std::future::pending().await,
            }
        }
    }
}

/// The line and the exit code of a signal that came before the command
/// started: podvm ends with 128 + N, and the caller removes what it made.
pub fn before_command(verb: &str, s: i32) -> i32 {
    eprintln!(
        "podvm {verb}: incus tier: {} came before the command started: podvm ends with {} \
         (TODO/machine.md T-2080)",
        sig_name(s),
        128 + s
    );
    128 + s
}

/// What a command gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    pub code: i32,
    /// The bytes of stdout and stderr together.
    pub wrote: u64,
}

/// Copy each message of `o` to `w`; the count of bytes. Where `w` refuses
/// a write (the reader of a pipe went: EPIPE, as SIGPIPE is blocked), the
/// command gets SIGPIPE by `control`, as a command that writes to that pipe
/// itself would, and the rest of its output is read and dropped, so that
/// the command ends and its instance goes (TODO/machine.md T-2080).
async fn pump(
    o: &mut pod_incus::Output,
    w: &mut dyn Write,
    control: &pod_incus::Control,
) -> (u64, Option<String>) {
    let mut n = 0u64;
    let mut broken = false;
    loop {
        match o.read().await {
            Ok(Some(b)) => {
                n += b.len() as u64;
                if broken {
                    continue;
                }
                if w.write_all(&b).and_then(|()| w.flush()).is_err() {
                    broken = true;
                    let _ = control.signal(13).await;
                }
            }
            Ok(None) => return (n, None),
            Err(e) => return (n, Some(e.to_string())),
        }
    }
}

/// Run `exec` in the instance `inst` with its websockets: `input` is its
/// stdin, its stdout and stderr go to `out` and `err`, and the signals of
/// `sig` go to it. `Err` where incusd did not start it or gave no end.
#[allow(clippy::too_many_arguments)]
pub async fn exec_streamed(
    c: &Client,
    verb: &str,
    inst: &str,
    exec: &Exec,
    input: &[u8],
    sig: &mut Signals,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<Outcome, String> {
    // A write to a pipe whose reader went gives EPIPE, not a SIGPIPE that
    // ends podvm and leaves the instance ([`pump`]).
    let mut old = 0u64;
    let _ = k::sigblock(1 << (13 - 1), &mut old);
    let mut a = c
        .exec_attached(inst, exec)
        .await
        .map_err(|e| e.to_string())?;
    let id = a.operation.id.clone();
    let mut first: Option<i32> = None;
    let mut hard = false;
    let mut wrote = 0u64;
    {
        let pod_incus::Attached {
            stdin,
            stdout,
            stderr,
            control,
            ..
        } = &mut a;
        let feed = async {
            // A command that does not read its stdin closes it first.
            if input.is_empty() || stdin.write(input).await.is_ok() {
                let _ = stdin.close().await;
            }
        };
        let ctl: &pod_incus::Control = control;
        let streams = async { tokio::join!(feed, pump(stdout, out, ctl), pump(stderr, err, ctl)) };
        tokio::pin!(streams);
        let grace = tokio::time::sleep(Duration::from_secs(365 * 24 * 3600));
        tokio::pin!(grace);
        loop {
            tokio::select! {
                ((), (n1, e1), (n2, e2)) = &mut streams => {
                    wrote = n1 + n2;
                    for (what, e) in [("stdout", e1), ("stderr", e2)] {
                        if let Some(e) = e {
                            eprintln!("podvm {verb}: incus tier: the {what} of the command: {e}");
                        }
                    }
                    break;
                }
                s = sig.next() => {
                    if first.is_some() {
                        eprintln!(
                            "podvm {verb}: incus tier: a second signal ({}) came: SIGKILL goes to \
                             the payload in the instance {inst}",
                            sig_name(s)
                        );
                        hard = true;
                        break;
                    }
                    first = Some(s);
                    match ctl.signal(s).await {
                        Ok(()) => eprintln!(
                            "podvm {verb}: forwarded {} to the payload in the instance {inst}",
                            sig_name(s)
                        ),
                        Err(e) => {
                            eprintln!(
                                "podvm {verb}: incus tier: {} came and did not reach the payload \
                                 ({e}): SIGKILL goes to it",
                                sig_name(s)
                            );
                            hard = true;
                            break;
                        }
                    }
                    grace.as_mut().reset(tokio::time::Instant::now() + GRACE);
                }
                () = &mut grace, if first.is_some() => {
                    eprintln!(
                        "podvm {verb}: incus tier: the payload did not end {} s after {}: SIGKILL \
                         goes to it",
                        GRACE.as_secs(),
                        sig_name(first.unwrap_or(15))
                    );
                    hard = true;
                    break;
                }
            }
        }
        if hard {
            let _ = ctl.signal(9).await;
        }
    }
    let bound = if hard { KILL_WAIT } else { END_WAIT };
    match finish(c, a, &id, bound).await {
        Ok(code) => Ok(Outcome { code, wrote }),
        Err(e) => match first {
            // After a signal the end of the command is the signal's.
            Some(s) => {
                eprintln!("podvm {verb}: incus tier: {e}");
                Ok(Outcome {
                    code: 128 + s,
                    wrote,
                })
            }
            None => Err(e),
        },
    }
}

/// The exit code of the command of the operation `id`, within `bound`.
async fn finish(
    c: &Client,
    a: pod_incus::Attached,
    id: &str,
    bound: Duration,
) -> Result<i32, String> {
    let e = match tokio::time::timeout(bound, a.wait()).await {
        Err(_) => {
            return Err(format!(
                "the operation {id} of the command gave no end within {} s",
                bound.as_secs()
            ))
        }
        Ok(Ok(code)) => return Ok(code),
        Ok(Err(e)) => e,
    };
    let words = e.incus_error().unwrap_or_default().to_string();
    if code_of_failure(&words, None).is_some() {
        // incusd keeps an operation that ended for some seconds.
        let ret = match tokio::time::timeout(END_WAIT, c.operation(id)).await {
            Ok(Ok(op)) => op.metadata.get("return").and_then(Value::as_i64),
            _ => None,
        };
        if let Some(code) = code_of_failure(&words, ret) {
            return Ok(code);
        }
    }
    Err(e.to_string())
}
