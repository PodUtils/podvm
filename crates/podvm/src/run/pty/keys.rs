//! The interrupt and quit keys of the pty of `-it` (OQ-05 of the sixth
//! outside pass, TODO/cli.md T-2052).
//!
//! `pod-enter` makes no session (COORD-188 asks for one). Where podvm left
//! the caller's session, the pty is the controlling terminal of podvm's
//! session (`super::session`, T-2070), and the foreground group
//! of that session can be podvm's own: a SIGINT of the line discipline
//! would reach podvm, whose waiter forwards it, and the payload would take
//! it twice. Elsewhere the pty is the controlling terminal of no process,
//! and the line discipline signals none. So podvm takes `VINTR` and
//! `VQUIT` out of the input of `-i` and sends the signal itself
//! ([`super::send`]).
//!
//! A terminal sends the signal to its foreground process group, not to one
//! process. Before this module podvm signalled the payload alone: `sh -c
//! 'sleep 6'` took the SIGINT only after `sleep` ended, as a shell runs its
//! trap after the foreground command. With `-t` and no `-i` the terminal of
//! the caller stays cooked, and its key reached the whole process group of
//! podvm, `sleep` too: so `-i` changed the result.
//!
//! The rule now: the signal goes to each process of the tree of the
//! payload (the payload and its descendants) that is in the foreground
//! group of the terminal of the payload (`tpgid`), as a terminal sends it
//! to that group. A job-control shell moves its foreground job to a group
//! of its own (T-2070): the job takes the signal, and the shell and its
//! background jobs do not. Where the payload has no controlling terminal,
//! or its foreground group is outside the PID namespace of the `/proc`
//! that podvm reads, the group is the group of the payload. A process of
//! podvm in that tree (the relay of `exec` that stays in a PID namespace)
//! gets none: it would forward the signal, and the payload would take it
//! twice. A process that left the group (`setsid`, a daemon) gets none, as
//! from a terminal. Where `/proc` cannot give the tree, the payload alone
//! gets the signal, as before.

use std::ffi::c_int;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

const SIGINT: c_int = 2;
const SIGQUIT: c_int = 3;

/// The keys of a terminal that send a signal, by the index of the key in
/// `c_cc` of `struct termios` (`VINTR` 0, `VQUIT` 1), and the signal. The
/// suspend key (`VSUSP`) is not in the table: with no job control a stop
/// of the payload stops the run, so its byte goes to the payload.
pub const KEYS: [(usize, c_int); 2] = [(0, SIGINT), (1, SIGQUIT)];

/// The most entries of `/proc` that one key reads: a bound of the work of
/// one key press on a host with very many processes.
const MOST_PROCS: usize = 1 << 16;

/// One process as `/proc` gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Proc {
    pub pid: i64,
    /// The process group, field 5 of `/proc/PID/stat`.
    pub pgrp: i64,
    /// The foreground group of the controlling terminal, field 8: -1 with
    /// no terminal, 0 for a group outside the PID namespace of `/proc`.
    pub tpgid: i64,
    /// True where the program of the process is podvm itself.
    pub podvm: bool,
}

/// The process group and the foreground group of the terminal in the text
/// of `/proc/PID/stat`: the fields start after the last `)`, as the name
/// of the command can hold one.
pub fn groups_of(stat: &str) -> Option<(i64, i64)> {
    let rest = &stat[stat.rfind(')')? + 1..];
    let f: Vec<&str> = rest.split_whitespace().take(6).collect();
    // The state, the parent, the group, the session, the terminal, tpgid.
    let [_, _, pgrp, _, _, tpgid] = f[..] else {
        return None;
    };
    Some((pgrp.parse().ok()?, tpgid.parse().ok()?))
}

/// The processes that a key signals: each of `tree` (the payload first,
/// then its descendants) in the foreground group of the terminal of the
/// payload (else in its process group), and not podvm. Where the payload
/// is not in `tree` (it ended, or `/proc` did not give it) or no process
/// stays, the payload alone.
pub fn targets(payload: i64, tree: &[Proc]) -> Vec<i64> {
    let Some(head) = tree.iter().find(|p| p.pid == payload) else {
        return vec![payload];
    };
    let group = if head.tpgid > 0 {
        head.tpgid
    } else {
        head.pgrp
    };
    let out: Vec<i64> = tree
        .iter()
        .filter(|p| p.pgrp == group && !p.podvm)
        .map(|p| p.pid)
        .collect();
    if out.is_empty() {
        vec![payload]
    } else {
        out
    }
}

/// The device and inode of a program, through the `exe` link of `/proc`.
fn program(path: &Path) -> Option<(u64, u64)> {
    std::fs::metadata(path).ok().map(|m| (m.dev(), m.ino()))
}

/// The payload and each of its descendants, from `proc`, with the group
/// and the program of each. A process that ends between two reads is left
/// out. `None` where `proc` gives no tree.
fn tree_of(proc: &Path, payload: i64) -> Option<Vec<Proc>> {
    let me = program(&proc.join("self/exe"));
    let mut pids = podvm_supervise::tree::descendants(proc, payload)?;
    // `descendants` names each process once, and not the payload.
    pids.truncate(MOST_PROCS);
    pids.push(payload);
    let one = |pid: i64| {
        let dir = proc.join(pid.to_string());
        let (pgrp, tpgid) = groups_of(&std::fs::read_to_string(dir.join("stat")).ok()?)?;
        // A program that cannot be read is not podvm's: the signal goes to
        // it, as to each process of the payload.
        let podvm = me.is_some() && program(&dir.join("exe")) == me;
        Some(Proc {
            pid,
            pgrp,
            tpgid,
            podvm,
        })
    };
    Some(pids.into_iter().filter_map(one).collect())
}

/// A second procfs for [`signal`]: the `/proc` of the root of a container
/// whose PID namespace `exec` joined. Its own `/proc` names the pids of the
/// host there, and the pid of the payload is a pid of the container.
pub static PROC: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Send `sig` to the processes of [`targets`] for the payload `payload`.
pub fn signal(payload: i64, sig: c_int) {
    unsafe extern "C" {
        fn kill(pid: c_int, sig: c_int) -> c_int;
    }
    let proc = PROC.get().map_or(Path::new("/proc"), PathBuf::as_path);
    let tree = tree_of(proc, payload).unwrap_or_default();
    for pid in targets(payload, &tree) {
        if let Ok(p) = c_int::try_from(pid)
            && p > 0
        {
            // SAFETY: a signal to a process of the payload of this run.
            unsafe { kill(p, sig) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: i64, pgrp: i64, podvm: bool) -> Proc {
        Proc {
            pid,
            pgrp,
            tpgid: -1,
            podvm,
        }
    }

    /// `p` with a controlling terminal whose foreground group is `tpgid`.
    fn t(pid: i64, pgrp: i64, tpgid: i64) -> Proc {
        Proc {
            tpgid,
            ..p(pid, pgrp, false)
        }
    }

    /// One rule for each shape of a tree: the members of the foreground
    /// group of the terminal of the payload (else of its group) that are
    /// not podvm, else the payload alone.
    #[test]
    fn a_key_signals_the_group_of_the_payload_in_its_tree() {
        let cases: &[(&str, i64, &[Proc], &[i64])] = &[
            (
                "sh -c 'sleep 6': the shell and its child",
                104,
                &[p(104, 7, false), p(105, 7, false)],
                &[104, 105],
            ),
            (
                "a daemon of the payload left the group",
                104,
                &[p(104, 7, false), p(105, 7, false), p(106, 106, false)],
                &[104, 105],
            ),
            (
                "the relay of exec is podvm: its child takes the signal once",
                30,
                &[p(30, 7, true), p(31, 7, false), p(32, 7, false)],
                &[31, 32],
            ),
            (
                "the payload is not in the tree",
                104,
                &[p(105, 7, false)],
                &[104],
            ),
            ("no tree from /proc", 104, &[], &[104]),
            (
                "T-2070: a job-control shell at its prompt: the shell",
                104,
                &[t(104, 104, 104), t(120, 120, 104)],
                &[104],
            ),
            (
                "T-2070: the foreground job of the shell, not its background job",
                104,
                &[
                    t(104, 104, 110),
                    t(110, 110, 110),
                    t(111, 110, 110),
                    t(120, 120, 110),
                ],
                &[110, 111],
            ),
            (
                "T-2070: the relay of exec reads the group of the job",
                30,
                &[
                    Proc {
                        tpgid: 40,
                        ..p(30, 7, true)
                    },
                    t(31, 31, 40),
                    t(40, 40, 40),
                ],
                &[40],
            ),
            (
                "the foreground group is podvm's: its members but podvm",
                30,
                &[
                    Proc {
                        tpgid: 7,
                        ..p(30, 7, true)
                    },
                    t(31, 7, 7),
                ],
                &[31],
            ),
            (
                "the foreground group is outside the namespace (0)",
                104,
                &[t(104, 7, 0), t(105, 7, 0)],
                &[104, 105],
            ),
            (
                "only podvm in the group",
                30,
                &[p(30, 7, true), p(31, 9, false)],
                &[30],
            ),
        ];
        for (what, payload, tree, want) in cases {
            assert_eq!(&targets(*payload, tree), want, "{what}");
        }
    }

    #[test]
    fn the_groups_are_read_after_the_last_parenthesis() {
        let cases: &[(&str, Option<(i64, i64)>)] = &[
            ("104 (sh) S 2 104 1 0 -1 4194304", Some((104, -1))),
            ("104 (sh) S 2 104 104 34816 110 0", Some((104, 110))),
            ("105 (a b) c)) S 104 77 1 0 0", Some((77, 0))),
            ("105 (sleep) S 104 104 1 0", None),
            ("no parenthesis", None),
            ("105 (x) S 104 notanumber 1 0 -1", None),
        ];
        for (stat, want) in cases {
            assert_eq!(groups_of(stat), *want, "{stat}");
        }
    }

    /// The tree of this test process, as `/proc` gives it: a child in the
    /// group of the test is a target, and the test itself is not podvm.
    #[test]
    fn the_tree_of_a_live_process_names_its_child() {
        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .expect("sleep starts");
        let me = i64::from(std::process::id());
        let tree = tree_of(Path::new("/proc"), me).expect("/proc gives a tree");
        let kid = i64::from(child.id());
        assert!(
            tree.iter().any(|p| p.pid == kid),
            "the child is in the tree"
        );
        assert!(targets(me, &tree).contains(&kid));
        let _ = child.kill();
        let _ = child.wait();
    }
}
