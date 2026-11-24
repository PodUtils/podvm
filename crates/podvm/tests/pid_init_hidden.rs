//! TODO/enter.md T-2054 (OQ-08, PodUtils/podvm#646): the init of the PID
//! namespace of `run -d` on the namespace rung and the processes of a join
//! of `exec` show a title and no argument or variable of podvm to the
//! payload. Since podcrates `v0.82.1` pod-enter sets the title of its init
//! and of its relay itself, and makes both not dumpable (podcrates@T-1112,
//! podcrates@T-1113). A caller with no root then cannot open the
//! namespaces of the init, so each join of podvm (`exec`, `exec -t`, `cp`,
//! the UTS namespace of `--hostname`) opens the payload of the run
//! (`pod_enter::joinable`). One table holds each rule for each caller.
//!
//! The image is an OCI layout with the host's `dash` as `/bin/sh`, `cat`,
//! `sleep` and `readlink`. A host with no glibc or no `dash` at the x86_64
//! paths, or a run that does not enter the namespace rung, skips the test
//! with a line on stderr.

use std::path::Path;
use std::process::{Command, Output};

mod host_image;
use host_image::text;

const IMAGE: &str = "localhost/t2054:1";
const NAME: &str = "t2054";
/// A variable of the caller of `run -d`, and one of `-e`: neither may show
/// in a process of podvm.
const CALLER: &str = "PODVM_T2054_CALLER=caller-token-t2054";
const SECRET: &str = "hunter2-t2054";

/// podvm with `args`, with the variable of the caller.
fn podvm(store: &Path, args: &[&str]) -> Output {
    let (k, v) = CALLER.split_once('=').unwrap();
    Command::new(env!("CARGO_BIN_EXE_podvm"))
        .env("PODVM_SPARE_TREE", "0")
        .env("PODVM_STORE", store)
        .env(k, v)
        .args(args)
        .output()
        .unwrap()
}

fn is_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .any(|l| l.starts_with("Uid:") && l.split_whitespace().nth(1) == Some("0"))
}

#[test]
fn the_init_and_the_join_show_no_argument_of_podvm_and_a_join_still_works() {
    let Some(tar) = host_image::layout(IMAGE, &["cat", "sleep", "readlink"]) else {
        eprintln!("T-2054: this host has no glibc or dash at the x86_64 paths; skipped");
        return;
    };
    let (dir, store) = host_image::store_with(NAME, tar);
    let secret = format!("SECRET={SECRET}");
    let d = podvm(
        &store,
        &[
            "run",
            "-d",
            "--name",
            NAME,
            "--hostname",
            "t2054-name",
            "-e",
            &secret,
            IMAGE,
            "sleep",
            "300",
        ],
    );
    let rung = text(&podvm(&store, &["inspect", NAME]).stdout);
    if !d.status.success() || !rung.contains("\"Rung\":\"namespace\"") {
        eprintln!("T-2054: run -d does not enter the namespace rung here; skipped");
        let _ = podvm(&store, &["rm", "-f", NAME]);
        let _ = std::fs::remove_dir_all(&dir);
        return;
    }
    let file = dir.join("in.txt");
    std::fs::write(&file, b"t2054-copied\n").unwrap();
    let to = format!("{NAME}:/tmp/in.txt");
    let cp = podvm(&store, &["cp", file.to_str().unwrap(), &to]);

    // Each process of the run: its pid and its command line, one a line.
    let each = "for p in /proc/[0-9]*; do printf '%s ' ${p#/proc/}; cat $p/cmdline; echo; done";
    // (what, the command of exec, the rule of its output)
    type Rule = fn(&str) -> bool;
    let cases: &[(&str, &[&str], Rule)] = &[
        (
            "/proc/1/cmdline is the title of the init",
            &["exec", NAME, "cat", "/proc/1/cmdline"],
            |o| o == "podvm-init\0",
        ),
        (
            "/proc/1/comm is the title of the init",
            &["exec", NAME, "cat", "/proc/1/comm"],
            |o| o == "podvm-init\n",
        ),
        (
            "/proc/1/environ holds no variable",
            &[
                "exec",
                NAME,
                "sh",
                "-c",
                "cat /proc/1/environ 2>/dev/null; true",
            ],
            |o| o.is_empty(),
        ),
        (
            "no process of the run shows podvm's argv or a variable",
            &["exec", NAME, "sh", "-c", each],
            |o| {
                // A process of podvm shows its title alone: `podvm-exec`
                // where a join of a caller with no root stays in the run.
                let titled = |l: &&str| l.ends_with("podvm-init\0") || l.ends_with("podvm-exec\0");
                o.contains("podvm-init")
                    && o.contains("sleep")
                    && o.lines()
                        .filter(|l| l.contains("podvm"))
                        .all(|l| titled(&l))
                    && !o.contains("hunter2")
                    && !o.contains("caller-token")
                    && !o.contains("--name")
            },
        ),
        (
            "the exec joins the PID namespace and the mounts of the run",
            &["exec", NAME, "cat", "/proc/2/cmdline"],
            |o| o == "sleep\x00300\x00",
        ),
        (
            "the exec joins the UTS namespace of the run",
            &["exec", NAME, "cat", "/proc/sys/kernel/hostname"],
            |o| o == "t2054-name\n",
        ),
        (
            "the payload of the exec gets the -e of the run",
            &["exec", NAME, "sh", "-c", "echo $SECRET"],
            |o| o == "hunter2-t2054\n",
        ),
        (
            "cp joins the mounts of the run",
            &["exec", NAME, "cat", "/tmp/in.txt"],
            |o| o == "t2054-copied\n",
        ),
        (
            "exec -t takes a pty of the devpts of the run",
            &[
                "exec",
                "-t",
                NAME,
                "sh",
                "-c",
                "test -c \"$(readlink /proc/self/fd/0)\" && echo in-run",
            ],
            |o| o.contains("in-run"),
        ),
    ];
    let mut failures = Vec::new();
    if !cp.status.success() {
        failures.push(format!("cp: {}", text(&cp.stderr)));
    }
    for (what, args, rule) in cases {
        let r = podvm(&store, args);
        let out = text(&r.stdout);
        if !r.status.success() || !rule(&out) {
            failures.push(format!(
                "{what}: {args:?}: exit {:?}, out {out:?}, err {:?}",
                r.status.code(),
                text(&r.stderr)
            ));
        }
    }
    // podcrates@T-1113: the init is not dumpable, so a payload with no
    // capability over the user namespace of the caller cannot read its
    // memory. A caller of uid 0 keeps that capability.
    // T-2084: the process of podvm's own join (`podvm-exec`) is not
    // dumpable either: its memory holds the variables of the caller.
    let exec_maps = "for p in /proc/[0-9]*; do [ \"$(cat $p/comm)\" = podvm-exec ] || continue; \
                     cat $p/maps >/dev/null 2>&1 && echo readable || echo refused; done";
    let dumps: &[(&str, &str)] = &[
        (
            "the init",
            "cat /proc/1/maps >/dev/null 2>&1 && echo readable || echo refused",
        ),
        ("the process of the join", exec_maps),
    ];
    for (what, script) in dumps.iter().filter(|_| !is_root()) {
        let r = podvm(&store, &["exec", NAME, "sh", "-c", script]);
        if text(&r.stdout) != "refused\n" {
            failures.push(format!(
                "{what} is not dumpable: out {:?}, err {:?}",
                text(&r.stdout),
                text(&r.stderr)
            ));
        }
    }
    let _ = podvm(&store, &["rm", "-f", NAME]);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
