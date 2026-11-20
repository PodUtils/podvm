//! TODO/machine.md T-2041 (OQ-02 of the sixth outside pass): a SIGHUP,
//! SIGINT, SIGTERM or another signal of `pod_enter::WAIT_FORWARDED` to a
//! foreground run of the machine tier goes to the command in the guest, as
//! the waiter of the rootfs rungs forwards it to the payload.
//!
//! Before, the signal ended podvm with its default action (143, 130, 129)
//! and the emulator with it, and the trap of the command never ran.
//!
//! * The host ([`Forward`]) blocks the set in the thread of the drive
//!   before it starts the emulator and each thread of the run, reads each
//!   signal from a signalfd, and writes one line [`frame`] on the first
//!   serial port (the stdin of the emulator).
//! * In the guest, `podvm guest-signals NONCE` ([`guest_main`]) puts the
//!   console in raw mode (no echo of the line), says [`up_marker`] before
//!   the ready marker, and sends each signal of a line of this run to the
//!   command, whose pid the init writes to [`PID_FILE`] ([`wrap`]).
//! * Where the guest cannot take it (no podvm in the guest, the command did
//!   not start, the line does not go out), podvm ends with the signal as
//!   before, after one line that says so.

use std::io::Write;

use podvm_probe::sys as k;

/// The file of the pid of the command in the guest.
pub const PID_FILE: &str = "/podvm-cmd.pid";
const UP: &str = "PODVM-SIGNALS-";
const SIG: &str = "PODVM-SIG-";

/// The signals that go to the guest, with their names: the set of
/// `pod_enter::WAIT_FORWARDED`.
pub const SET: [(i32, &str); 8] = [
    (1, "SIGHUP"),
    (2, "SIGINT"),
    (3, "SIGQUIT"),
    (10, "SIGUSR1"),
    (12, "SIGUSR2"),
    (14, "SIGALRM"),
    (15, "SIGTERM"),
    (30, "SIGPWR"),
];

fn name(signo: i32) -> Option<&'static str> {
    SET.iter().find(|(s, _)| *s == signo).map(|(_, n)| *n)
}

/// The line of the guest whose reader runs.
pub fn up_marker(nonce: &str) -> String {
    format!("{UP}{nonce}")
}

/// The line of one signal for the guest of the run `nonce`.
pub fn frame(nonce: &str, signo: i32) -> String {
    format!("\n{SIG}{nonce} {signo}\n")
}

/// The signal of one line of the console, where it is a frame of this run
/// with a signal of [`SET`].
pub fn parse(line: &str, nonce: &str) -> Option<i32> {
    let rest = line.trim().strip_prefix(SIG)?.strip_prefix(nonce)?;
    let signo = rest.strip_prefix(' ')?.parse::<i32>().ok()?;
    name(signo).map(|_| signo)
}

/// The line of the init that starts the reader, before the ready marker.
pub fn start(nonce: &str) -> String {
    format!("[ -x /podvm/podvm ] && /podvm/podvm guest-signals {nonce}\n")
}

/// The command `run` (shell words that end with `"$@"`), with its pid in
/// [`PID_FILE`]: `exec` keeps the pid, and the command stays in the
/// foreground, so no signal is ignored on its entry.
pub fn wrap(run: &str) -> String {
    format!("sh -c 'echo $$ >{PID_FILE} 2>/dev/null; exec \"$@\"' /podvm-init {run}")
}

/// Block the set in this thread from the start of the keeper of `run -d`
/// (T-2042): a `kill` during its boot then waits for the drive, which ends
/// the keeper with the signal and writes the end of the record, where the
/// default action ended it with no end.
pub fn block_early() {
    let mut old = 0u64;
    let _ = k::sigblock(podvm_enter::WAIT_FORWARDED, &mut old);
}

/// The forward of the host for one drive of an emulator.
pub struct Forward {
    sfd: Option<i64>,
    up: bool,
    nonce: String,
    verb: String,
}

impl Forward {
    /// Block the set in this thread (each thread that it starts after this
    /// inherits the mask; the emulator gets an empty mask from its spawn)
    /// and open the signalfd. Where one is refused, a line says so and the
    /// run goes on as before.
    pub fn new(verb: &str, nonce: &str) -> Forward {
        let mut old = 0u64;
        let sfd = k::sigblock(podvm_enter::WAIT_FORWARDED, &mut old)
            .and_then(|_| k::signalfd(podvm_enter::WAIT_FORWARDED));
        let sfd = match sfd {
            Ok(fd) => Some(fd),
            Err(e) => {
                let _ = k::sigrestore(old);
                eprintln!(
                    "podvm {verb}: machine tier: no signal goes to the guest ({}): a SIGTERM \
                     ends podvm and the guest (TODO/machine.md T-2041)",
                    e.name()
                );
                None
            }
        };
        Forward {
            sfd,
            up: false,
            nonce: nonce.to_string(),
            verb: verb.to_string(),
        }
    }

    /// True where `line` of the console is the marker of the reader.
    pub fn line(&mut self, line: &str) -> bool {
        let up = line.trim() == up_marker(&self.nonce);
        self.up |= up;
        up
    }

    /// Give each signal that came to the guest. `stdin` is the stdin of the
    /// emulator; `sent` the writer of the job, which gives it back.
    pub fn poll(
        &mut self,
        stdin: &mut Option<std::process::ChildStdin>,
        sent: &mut Option<std::thread::JoinHandle<std::process::ChildStdin>>,
    ) {
        let Some(sfd) = self.sfd else { return };
        loop {
            let mut fds = [k::PollFd {
                fd: sfd as i32,
                events: k::POLLIN,
                revents: 0,
            }];
            if !matches!(k::ppoll(&mut fds, 0), Ok(n) if n > 0) {
                return;
            }
            let mut buf = [0u8; 128];
            let signo = match k::read(sfd, &mut buf) {
                Ok(n) if n >= 4 => i32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]),
                _ => return,
            };
            if stdin.is_none() && sent.as_ref().is_some_and(|s| s.is_finished()) {
                *stdin = sent.take().and_then(|s| s.join().ok());
            }
            let sig = name(signo).unwrap_or("a signal");
            let why = if !self.up {
                Some("takes no signal yet (its command did not start, or it has no podvm)".into())
            } else {
                send(stdin, frame(&self.nonce, signo).into_bytes()).err()
            };
            match why {
                None => eprintln!(
                    "podvm {}: forwarded {sig} to the payload in the guest",
                    self.verb
                ),
                Some(why) => {
                    eprintln!(
                        "podvm {}: {sig} came, and the guest {why}: podvm ends with {sig} \
                         and the guest with it (TODO/machine.md T-2041)",
                        self.verb
                    );
                    // The keeper of `run -d` writes the end (T-2042).
                    crate::machine::detach::ended(128 + signo);
                    raise(signo);
                }
            }
        }
    }
}

impl Drop for Forward {
    fn drop(&mut self) {
        if let Some(fd) = self.sfd {
            let _ = k::close(fd);
        }
    }
}

/// Write `line` to the stdin of the emulator, in a thread of its own that
/// blocks SIGPIPE: an emulator that ended gives EPIPE, and the end of the
/// thread drops its SIGPIPE (as `state::send_job`).
fn send(stdin: &mut Option<std::process::ChildStdin>, line: Vec<u8>) -> Result<(), String> {
    let Some(mut w) = stdin.take() else {
        return Err("has no serial line from podvm".to_string());
    };
    let t = std::thread::spawn(move || {
        let mut old = 0u64;
        let _ = k::sigblock(1 << (13 - 1), &mut old);
        let r = w.write_all(&line).and_then(|()| w.flush());
        (w, r)
    });
    match t.join() {
        Ok((w, r)) => {
            *stdin = Some(w);
            r.map_err(|e| format!("did not take it: {e}"))
        }
        Err(_) => Err("did not take it: the writer of the line failed".to_string()),
    }
}

/// End podvm with `signo`, as before the forward: unblock it in this thread
/// and send it to this process; an exit with 128 and the signal where it
/// does not end podvm.
fn raise(signo: i32) -> ! {
    let mut now = 0u64;
    let _ = k::sigblock(0, &mut now);
    let _ = k::sigrestore(now & !(1u64 << (signo - 1)));
    let _ = k::kill(k::getpid(), i64::from(signo));
    std::process::exit(128 + signo)
}

unsafe extern "C" {
    fn fork() -> i32;
}

/// `podvm guest-signals NONCE`, in a guest: the console raw, a reader of
/// the console in the background, and the marker [`up_marker`].
pub fn guest_main(args: &[String]) -> i32 {
    let [nonce] = args else {
        eprintln!("podvm guest-signals: the argument is NONCE");
        return 125;
    };
    super::tty::make_raw(0);
    // SAFETY: this process has one thread; the child reads and signals.
    let pid = unsafe { fork() };
    if pid < 0 {
        eprintln!(
            "podvm guest-signals: fork: {}",
            std::io::Error::last_os_error()
        );
        return 1;
    }
    if pid > 0 {
        let mut o = std::io::stdout().lock();
        return match writeln!(o, "{}", up_marker(nonce)).and_then(|()| o.flush()) {
            Ok(()) => 0,
            Err(_) => 1,
        };
    }
    let mut line = Vec::with_capacity(256);
    let mut buf = [0u8; 256];
    while let Ok(n) = k::read(0, &mut buf) {
        if n <= 0 {
            break;
        }
        for &b in &buf[..n as usize] {
            if b != b'\n' && b != b'\r' {
                // A line longer than a frame is noise: it is dropped.
                if line.len() < 256 {
                    line.push(b);
                }
                continue;
            }
            if let Some(signo) = parse(&String::from_utf8_lossy(&line), nonce) {
                deliver(signo);
            }
            line.clear();
        }
    }
    k::exit_group(0)
}

/// Send `signo` to the command, whose pid comes in [`PID_FILE`] when it
/// starts: wait for it for 5 s at most.
fn deliver(signo: i32) {
    for _ in 0..100 {
        let pid = std::fs::read_to_string(PID_FILE)
            .ok()
            .and_then(|p| p.trim().parse::<i64>().ok())
            .filter(|p| *p > 1);
        if let Some(pid) = pid {
            let _ = k::kill(pid, i64::from(signo));
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-2041: one table of the signals that go to the guest. It is the set
    /// of the waiter of pod-enter; each frame goes through `parse` to its
    /// signal, and a line of another run, a signal out of the set, or noise
    /// gives none.
    #[test]
    fn each_forwarded_signal_reaches_the_guest_and_nothing_else_does() {
        let mask: u64 = SET.iter().map(|(s, _)| 1u64 << (s - 1)).sum();
        assert_eq!(mask, podvm_enter::WAIT_FORWARDED, "the set of pod-enter");
        let nonce = "0123abcd";
        for (signo, n) in SET {
            assert_eq!(parse(&frame(nonce, signo), nonce), Some(signo), "{n}");
            assert_eq!(parse(&frame("other", signo), nonce), None, "{n}");
        }
        for noise in [
            format!("{SIG}{nonce} 9"),
            format!("{SIG}{nonce} 0"),
            format!("{SIG}{nonce} -15"),
            format!("{SIG}{nonce}15"),
            format!("{SIG}{nonce} 15 x"),
            format!("{SIG}{nonce}x 15"),
            "SIGTERM".to_string(),
            String::new(),
        ] {
            assert_eq!(parse(&noise, nonce), None, "{noise:?}");
        }
        let mut f = Forward {
            sfd: None,
            up: false,
            nonce: nonce.into(),
            verb: "run".into(),
        };
        assert!(!f.line("PODVM-READY-0123abcd") && !f.up);
        assert!(f.line(&format!("{}\r", up_marker(nonce))) && f.up);
        assert!(wrap("\"$@\"").ends_with("' /podvm-init \"$@\""));
        assert!(start(nonce).contains("guest-signals 0123abcd"));
    }
}
