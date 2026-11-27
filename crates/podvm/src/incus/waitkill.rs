//! `wait` and `kill` with `--podvm-tier=incus` (TODO/cli.md T-2087,
//! OR-B40-04 of the ninth outside pass): docker's `run -d`, `wait` and
//! `kill` on an instance of podvm.
//!
//! * `wait NAME` reads the state of the instance each [`PAUSE`] until it
//!   stops, then prints the exit code that its init wrote to
//!   [`super::EXIT`]. An instance that never started has no exit code, as
//!   a created container of the other tiers (TODO/supervise.md T-0604).
//! * `kill [-s SIG] NAME` runs `podvm guest-kill SIG` in the instance by the
//!   exec API: podvm in the image sends the signal to the command, whose
//!   pid the init writes to [`super::PID`] while it runs. incusd has no
//!   call that signals a process of an instance. The default is SIGKILL,
//!   as docker's; the init then ends the instance with 128 + 9.
//!
//! Limit: between the end of the command and the removal of the pid file
//! by the init, another process of the instance can take the same pid.

use std::time::{Duration, Instant};

use pod_incus::{Client, Exec, FileContent, FileWrite, Payload};
use podvm_image::error::{EXIT_CLI_ERROR, EXIT_FLAG_ERROR, EXIT_RUNTIME_ERROR};

use super::verbs::{block_on, connect, gone, ours};

/// The verb of podvm in the instance that sends a signal to the command.
pub const GUEST_KILL: &str = "guest-kill";

/// The pause between two reads of the state of an instance.
pub const PAUSE: Duration = Duration::from_millis(250);
/// The pause for an instance of `run --rm`: incusd removes it, and the
/// exit file with it, when it stops.
pub const PAUSE_RM: Duration = Duration::from_millis(100);
/// How long a stopped instance can give no exit file before `wait` says
/// so: the state can say `Stopped` before the init wrote the file.
pub const SETTLE: Duration = Duration::from_secs(3);

/// The exit code and the line of a refusal.
pub type Refusal = (i32, String);

/// The exit code of `podvm guest-kill` where the command already ended.
pub const GUEST_ENDED: i32 = 3;

/// The line of a 404 of incusd for the instance `name`.
fn no_such(name: &str) -> Refusal {
    (
        EXIT_CLI_ERROR,
        format!("no such instance in incus: {name}. `podvm ps -a --podvm-tier incus` lists them"),
    )
}

/// The instance `name`, where it is one of podvm.
async fn instance(c: &Client, name: &str) -> Result<pod_incus::Instance, Refusal> {
    match c.instance(name).await {
        Ok(i) if ours(&i) => Ok(i),
        Ok(_) => Err((
            EXIT_CLI_ERROR,
            "not an instance of podvm (no user.podvm label)".into(),
        )),
        Err(e) if gone(&e) => Err(no_such(name)),
        Err(e) => Err((EXIT_RUNTIME_ERROR, e.to_string())),
    }
}

/// The exit code in [`super::EXIT`] of the instance `name`.
pub async fn read_exit(c: &Client, name: &str) -> Result<i32, String> {
    let f = c
        .get_file(name, super::EXIT)
        .await
        .map_err(|e| e.to_string())?;
    match f.content {
        FileContent::Bytes(b) => {
            let t = String::from_utf8_lossy(&b);
            t.trim()
                .parse::<i32>()
                .map_err(|_| format!("{}: {:?} is not an exit code", super::EXIT, t.trim()))
        }
        FileContent::Entries(_) => Err(format!("{} is a directory", super::EXIT)),
    }
}

/// The pauses and the bounds of [`wait_one`].
#[derive(Debug, Clone, Copy)]
pub struct Pace {
    /// Between two reads of the state.
    pub pause: Duration,
    /// How long a stopped instance can give no exit file.
    pub settle: Duration,
    /// The bound of the whole wait.
    pub bound: Duration,
}

impl Pace {
    /// The pace of `podvm wait`.
    pub fn of_wait() -> Pace {
        Pace {
            pause: PAUSE,
            settle: SETTLE,
            bound: Duration::from_millis(crate::lifecycle::WAIT_BOUND_MS),
        }
    }
}

/// Wait until the instance `name` stops, at most `pace.bound`; its exit
/// code.
pub async fn wait_one(c: &Client, name: &str, pace: Pace) -> Result<i32, Refusal> {
    let start = Instant::now();
    let mut stopped: Option<Instant> = None;
    // The exit code that an instance of `run --rm` wrote before it went.
    let mut early: Option<i32> = None;
    let mut seen = false;
    loop {
        let i = match c.instance(name).await {
            Ok(i) if ours(&i) => i,
            Ok(_) => {
                return Err((
                    EXIT_CLI_ERROR,
                    "not an instance of podvm (no user.podvm label)".into(),
                ))
            }
            Err(e) if gone(&e) && seen => {
                return early.ok_or((
                    EXIT_RUNTIME_ERROR,
                    "the instance went before podvm read its exit code (incusd removes an \
                     instance of run --rm when it stops, and the exit code with it)"
                        .into(),
                ));
            }
            Err(e) if gone(&e) => return Err(no_such(name)),
            Err(e) => return Err((EXIT_RUNTIME_ERROR, e.to_string())),
        };
        seen = true;
        if super::ps::never_started(&i) {
            return Err((
                EXIT_RUNTIME_ERROR,
                "has no exit code podvm measured: it is Created (TODO/supervise.md T-0604)".into(),
            ));
        }
        let ended = matches!(i.status.as_str(), "Stopped" | "Error");
        // TODO/machine.md T-1998: incusd reads the files of a VM only while
        // it runs; the VM waits for this read before it stops.
        let vm = i.kind == "virtual-machine";
        if vm && ended {
            return Err((EXIT_RUNTIME_ERROR, vm_gone(&i.status)));
        }
        if ended || i.ephemeral || vm {
            match read_exit(c, name).await {
                Ok(code) if ended => return Ok(code),
                Ok(code) if vm => {
                    let mut w = FileWrite::file(Payload::bytes(b"1\n".to_vec()));
                    (w.uid, w.gid, w.mode) = (Some(0), Some(0), Some(0o600));
                    let _ = c.put_file(name, super::EXIT_READ, w).await;
                    return Ok(code);
                }
                Ok(code) => early = Some(code),
                Err(e) if ended => {
                    let since = *stopped.get_or_insert_with(Instant::now);
                    if since.elapsed() >= pace.settle {
                        return Err((
                            EXIT_RUNTIME_ERROR,
                            format!(
                                "the instance is {} and has no exit code podvm measured ({e}): \
                                 a forced stop, or its init ended before the command",
                                i.status
                            ),
                        ));
                    }
                }
                Err(_) => {}
            }
        }
        if !ended {
            stopped = None;
        }
        if start.elapsed() >= pace.bound {
            return Err((
                EXIT_RUNTIME_ERROR,
                format!(
                    "has no exit code podvm measured: it is {} after {} s, the bound of wait",
                    i.status,
                    pace.bound.as_secs()
                ),
            ));
        }
        let p = match i.ephemeral {
            true => pace.pause.min(PAUSE_RM),
            false => pace.pause,
        };
        tokio::time::sleep(p).await;
    }
}

/// The line of a VM that stopped before `wait` read its exit code
/// (TODO/machine.md T-1998).
pub fn vm_gone(status: &str) -> String {
    format!(
        "the virtual machine is {status} and podvm read no exit code: incusd reads the files \
         of a VM only while it runs, and the VM stays up for {} s after its command ends \
         for this read (TODO/machine.md T-1998)",
        super::vm::LINGER_S
    )
}

/// Send the signal `sig` to the command of the instance `name`.
pub async fn kill_one(c: &Client, name: &str, sig: i32) -> Result<(), Refusal> {
    let i = instance(c, name).await?;
    if i.status != "Running" {
        return Err((
            EXIT_CLI_ERROR,
            format!(
                "the instance is {}, not running: no command to signal",
                super::ps::status(&i)
            ),
        ));
    }
    let x = Exec::new([
        super::guest_exec::PODVM.to_string(),
        GUEST_KILL.to_string(),
        sig.to_string(),
    ]);
    let out = c
        .exec_recorded(name, &x)
        .await
        .map_err(|e| (EXIT_RUNTIME_ERROR, e.to_string()))?;
    if out.exit_code == 0 {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&out.stderr);
    let said = said.lines().next().unwrap_or("").trim().to_string();
    let said = match said.is_empty() {
        true => format!("ended with {}", out.exit_code),
        false => said,
    };
    let said = format!(
        "{} {GUEST_KILL} in the instance: {said}",
        super::guest_exec::PODVM
    );
    let code = if out.exit_code == GUEST_ENDED {
        EXIT_CLI_ERROR
    } else {
        EXIT_RUNTIME_ERROR
    };
    Err((code, said))
}

/// The names of `args`, or the exit of a flag that is not served.
fn names(verb: &str, args: &[String], usage: &str) -> Result<Vec<String>, i32> {
    let mut out = Vec::new();
    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                println!("{usage}");
                return Err(0);
            }
            f if f.starts_with('-') => {
                eprintln!(
                    "podvm {verb}: {f} is not served with --podvm-tier=incus (TODO/cli.md T-2087)"
                );
                return Err(EXIT_FLAG_ERROR);
            }
            n => out.push(n.to_string()),
        }
    }
    if out.is_empty() {
        eprintln!("{usage}");
        return Err(EXIT_CLI_ERROR);
    }
    Ok(out)
}

/// Each name of `names` with `one`: the line of each refusal, and the
/// code of the last one.
fn each<F>(verb: &'static str, names: Vec<String>, one: F) -> i32
where
    F: AsyncFn(&Client, &str) -> Result<String, Refusal>,
{
    block_on(verb, async move {
        let c = match connect().await {
            Ok((c, _)) => c,
            Err(e) => {
                eprintln!("podvm {verb}: incus tier: {e}");
                return EXIT_RUNTIME_ERROR;
            }
        };
        let mut code = 0;
        for n in &names {
            match one(&c, n).await {
                Ok(line) => println!("{line}"),
                Err((c, line)) => {
                    eprintln!("podvm {verb}: {n}: {line}");
                    code = c;
                }
            }
        }
        code
    })
}

/// `podvm wait --podvm-tier=incus NAME...`.
pub fn wait(args: &[String]) -> i32 {
    const USAGE: &str = "usage: podvm wait [--podvm-tier incus] <container> [container...]";
    let names = match names("wait", args, USAGE) {
        Ok(n) => n,
        Err(c) => return c,
    };
    each("wait", names, async |c: &Client, n: &str| {
        wait_one(c, n, Pace::of_wait()).await.map(|x| x.to_string())
    })
}

/// The signal of `-s SIG`, `--signal SIG`, `--signal=SIG` or `-sSIG`, and
/// the other arguments.
pub fn signal_of(args: &[String]) -> Result<(i32, Vec<String>), Refusal> {
    let mut sig = 9;
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let v = match a.as_str() {
            "-s" | "--signal" => match it.next() {
                Some(v) => v.clone(),
                None => return Err((EXIT_FLAG_ERROR, format!("{a} needs a signal"))),
            },
            x if x.starts_with("--signal=") => x["--signal=".len()..].to_string(),
            x if x.starts_with("-s") && x.len() > 2 && !x.starts_with("--") => x[2..].to_string(),
            _ => {
                rest.push(a.clone());
                continue;
            }
        };
        sig = crate::lifecycle::signal_number(&v)
            .ok_or_else(|| (EXIT_FLAG_ERROR, crate::lifecycle::signal_refusal(&v)))?;
    }
    Ok((sig, rest))
}

/// `podvm kill --podvm-tier=incus [-s SIG] NAME...`.
pub fn kill(args: &[String]) -> i32 {
    const USAGE: &str =
        "usage: podvm kill [-s SIGNAL] [--podvm-tier incus] <container> [container...]";
    let (sig, rest) = match signal_of(args) {
        Ok(s) => s,
        Err((c, line)) => {
            eprintln!("podvm kill: {line}");
            return c;
        }
    };
    let names = match names("kill", &rest, USAGE) {
        Ok(n) => n,
        Err(c) => return c,
    };
    each("kill", names, async move |c: &Client, n: &str| {
        kill_one(c, n, sig).await.map(|()| n.to_string())
    })
}

/// `podvm guest-kill SIGNAL`, in the instance: send the signal to the pid
/// of [`super::PID`]. Exit 0 where it went, [`GUEST_ENDED`] where the
/// command ended, 2 where there is no pid, 1 for another error.
pub fn guest_kill(args: &[String]) -> i32 {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    let Some(sig) = args
        .first()
        .and_then(|a| a.parse::<i32>().ok())
        .filter(|s| (1..65).contains(s))
    else {
        eprintln!("usage: podvm {GUEST_KILL} SIGNAL (a number from 1 to 64)");
        return 2;
    };
    let text = match std::fs::read_to_string(super::PID) {
        Ok(t) => t,
        Err(e) => {
            eprintln!(
                "{}: {e}: the command has ended or not started, or an older podvm made \
                 the instance",
                super::PID
            );
            return 2;
        }
    };
    // Never PID 1: the init of the instance is not the command.
    let Some(pid) = text.trim().parse::<i32>().ok().filter(|p| *p > 1) else {
        eprintln!(
            "{}: {:?} is not the pid of a command",
            super::PID,
            text.trim()
        );
        return 2;
    };
    // SAFETY: a plain system call.
    if unsafe { kill(pid, sig) } == 0 {
        return 0;
    }
    let e = std::io::Error::last_os_error();
    if e.raw_os_error() == Some(3) {
        eprintln!("the command (pid {pid}) has ended");
        return GUEST_ENDED;
    }
    eprintln!("kill of the command (pid {pid}) with signal {sig}: {e}");
    1
}
