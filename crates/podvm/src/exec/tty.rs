//! `exec -t`: a pty for the command, as `run -t` gives one
//! (TODO/cli.md T-2043, OQ-04 of the sixth outside pass).
//!
//! Before, `exec` parsed `-t` and opened no pty: `exec -t NAME tty` printed
//! `not a tty`. Now each rung of `exec` that `run -t` serves (the chroot,
//! the namespace rung and its join, the userland families) gets the slave
//! of a host pty as its descriptors 0, 1 and 2, through the relay of
//! `run -t` ([`crate::run::pty`]). Where no pty can serve, `exec -t`
//! refuses by name ([`refusal`]): `/dev/ptmx` is unusable, or the target
//! is a guest of the machine tier or an instance of the incus tier, whose
//! exec channel carries no terminal.

use std::io::Write;
use std::os::fd::OwnedFd;

use crate::run::pty::{Pty, Relay};

/// Where `exec -t` gives a pty, and where it refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    /// A container or an image of the store, entered on this host.
    Host,
    /// A detached guest of the machine tier (Linux or Windows): its exec
    /// channel carries frames of stdout and stderr, not a terminal.
    Machine,
    /// An instance of the incus tier: its job carries no terminal.
    Incus,
}

/// The refusal of `exec -t`, or `None` where a pty serves. `ptmx` is the
/// shared predicate of the probe (`podvm_probe::probes::ptmx_usable`).
pub(crate) fn refusal(target: Target, ptmx: bool) -> Option<&'static str> {
    match target {
        Target::Host if ptmx => None,
        Target::Host => Some(super::TTY_REFUSAL),
        Target::Machine => Some(
            "podvm exec: -t: the exec channel of a guest of the machine tier carries no \
             terminal, so podvm refuses rather than running the command without one; \
             run -t --podvm-tier=machine gives one (TODO/cli.md T-2043)",
        ),
        Target::Incus => Some(
            "podvm exec: -t: an instance of the incus tier gets no terminal from podvm, \
             so podvm refuses rather than running the command without one \
             (TODO/cli.md T-2043)",
        ),
    }
}

/// An open pty of one `exec -t`, before the entry.
pub(crate) struct Term {
    pty: Pty,
    slaves: [OwnedFd; 3],
}

/// Open the pty of `exec -t` where `want`; `Ok(None)` without `-t`. A
/// refusal or a pty that does not open prints one line and gives the
/// exit code.
/// `joined` is the payload of a running container that the command joins:
/// the pty is then one of its `/dev/pts`, else one of the host's, where the
/// name of the slave does not resolve in the container but the terminal
/// works.
pub(crate) fn open(
    want: bool,
    ptmx: bool,
    joined: Option<i64>,
    err: &mut dyn Write,
) -> Result<Option<Term>, i32> {
    if !want {
        return Ok(None);
    }
    if let Some(text) = refusal(Target::Host, ptmx) {
        writeln!(err, "{text}").unwrap_or(());
        return Err(podvm_enter::EXIT_RUNTIME_ERROR);
    }
    let inner = joined.and_then(|p| Pty::open_in(&format!("/proc/{p}/root/dev")).ok());
    match inner
        .map_or_else(Pty::open, Ok)
        .and_then(|pty| Ok((pty.slaves()?, pty)))
    {
        Ok((slaves, pty)) => Ok(Some(Term { pty, slaves })),
        Err(e) => {
            writeln!(
                err,
                "podvm exec: -t: podvm could not open a pty: {e} (TODO/cli.md T-2043)"
            )
            .unwrap_or(());
            Err(podvm_enter::EXIT_RUNTIME_ERROR)
        }
    }
}

impl Term {
    /// The slave as the command's 0, 1 and 2, for `Fds::pass`.
    pub(crate) fn pass(&self) -> [(i64, i64); 3] {
        let fd = |i: usize| crate::run::pty::raw(&self.slaves[i]);
        [(0, fd(0)), (1, fd(1)), (2, fd(2))]
    }

    /// Start the relay of `run -t`; stdin goes to the pty only with `-i`.
    pub(crate) fn start(self, stdin: bool) -> Running {
        Running {
            relay: self.pty.relay(stdin),
            slaves: self.slaves,
        }
    }
}

/// The relay of a running `exec -t`.
pub(crate) struct Running {
    relay: std::io::Result<Relay>,
    slaves: [OwnedFd; 3],
}

impl Running {
    /// After the command exited: close the slaves and drain the output.
    pub(crate) fn finish(self, err: &mut dyn Write) {
        drop(self.slaves);
        match self.relay {
            Ok(r) => r.finish(),
            Err(e) => {
                writeln!(err, "podvm exec: -t: the relay of the pty: {e}").unwrap_or(());
            }
        }
    }
}

/// Start the relay where a pty is open; the target of the interrupt key
/// is set at the spawn ([`on_spawn`]).
pub(crate) fn start(term: Option<Term>, stdin: bool) -> Option<Running> {
    term.map(|t| t.start(stdin))
}

/// The hook of the spawn: the interrupt key of the pty goes to `pid`.
pub(crate) fn on_spawn(pid: i64) {
    crate::run::pty::set_payload(pid);
}

/// TODO/cli.md T-2052: the interrupt key reads the processes of the
/// payload in the `/proc` of `rootfs` where this process joined the PID
/// namespace of a run (T-1910): its own `/proc` names the pids of the host,
/// and the pid of the payload is a pid of the run, as in that `/proc`.
pub(crate) fn proc_of(rootfs: &str) {
    if crate::userns::joined() {
        crate::run::pty::set_proc(std::path::Path::new(rootfs).join("proc"));
    }
}

/// Finish the relay where one runs.
pub(crate) fn finish(running: Option<Running>, err: &mut dyn Write) {
    if let Some(r) = running {
        r.finish(err);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-2043: one rule for each target: a pty on the host where
    /// /dev/ptmx serves, a refusal by name in each other case, and never
    /// a run with no terminal.
    #[test]
    fn exec_t_gives_a_pty_or_refuses_by_name() {
        let cases = [
            (Target::Host, true, None),
            (Target::Host, false, Some("/dev/ptmx is not usable")),
            (Target::Machine, true, Some("machine tier")),
            (Target::Machine, false, Some("machine tier")),
            (Target::Incus, true, Some("incus tier")),
            (Target::Incus, false, Some("incus tier")),
        ];
        for (target, ptmx, want) in cases {
            let got = refusal(target, ptmx);
            match want {
                None => assert_eq!(got, None, "{target:?} ptmx={ptmx}"),
                Some(w) => {
                    let text = got.unwrap_or_else(|| panic!("{target:?} ptmx={ptmx}: no refusal"));
                    assert!(text.contains(w), "{target:?}: {text}");
                    assert!(text.starts_with("podvm exec: "), "{text}");
                }
            }
        }
        let mut err = Vec::new();
        assert_eq!(
            open(true, false, None, &mut err).err(),
            Some(podvm_enter::EXIT_RUNTIME_ERROR)
        );
        assert!(String::from_utf8_lossy(&err).contains("/dev/ptmx"));
        assert!(matches!(
            open(false, false, None, &mut Vec::new()),
            Ok(None)
        ));
    }
}
