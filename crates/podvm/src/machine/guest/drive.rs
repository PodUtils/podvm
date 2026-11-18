//! The emulator of one guest run: spawn it, read its console to the exit
//! marker, and end it within a bound (TODO/machine.md T-1656).

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use podvm_image::error::EXIT_RUNTIME_ERROR;

use super::{init, stderr, tty, Request, DEFAULT_TIMEOUT};

/// Spawn the emulator and read its console to the exit marker.
pub(super) fn drive(
    r: &Request,
    qemu: &Path,
    argv: &[String],
    nonce: &str,
    dir: &Path,
    sock: &Path,
) -> i32 {
    use std::os::unix::process::CommandExt;
    let errlog = dir.join("qemu.err");
    let mut cmd = Command::new(qemu);
    crate::machine::die_with_podvm(&mut cmd); // T-1884
    let mut child = match cmd
        .args(argv)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(
            std::fs::File::create(&errlog)
                .map(Stdio::from)
                .unwrap_or(Stdio::null()),
        )
        .process_group(0)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("podvm {}: machine tier: {}: {e}", r.verb, qemu.display());
            return EXIT_RUNTIME_ERROR;
        }
    };
    let pid = child.id() as i32;
    // TODO/machine.md T-1750: the keeper of `run -d` records the emulator.
    if let Some(k) = &r.keeper {
        let start = podvm_windows::machines::start_of(child.id()).unwrap_or(0);
        let _ = std::fs::write(&k.pidfile, format!("{pid} {start}\n"));
    }
    let started = Instant::now();
    let deadline = started + r.timeout.unwrap_or(DEFAULT_TIMEOUT);
    // PodUtils/podrunner#198: why the wait for the console ended.
    let mut console_closed = false;
    let gone = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let errs = stderr::pump(sock.to_path_buf(), deadline, gone.clone());
    // TODO/machine.md T-1751: the host side of the channel.
    let _channel = (r.channel.is_some() || r.sealed.is_some()).then(|| {
        super::channel::serve(
            super::channel::socket_path(sock),
            r.channel.as_ref().map(|c| c.0.clone()),
            r.sealed.clone(),
            deadline,
        )
    });
    // TODO/machine.md T-1723: the terminal of -t, raw on the host until the
    // end of the run.
    let (_raw, term) = if r.tty {
        (
            Some(tty::Raw::new()),
            Some(tty::pump(tty::socket_path(sock), deadline)),
        )
    } else {
        (None, None)
    };
    let out = child.stdout.take().expect("piped");
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(out).split(b'\n').map_while(Result::ok) {
            if tx
                .send(String::from_utf8_lossy(&line).into_owned())
                .is_err()
            {
                break;
            }
        }
    });
    let mut console = init::Console::new(nonce);
    // PodUtils/podrunner#195: the last lines of the console before the
    // ready marker, for the message of a guest that gave no exit code.
    let mut boot = std::collections::VecDeque::with_capacity(BOOT_LINES);
    let mut ready = false;
    let mut blanks = 0usize;
    let mut code: Option<i32> = None;
    let stdout = std::io::stdout();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left.max(Duration::from_millis(1))) {
            Ok(line) => match console.line(&line) {
                init::Line::Before => {
                    let l = line.trim_end_matches('\r').trim();
                    if !l.is_empty() {
                        if boot.len() == BOOT_LINES {
                            boot.pop_front();
                        }
                        boot.push_back(l.to_string());
                    }
                }
                init::Line::Ready => ready = true,
                init::Line::Out("") => blanks += 1,
                init::Line::Out(s) => {
                    let mut o = stdout.lock();
                    for _ in 0..blanks {
                        let _ = writeln!(o);
                    }
                    blanks = 0;
                    let _ = writeln!(o, "{s}");
                }
                init::Line::Exit(c, before) => {
                    // The script's `echo` before the marker ends a last line
                    // without a newline; that one blank line is not output.
                    let mut o = stdout.lock();
                    for _ in 0..blanks.saturating_sub(1) {
                        let _ = writeln!(o);
                    }
                    if !before.is_empty() {
                        let _ = writeln!(o, "{before}");
                    }
                    code = Some(c);
                    if let Some(n) = console.oom_kills {
                        crate::run::cgroup::set_guest_oom_kills(n);
                    }
                    break;
                }
            },
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                console_closed = true;
                break;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
        }
        if Instant::now() >= deadline {
            break;
        }
    }
    // The guest resets after the marker and the emulator exits; a guest
    // that does not, and each child of the emulator, goes with SIGKILL.
    // TODO/machine.md T-1809: with no marker (the deadline, or an emulator
    // that ended) nothing is left to wait for, so the kill comes at once;
    // the bounded wait of 10 s made `--timeout 2` end at 12 s.
    // PodUtils/podrunner#199: a console that closed comes before the end of
    // the emulator; two seconds give its status (a code or a signal, such
    // as the OOM killer's) in place of "still running".
    let grace = if code.is_some() {
        Duration::from_secs(10)
    } else if console_closed {
        Duration::from_secs(2)
    } else {
        Duration::ZERO
    };
    let timed_out = code.is_none() && Instant::now() >= deadline;
    let status = match wait_bounded(&mut child, grace) {
        Some(s) => Some(s),
        None => {
            // SAFETY: a plain system call on the process group that `spawn`
            // made for the emulator.
            unsafe { kill(-pid, 9) };
            let _ = child.wait();
            None
        }
    };
    // The port closed with the emulator: the pump has each byte now.
    gone.store(true, std::sync::atomic::Ordering::Release);
    let _ = errs.join();
    if let Some(t) = term {
        let _ = t.join();
    }
    match code {
        Some(c) => c,
        // TODO/machine.md T-1809: the contract of `--timeout` of the other
        // rungs: the payload is killed at the bound, and the run exits 137.
        None if timed_out && r.timeout.is_some() => {
            eprintln!(
                "podvm {}: machine tier: the payload ran past --timeout {} s, and the \
                 guest was killed (TODO/machine.md T-1809)",
                r.verb,
                r.timeout.map(|t| t.as_secs()).unwrap_or_default()
            );
            137
        }
        None => {
            let tail = std::fs::read_to_string(dir.join("qemu.err")).unwrap_or_default();
            let tail = tail.lines().last().unwrap_or("");
            // TODO/machine.md T-1823 kept the last line of the emulator for
            // --verbose (it is most often a chardev notice); PodUtils/
            // podrunner#193: a unit runs with no --verbose, and the line was
            // the one fact of the cause. On this failure it is always said.
            eprintln!(
                "podvm {}: machine tier: the guest ended before it gave the exit code of \
                 the payload{}; {}; {}{}",
                r.verb,
                if Instant::now() >= deadline {
                    ", at the bound of the run"
                } else {
                    ""
                },
                ended_how(status, ready),
                waited(started.elapsed(), console_closed, initrd_bytes(argv)),
                if tail.is_empty() {
                    String::new()
                } else {
                    format!("; qemu: {tail}")
                }
            );
            // PodUtils/podrunner#199: a guest that reset with no line, and
            // a large initrd, ran short of memory (T-1922).
            let bytes = initrd_bytes(argv).unwrap_or(0);
            let reset = status.and_then(|s| s.code()) == Some(0) && boot.is_empty();
            if !ready && reset && bytes > 100 << 20 {
                eprintln!(
                    "podvm {}: machine tier: an initrd of {} MiB likely needs about {} MiB of \
                     memory: give --podvm-mem {}M (an estimate, TODO/machine.md T-1922)",
                    r.verb,
                    bytes.div_ceil(1 << 20),
                    super::mem::estimate(bytes) >> 20,
                    super::mem::estimate(bytes) >> 20
                );
            }
            // PodUtils/podrunner#195: what the guest printed last, where it
            // did not reach the command.
            if !ready {
                for l in &boot {
                    eprintln!("podvm {}: machine tier: console: {l}", r.verb);
                }
            }
            EXIT_RUNTIME_ERROR
        }
    }
}

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

/// PodUtils/podrunner#198: what podvm waited for, how long, and the size
/// of what the guest had to unpack first.
fn waited(took: Duration, console_closed: bool, initrd: Option<u64>) -> String {
    let why = if console_closed {
        "the console of the emulator closed"
    } else {
        "the bound of the run came"
    };
    let size = initrd.map_or(String::new(), |b| {
        format!(", with an initrd of {} MiB", b.div_ceil(1 << 20))
    });
    format!("{why} {:.1} s after the start{size}", took.as_secs_f64())
}

/// The size of the file of `-initrd` in the emulator's arguments.
fn initrd_bytes(argv: &[String]) -> Option<u64> {
    let at = argv.iter().position(|a| a == "-initrd")?;
    std::fs::metadata(argv.get(at + 1)?).ok().map(|m| m.len())
}

/// The lines of the console that the message of a failed start keeps.
const BOOT_LINES: usize = 8;

/// The status of the emulator, or `None` where it did not end within `d`.
/// A zero bound still reads an emulator that has ended.
fn wait_bounded(child: &mut std::process::Child, d: Duration) -> Option<std::process::ExitStatus> {
    let end = Instant::now() + d;
    loop {
        if let Ok(Some(s)) = child.try_wait() {
            return Some(s);
        }
        if Instant::now() >= end {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// How the emulator ended, and whether the guest reached the command
/// (PodUtils/podrunner#195): an exit code, a signal (`SIGKILL` from podvm
/// at the bound, or from another process such as the OOM killer), or still
/// running when podvm killed it.
fn ended_how(status: Option<std::process::ExitStatus>, ready: bool) -> String {
    use std::os::unix::process::ExitStatusExt;
    let how = match status {
        None => "the emulator was still running, and podvm killed it".to_string(),
        Some(s) => match (s.code(), s.signal()) {
            (Some(c), _) => format!("the emulator exited with code {c}"),
            (None, Some(sig)) => format!("the emulator ended on signal {sig}"),
            _ => format!("the emulator ended: {s}"),
        },
    };
    let stage = if ready {
        "after the guest started the command"
    } else {
        "before the guest reached the command"
    };
    format!("{how}, {stage}")
}

/// TODO/machine.md T-1906: remove each `run-PID-*` directory under `dir`
/// whose PID is not a running process: a podvm that was killed never
/// reached the removal at the end of its boot. A live PID keeps its
/// directory, also where the kernel gave the PID to another process.
pub(super) fn sweep(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name();
        let Some(pid) = name
            .to_str()
            .and_then(|n| n.strip_prefix("run-"))
            .and_then(|r| r.split('-').next())
            .and_then(|p| p.parse::<u32>().ok())
        else {
            continue;
        };
        if pid != std::process::id() && !Path::new("/proc").join(pid.to_string()).exists() {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

#[cfg(test)]
mod sweep_tests {
    use super::*;

    #[test]
    fn machine_run_dirs_of_a_dead_pid_go() {
        let d = std::env::temp_dir().join(format!("podvm-t1906-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        // A PID above the kernel's limit runs nothing.
        let dead = d.join("run-4194999-abcdef01");
        let live = d.join(format!("run-{}-abcdef02", std::process::id()));
        let other = d.join("base-1.cpio");
        for p in [&dead, &live] {
            std::fs::create_dir_all(p).unwrap();
        }
        std::fs::write(&other, b"x").unwrap();
        sweep(&d);
        assert!(!dead.exists(), "the directory of a dead PID stays");
        assert!(live.exists(), "the directory of a live PID went");
        assert!(other.exists(), "a file that is no run directory went");
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[cfg(test)]
mod tests {
    use super::ended_how;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    /// PodUtils/podrunner#195: the message names the end of the emulator.
    #[test]
    fn the_end_of_the_emulator_is_named() {
        let code = ExitStatus::from_raw(1 << 8);
        let killed = ExitStatus::from_raw(9);
        assert_eq!(
            ended_how(Some(code), false),
            "the emulator exited with code 1, before the guest reached the command"
        );
        assert_eq!(
            ended_how(Some(killed), true),
            "the emulator ended on signal 9, after the guest started the command"
        );
        assert!(ended_how(None, false).starts_with("the emulator was still running"));
    }

    /// PodUtils/podrunner#198: the wait, its time and the initrd are named.
    #[test]
    fn the_wait_is_named_with_its_time_and_the_initrd() {
        use super::{initrd_bytes, waited};
        use std::time::Duration;
        assert_eq!(
            waited(Duration::from_millis(2500), true, Some(300 << 20)),
            "the console of the emulator closed 2.5 s after the start, with an initrd of 300 MiB"
        );
        assert_eq!(
            waited(Duration::from_secs(600), false, None),
            "the bound of the run came 600.0 s after the start"
        );
        assert_eq!(initrd_bytes(&["-m".to_string(), "512M".to_string()]), None);
    }
}
