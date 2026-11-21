//! The interrupt and quit keys of a pty with no session (OQ-05 of the
//! sixth outside pass, TODO/cli.md T-2052).
//!
//! The slave of the pty is not the controlling terminal of the payload:
//! `pod-enter` makes no session (COORD-188 asks for one). So the line
//! discipline signals no process for `VINTR` or `VQUIT`, and podvm takes
//! the key out of the input of `-i` and sends the signal itself
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
//! payload (the payload and its descendants) that is in the process group
//! of the payload, as a terminal sends it to that group. A process of
//! podvm in that tree (the relay of `exec` that stays in a PID namespace)
//! gets none: it would forward the signal, and the payload would take it
//! twice. A process that left the group (`setsid`, a daemon) gets none, as
//! from a terminal. Where `/proc` cannot give the tree, the payload alone
//! gets the signal, as before.

use std::collections::BTreeMap;
use std::ffi::c_int;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

const SIGINT: c_int = 2;
const SIGQUIT: c_int = 3;

/// The keys of a terminal that send a signal, by the index of the key in
/// `c_cc` of `struct termios` (`VINTR` 0, `VQUIT` 1), and the signal. The
/// suspend key (`VSUSP`) is not in the table: with no job control a stop
/// of the payload stops the run, so its byte goes to the payload.
pub const KEYS: &[(usize, c_int)] = &[(0, SIGINT), (1, SIGQUIT)];

/// The most entries of `/proc` that one key reads: a bound of the work of
/// one key press on a host with very many processes.
const MOST_PROCS: usize = 1 << 16;

/// One process as `/proc` gives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Proc {
    pub pid: i64,
    /// The process group, field 5 of `/proc/PID/stat`.
    pub pgrp: i64,
    /// True where the program of the process is podvm itself.
    pub podvm: bool,
}

/// The process group in the text of `/proc/PID/stat`: the fields start
/// after the last `)`, as the name of the command can hold one.
pub fn pgrp_of(stat: &str) -> Option<i64> {
    let rest = &stat[stat.rfind(')')? + 1..];
    let mut fields = rest.split_whitespace();
    fields.next()?; // the state
    fields.next()?; // the parent
    fields.next()?.parse().ok()
}

/// The processes that a key signals: each of `tree` (the payload first,
/// then its descendants) in the process group of the payload, and not
/// podvm. Where the payload is not in `tree` (it ended, or `/proc` did not
/// give it) or no process stays, the payload alone.
pub fn targets(payload: i64, tree: &[Proc]) -> Vec<i64> {
    let Some(head) = tree.iter().find(|p| p.pid == payload) else {
        return vec![payload];
    };
    let out: Vec<i64> = tree
        .iter()
        .filter(|p| p.pgrp == head.pgrp && !p.podvm)
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
    let mut pids = vec![payload];
    pids.extend(podvm_supervise::tree::descendants(proc, payload)?);
    pids.truncate(MOST_PROCS);
    let mut seen = BTreeMap::new();
    for pid in pids {
        let dir = proc.join(pid.to_string());
        let Some(pgrp) = std::fs::read_to_string(dir.join("stat"))
            .ok()
            .as_deref()
            .and_then(pgrp_of)
        else {
            continue;
        };
        // A program that cannot be read is not podvm's: the signal goes to
        // it, as to each process of the payload.
        let podvm = me.is_some() && program(&dir.join("exe")) == me;
        seen.insert(pid, Proc { pid, pgrp, podvm });
    }
    Some(seen.into_values().collect())
}

/// A second procfs for [`signal`]: the `/proc` of the root of a container
/// whose PID namespace `exec` joined. Its own `/proc` names the pids of the
/// host there, and the pid of the payload is a pid of the container.
pub static PROC: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// The first procfs of `candidates` whose `self` names this process by
/// the pid that `getpid` gives: the one whose pids are the pids of the
/// payload.
fn own_proc(candidates: &[&Path]) -> Option<PathBuf> {
    let me = std::process::id().to_string();
    candidates
        .iter()
        .find(|p| std::fs::read_link(p.join("self")).is_ok_and(|l| l.as_os_str() == me.as_str()))
        .map(|p| p.to_path_buf())
}

/// Send `sig` to the processes of [`targets`] for the payload `payload`.
pub fn signal(payload: i64, sig: c_int) {
    unsafe extern "C" {
        fn kill(pid: c_int, sig: c_int) -> c_int;
    }
    let mut candidates = vec![Path::new("/proc")];
    candidates.extend(PROC.get().map(PathBuf::as_path));
    let tree = own_proc(&candidates)
        .and_then(|proc| tree_of(&proc, payload))
        .unwrap_or_default();
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
        Proc { pid, pgrp, podvm }
    }

    /// One rule for each shape of a tree: the members of the group of the
    /// payload that are not podvm, else the payload alone.
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
    fn the_group_is_read_after_the_last_parenthesis() {
        let cases: &[(&str, Option<i64>)] = &[
            ("104 (sh) S 2 104 1 0", Some(104)),
            ("105 (a b) c)) S 104 77 1 0", Some(77)),
            ("105 (sleep) S 104", None),
            ("no parenthesis", None),
            ("105 (x) S 104 notanumber", None),
        ];
        for (stat, want) in cases {
            assert_eq!(pgrp_of(stat), *want, "{stat}");
        }
    }

    #[test]
    fn the_procfs_of_this_process_is_the_one_whose_self_is_its_pid() {
        let none = Path::new("/nonexistent-proc");
        let proc = Path::new("/proc");
        assert_eq!(own_proc(&[none, proc]).as_deref(), Some(proc));
        assert_eq!(own_proc(&[none]), None);
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
