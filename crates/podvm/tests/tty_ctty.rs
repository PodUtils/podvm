//! TODO/cli.md T-2070 (OR-B38-17) and T-2087 (OR-B40-13): the pty of
//! `run -it` and `exec -it` is the controlling terminal of the payload, as
//! docker makes it (`setsid`, then `TIOCSCTTY`), and the process group of
//! the shell is its foreground group. Before, a job-control shell printed
//! "can't access tty; job control turned off".
//!
//! The payload reads its own `/proc/self/stat`: the terminal (`tty_nr`)
//! is not 0, the foreground group of that terminal (`tpgid`) is the group
//! of the shell, and the leader of its session is the shell or its parent.
//! The keys of the terminal then reach the foreground job of the shell:
//! Ctrl-C ends `sleep 30` and the shell goes on, and Ctrl-Z stops it as a
//! job of the shell. The image is the one of the host's programs
//! (`host_image`).
//!
//! ⚠ podvm makes the pty the terminal of a new session of its own, where
//! it can leave the caller's session (`run/pty.rs`, `leave_the_session`):
//! as uid 1000, podvm runs in a user namespace as a child, and it can. As
//! root, podvm leads its group (here its session), so the payload of root
//! gets the pty as its terminal only from the fork of `pod-enter`
//! (COORD-188, podcrates@T-927). The test skips as root with a line.

mod host_image;
mod tty_session;
use host_image::{podvm, text};
use tty_session::{pty, session, Steps};

const IMAGE: &str = "localhost/t2070:1";

/// The line that the shell types: its own stat and the one of its parent
/// (none where the parent is outside its PID namespace: `$PPID` is 0).
const PROBE: &[u8] = b"read s < /proc/self/stat; echo \"S=$s\"; \
    p=; [ \"$PPID\" != 0 ] && read p < /proc/$PPID/stat; echo \"P=$p\"; echo E$((40+2))N\n";

/// The fields of one line of `/proc/PID/stat` that the rule reads.
#[derive(Debug)]
struct Stat {
    pid: i64,
    ppid: i64,
    pgrp: i64,
    session: i64,
    tty_nr: i64,
    tpgid: i64,
}

/// The stat after `tag` in `out`, from the line that the shell printed
/// (not the echo of the typed line, which holds `$s`).
fn stat_of(out: &str, tag: &str) -> Option<Stat> {
    let line = out
        .split(['\r', '\n'])
        .filter_map(|l| l.split_once(tag).map(|(_, rest)| rest))
        .find(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))?;
    let (pid, rest) = line.split_once(" (")?;
    let f: Vec<i64> = rest
        .rsplit_once(") ")?
        .1
        .split_whitespace()
        .skip(1)
        .take(5)
        .map(|x| x.parse().ok())
        .collect::<Option<_>>()?;
    Some(Stat {
        pid: pid.trim().parse().ok()?,
        ppid: f[0],
        pgrp: f[1],
        session: f[2],
        tty_nr: f[3],
        tpgid: f[4],
    })
}

/// One rule for each path of `-it`: the controlling terminal, the
/// foreground group, the leader of the session, and no line of a shell
/// that has no terminal.
#[test]
fn the_pty_of_run_it_and_exec_it_is_the_controlling_terminal() {
    let Some(tar) = host_image::layout(IMAGE, &["sleep"]) else {
        eprintln!("T-2070: this host has no glibc or dash at the x86_64 paths; skipped");
        return;
    };
    if pty().is_none() {
        eprintln!("T-2070: /dev/ptmx does not open here; skipped");
        return;
    }
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid takes nothing and cannot fail.
    if unsafe { geteuid() } == 0 {
        eprintln!("T-2070: as root the payload has the pty as its terminal only with podcrates@T-927; skipped");
        return;
    }
    let (dir, store) = host_image::store_with("t2070", tar);
    let d = podvm(
        &store,
        &["run", "-d", "--name", "t2070", IMAGE, "sleep", "300"],
    );
    assert!(d.status.success(), "run -d: {}", text(&d.stderr));
    // `sleep 30` outlasts the wait of a step (20 s): only the key ends it.
    let steps: Steps = &[
        (PROBE, "E42N", 1),
        (b"sleep 30\n", "sleep 30", 1),
        (b"\x03", "", 0),
        (b"echo C$((40+3))\n", "C43", 1),
        (b"sleep 30\n", "sleep 30", 2),
        (b"\x1a", "Stopped", 1),
        (b"kill -9 %1; sleep 1; jobs; echo Z$((40+4))\n", "Z44", 1),
        (b"exit 0\n", "exit 0", 1),
    ];
    let cases: &[&[&str]] = &[
        &["run", "--rm", "-it", IMAGE, "sh", "-i"],
        &["exec", "-it", "t2070", "sh", "-i"],
    ];
    let mut failures = Vec::new();
    for args in cases {
        let out = match session(&store, args, steps) {
            Ok((0, out)) => out,
            Ok((code, out)) => {
                failures.push(format!("{args:?}: exit {code}; output: {out:?}"));
                continue;
            }
            Err(e) => {
                failures.push(e);
                continue;
            }
        };
        let Some(s) = stat_of(&out, "S=") else {
            failures.push(format!("{args:?}: no stat of the shell; output: {out:?}"));
            continue;
        };
        let parent = stat_of(&out, "P=");
        let leader = s.session == s.pid || s.session == s.ppid;
        let mut why = Vec::new();
        if s.tty_nr == 0 {
            why.push("the shell has no controlling terminal (tty_nr 0)");
        }
        if s.tpgid != s.pgrp {
            why.push("the group of the shell is not the foreground group");
        }
        if !leader {
            why.push("the leader of the session is not the shell or its parent");
        }
        if parent.is_some_and(|p| p.tty_nr != s.tty_nr && s.session == s.ppid) {
            why.push("the parent leads the session on another terminal");
        }
        if out.contains("job control turned off") {
            why.push("the shell turned job control off");
        }
        if !why.is_empty() {
            failures.push(format!(
                "{args:?}: {}; {s:?}; output: {out:?}",
                why.join(", ")
            ));
        }
    }
    let _ = podvm(&store, &["rm", "-f", "t2070"]);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
