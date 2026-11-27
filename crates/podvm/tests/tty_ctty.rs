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
//! The table has a row for each rung that this host and CI give: the rung
//! that podvm takes here (a user: the namespace rung through podvm's own
//! user namespace; root: the namespace rung of root), and the rung of a
//! host that refuses each user namespace (the userland rung), which the
//! test makes with a user namespace that allows no child. The runner of
//! GitHub refuses each user namespace, so there the first row is the
//! userland rung already and the second row cannot be made: the test says
//! so in one line. As root the second row is not made: a user namespace of
//! the test would make podvm uid 0 of a nested namespace, which no caller
//! has.
//!
//! ⚠ One rung gives no job control: where the shell is in a PID namespace
//! that holds no process of podvm (the namespace rung of root: its parent
//! is the init of `pod-enter`, or is outside the namespace). The first
//! group of the shell is podvm's there, which the shell cannot name, so
//! podvm gives that payload no terminal (`run/pty/session.rs`,
//! `ctty_holds`) until `pod-enter` gives it a session (podcrates@T-927).
//! For that rung the rule is the gap as documented: no terminal (not the
//! caller's), job control off, Ctrl-C ends `sleep 30`, and exit 0.

mod host_image;
mod tty_session;
use host_image::text;
use tty_session::{can_prep_no_userns, podvm_on, pty, session_on, Prep, Steps};

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

/// One rule for each rung and each path of `-it`: the controlling
/// terminal is not the caller's (its session is another), the foreground group,
/// the leader of the session, the keys, and no line of a shell that has no
/// terminal.
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
    let mut preps = vec![Prep::Host];
    // SAFETY: geteuid takes nothing and cannot fail.
    if unsafe { geteuid() } == 0 {
        eprintln!("T-2070: as root the row with no user namespace is not made (see the module)");
    } else if can_prep_no_userns() {
        preps.push(Prep::NoUserns);
    } else {
        eprintln!(
            "T-2070: this host refuses a user namespace to the test: the first row is the rung of \
             a host with none"
        );
    }
    let (dir, store) = host_image::store_with("t2070", tar);
    let mut failures = Vec::new();
    for prep in preps {
        let name = format!("t2070-{prep:?}").to_lowercase();
        let d = podvm_on(&store, prep)
            .args(["run", "-d", "--name", &name, IMAGE, "sleep", "300"])
            .output()
            .unwrap();
        assert!(d.status.success(), "{prep:?}: run -d: {}", text(&d.stderr));
        let cases: [&[&str]; 2] = [
            &["run", "--rm", "-it", IMAGE, "sh", "-i"],
            &["exec", "-it", &name, "sh", "-i"],
        ];
        for args in cases {
            if let Err(why) = check(&store, prep, args) {
                failures.push(format!("{prep:?} {args:?}: {why}"));
            }
        }
        let _ = podvm_on(&store, prep).args(["rm", "-f", &name]).output();
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// One session of `args`: its output, or the failure.
fn run(
    store: &std::path::Path,
    prep: Prep,
    args: &[&str],
    steps: Steps,
) -> Result<(String, i64), String> {
    match session_on(store, prep, args, steps) {
        Ok((0, out, caller)) => Ok((out, caller)),
        Ok((code, out, _)) => Err(format!("exit {code}; output: {out:?}")),
        Err(e) => Err(e),
    }
}

/// The rule of one row: first the stat of the shell, then the keys.
fn check(store: &std::path::Path, prep: Prep, args: &[&str]) -> Result<(), String> {
    let (out, caller) = run(
        store,
        prep,
        args,
        &[(PROBE, "E42N", 1), (b"exit 0\n", "exit 0", 1)],
    )?;
    let s = stat_of(&out, "S=").ok_or_else(|| format!("no stat of the shell; output: {out:?}"))?;
    let parent = stat_of(&out, "P=");
    let fail = |why: &[&str]| Err(format!("{}; {s:?}; output: {out:?}", why.join(", ")));
    // The gap of the module: no process of podvm in the PID namespace.
    let gap = s.ppid == 0 || out.contains("(podvm-init)");
    // `sleep 30` outlasts the wait of a step (20 s): only the key ends it.
    let ctrl_c: &[(&[u8], &str, usize)] = &[
        (b"sleep 30\n", "sleep 30", 1),
        (b"\x03", "", 0),
        (b"echo C$((40+3))\n", "C43", 1),
    ];
    let ctrl_z: &[(&[u8], &str, usize)] = &[
        (b"sleep 30\n", "sleep 30", 2),
        (b"\x1a", "Stopped", 1),
        (b"kill -9 %1; sleep 1; jobs; echo Z$((40+4))\n", "Z44", 1),
    ];
    let exit: &[(&[u8], &str, usize)] = &[(b"exit 0\n", "exit 0", 1)];
    let mut why = Vec::new();
    // The terminal of a payload in the caller's session is the caller's
    // (the fault of a run that did not leave it).
    if caller > 0 && s.session == caller {
        why.push("the shell is in the session of the caller's terminal");
    }
    if gap {
        if s.tty_nr != 0 {
            why.push("a shell with no podvm in its PID namespace has a terminal");
        }
        if !why.is_empty() {
            return fail(&why);
        }
        return run(store, prep, args, &[ctrl_c, exit].concat()).map(|_| ());
    }
    // The leader of the session is the shell or its parent (podvm).
    if s.session != s.pid && s.session != s.ppid {
        why.push("the leader of the session is not the shell or its parent");
    }
    if s.tty_nr == 0 {
        why.push("the shell has no controlling terminal (tty_nr 0)");
    }
    if s.tpgid != s.pgrp {
        why.push("the group of the shell is not the foreground group");
    }
    if parent.is_some_and(|p| p.tty_nr != s.tty_nr) {
        why.push("the parent leads the session on another terminal");
    }
    if out.contains("job control turned off") {
        why.push("the shell turned job control off");
    }
    if !why.is_empty() {
        return fail(&why);
    }
    run(store, prep, args, &[ctrl_c, ctrl_z, exit].concat()).map(|_| ())
}
