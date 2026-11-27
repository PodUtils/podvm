//! The session of the pty of `-it` (TODO/cli.md T-2070, OR-B38-17 and
//! OR-B40-13).
//!
//! Docker makes the pty the controlling terminal of the payload. podvm
//! makes it the controlling terminal of a new session of its own, and the
//! payload is a member of that session (`pod-enter` makes no session of
//! the payload: COORD-188, podcrates@T-927). A process leaves its session
//! only where it does not lead its process group (`setsid` gives `EPERM`).
//! podvm leads its group where a shell started it (each job of a shell is
//! a group of its own), and where a terminal emulator started it. Only the
//! process of podvm in a user namespace of its own (pid 2, T-1909) was not
//! a leader, so the pty was the terminal of the payload only there: the
//! userland rung of a host with no user namespace (a GitHub runner) and
//! each run of root kept the caller's terminal as the terminal of the
//! payload, and an interactive shell said "can't access tty; job control
//! turned off".
//!
//! [`split_if`]: first in `main`, with one thread, where the verb is
//! `run` or `exec` with `-t` and `-i` and this process leads its group,
//! podvm forks. The parent stays in the caller's session, relays each
//! signal of a process to the child and exits with its code; the child
//! leads a new session with no terminal and runs the verb. Its relay of
//! the pty then takes the pty as its terminal ([`leave_the_session`]).

use std::ffi::c_int;
use std::sync::atomic::{AtomicBool, Ordering};

/// True in the child of [`split_if`]: it leads a session with no terminal.
static LEFT: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn setsid() -> c_int;
    fn getpid() -> c_int;
    fn getpgrp() -> c_int;
    fn getppid() -> c_int;
    fn fork() -> c_int;
    fn ioctl(fd: c_int, req: std::ffi::c_ulong, ...) -> c_int;
}

const TIOCSCTTY: std::ffi::c_ulong = 0x540E;

/// True where the words of `verb` ask for `-t` and `-i`: the flags before
/// the image or the container, as the parser reads them (the clusters
/// expanded, the value of a flag skipped). A word that the expansion
/// refuses gives false: the parser then refuses it by name.
pub fn tty_and_stdin(verb: &str, args: &[String]) -> bool {
    use crate::parity::{flag, BOOLEAN_LONGS, CLUSTER_VALUES};
    if !matches!(verb, "run" | "exec") {
        return false;
    }
    let Ok(words) = crate::parity::expand(verb, args) else {
        return false;
    };
    let (mut tty, mut stdin, mut value_next) = (false, false, false);
    for w in &words {
        if std::mem::take(&mut value_next) {
            continue;
        }
        match w.as_str() {
            "--" => break,
            "-t" | "--tty" => tty = true,
            "-i" | "--interactive" => stdin = true,
            long if long.starts_with("--") => {
                value_next = !long.contains('=')
                    && !BOOLEAN_LONGS.contains(&long)
                    && flag(verb, long).is_some();
            }
            short if short.len() == 2 && short.starts_with('-') => {
                let c = short.as_bytes()[1] as char;
                value_next = CLUSTER_VALUES.contains(&(verb, c));
            }
            _ => break, // the image or the container: the payload's words
        }
    }
    tty && stdin
}

/// Fork where [`tty_and_stdin`] holds and this process leads its process
/// group; return in the child, which leads a new session. The parent
/// relays the signals of a process to the child and exits with its code.
/// A refused fork or `setsid` changes nothing: the run goes on as before.
///
/// ⛔ Call it first in `main`, with one thread, after the entry of the user
/// namespace (`userns::enter_if`): pid 2 of that entry leads no group.
pub fn split_if(argv: &[String]) {
    let verb = argv.get(1).map_or("", String::as_str);
    let args = argv.get(2..).unwrap_or(&[]);
    // A join of the namespaces of a run (`exec`, T-1910) holds the PID
    // namespace of the run for the children of this process: a child here
    // would be a process of the run. Its relay leaves the session itself.
    if crate::userns::joined() || !tty_and_stdin(verb, args) {
        return;
    }
    // SAFETY: getpid and getpgrp cannot fail.
    if unsafe { getpgrp() != getpid() } {
        return; // `setsid` works in this process (`leave_the_session`)
    }
    // SAFETY: one thread (the contract of this function).
    let parent = unsafe { getpid() };
    let child = unsafe { fork() };
    if child < 0 {
        return;
    }
    if child > 0 {
        std::process::exit(crate::userns::sigrelay::wait_relaying(child));
    }
    // The child: it ends with the parent (a SIGKILL of a caller's bound),
    // as the payload ends with it (T-1748).
    crate::userns::sigrelay::die_with_outer(true);
    // SAFETY: getppid cannot fail.
    if unsafe { getppid() } != parent {
        std::process::exit(125);
    }
    // SAFETY: setsid takes no argument; this child leads no group.
    if unsafe { setsid() } >= 0 {
        LEFT.store(true, Ordering::Relaxed);
    }
}

/// True where the pty may be the controlling terminal of the payload:
/// `outside` is false. `outside` is true where the payload enters a PID
/// namespace that this process is not a member of (the namespace rung of
/// root: the PID namespace of `pod-enter`, its own run or a join of a
/// `run -d`). There the first process group of the payload is podvm's,
/// which the payload cannot name: a job-control shell gave the terminal
/// back to that group at its end, got `ESRCH`, said "Cannot set tty
/// process group (No such process)" and exited with 2 for `exit 0`
/// (measured as root, 2026-11-27). There the payload gets no terminal
/// and the shell turns job control off, until `pod-enter` gives the
/// payload a session of its own (COORD-188, podcrates@T-927).
pub fn ctty_holds(outside: bool) -> bool {
    !outside
}

/// OR-B38-01 of the seventh outside pass (TODO/cli.md T-2071): where podvm
/// relays its stdin to the pty, the payload must not get the caller's
/// terminal. Before, the payload was a member of podvm's session, whose
/// controlling terminal is the caller's. In a joined run of a user
/// namespace, `/dev/tty` of the container is the host's, so a job-control
/// shell (`sh`, `sh -i`, `set -m`) opened it, made its own process group
/// the foreground group of the caller's terminal, and the relay, now in a
/// background group, got no more input after the first chunk. This
/// process leaves the session before the spawn (`setsid`), or left it in
/// [`split_if`], so its payload does not get the caller's terminal.
/// T-2070: the new session then takes the slave `slave` as its
/// controlling terminal (`TIOCSCTTY`), so the payload, a member of the
/// session, has the pty as its terminal: its `/dev/tty` is the pty, and a
/// job-control shell makes its group the foreground group. The leader of
/// the session is podvm, the parent of the payload; in docker the payload
/// leads its own session, which needs the fork of `pod-enter`.
/// ⚠ Only with `-i`: the caller's terminal is then raw and sends no
/// signal. With `-t` and no `-i` it stays cooked, and its interrupt key
/// must reach the process group of podvm and the payload (OR-03).
pub(super) fn leave_the_session(slave: c_int, ctty: bool) {
    // SAFETY: setsid takes no argument; a refusal changes nothing.
    if !LEFT.load(Ordering::Relaxed) && unsafe { setsid() } < 0 {
        // This process leads its group and `split_if` did not fork: a
        // word that the scan of the flags did not read as `-t` and `-i`.
        return;
    }
    if !ctty {
        return; // `ctty_holds`: the payload is better with no terminal
    }
    // SAFETY: an open descriptor of the slave; the argument 0 takes no
    // terminal of another session.
    if unsafe { ioctl(slave, TIOCSCTTY, 0) } != 0 {
        let e = std::io::Error::last_os_error();
        // The caller's terminal is raw: a line ends with CR LF.
        eprint!(
            "podvm: -t: the pty is not the controlling terminal of the payload ({e}); \
             a job-control shell turns job control off (TODO/cli.md T-2070)\r\n"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::tty_and_stdin;

    /// One rule for each spelling: `-t` and `-i` before the image or the
    /// container, in any form that the parser reads; a word of the payload
    /// and the value of a flag do not count.
    #[test]
    fn the_scan_reads_tty_and_stdin_as_the_parser_does() {
        let cases: &[(&str, &[&str], bool)] = &[
            ("run", &["-it", "img", "sh"], true),
            ("run", &["-ti", "img"], true),
            ("run", &["--rm", "-i", "-t", "img"], true),
            ("run", &["--tty", "--interactive", "img"], true),
            ("run", &["--tty=true", "-i", "img"], true),
            ("run", &["-e", "X=1", "-it", "img"], true),
            ("run", &["--name", "n", "-it", "img"], true),
            ("exec", &["-it", "ctr", "sh"], true),
            ("exec", &["-u", "0", "-it", "ctr", "sh"], true),
            ("run", &["-t", "img", "sh"], false),
            ("run", &["-i", "img", "sh"], false),
            ("run", &["--tty=false", "-i", "img"], false),
            ("run", &["img", "ls", "-it"], false),
            ("run", &["-e", "-it", "img"], false),
            ("run", &["--", "img", "-it"], false),
            ("exec", &["ctr", "sh", "-it"], false),
            ("run", &["-it?", "img"], false),
            ("ps", &["-it"], false),
            ("create", &["-it", "img"], false),
        ];
        for (verb, args, want) in cases {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            assert_eq!(tty_and_stdin(verb, &args), *want, "{verb} {args:?}");
        }
    }
}
