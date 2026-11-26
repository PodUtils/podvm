//! TODO/cli.md T-2087 (OR-B40-05) and TODO/enter.md T-2054 item 1: each
//! foreground run of the namespace rung has a PID namespace whose PID 1 is
//! an init of podvm (`podvm-init`): podvm's own for a caller that is not
//! uid 0 (T-1909), pod-enter's for uid 0 and with `PODVM_USERNS=0`. PID 1
//! reaps each orphan of the payload as it exits, as docker's PID 1 does;
//! the payload holds no descriptor of the image lock; and the end of the
//! run still ends each process of the payload (TODO/supervise.md T-1708).
//!
//! The payload starts three orphans that exit at once and one that stays,
//! waits, counts the processes in state `Z` and its descriptors of a lock.
//! After the run, no process of the payload is left on the host. The image
//! is an OCI layout with the host's `dash` as `/bin/sh` and `sleep`, `cat`,
//! `readlink`. A host with no glibc or no `dash` at the x86_64 paths skips
//! the test with a line on stderr. A run that does not enter the namespace
//! rung holds the end of the run only.

use std::process::Command;

mod host_image;
use host_image::text;

const IMAGE: &str = "localhost/orphanreap:1";

/// The pids of the host whose command line is `cmdline`.
fn host_pids(cmdline: &[u8]) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    rd.flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.parse::<u32>().is_ok())
        })
        .filter(|e| std::fs::read(e.path().join("cmdline")).is_ok_and(|c| c == cmdline))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn pid_1_reaps_each_orphan_at_once_and_the_end_of_the_run_ends_each_process() {
    let Some(tar) = host_image::layout(IMAGE, &["sleep", "cat", "readlink"]) else {
        eprintln!("OR-B40-05: this host has no glibc or dash at the x86_64 paths; skipped");
        return;
    };
    let (dir, store) = host_image::store_with("orphanreap", tar);
    let mut failures = Vec::new();
    // (the shape, the value of PODVM_USERNS)
    for (shape, userns) in [("the default entry", None), ("PODVM_USERNS=0", Some("0"))] {
        // An argument that no other process of the host has: the orphan
        // that stays.
        let stays = format!("4{}{}", std::process::id(), u8::from(userns.is_some()));
        let script = format!(
            "for i in 1 2 3; do (sleep 0.1 &); done; (sleep {stays} &); sleep 1.5; \
             z=0; for f in /proc/[0-9]*/stat; do read -r l 2>/dev/null <\"$f\" || continue; \
             case \"$l\" in *'(sleep) Z '*) z=$((z+1));; esac; done; \
             k=0; for d in /proc/$$/fd/*; do case \"$(readlink $d)\" in *.lock) k=$((k+1));; esac; done; \
             printf 'init=%s zombies=%s locks=%s\\n' \"$(cat /proc/1/comm)\" \"$z\" \"$k\""
        );
        let mut c = Command::new(env!("CARGO_BIN_EXE_podvm"));
        c.env("PODVM_SPARE_TREE", "0").env("PODVM_STORE", &store);
        if let Some(v) = userns {
            c.env("PODVM_USERNS", v);
        }
        let r = c
            .args(["run", "--rm", "--verbose", IMAGE, "sh", "-c", &script])
            .output()
            .unwrap();
        // The end of the run: no process of the payload is left when podvm
        // returns (T-1708).
        let left = host_pids(format!("sleep\0{stays}\0").as_bytes());
        let (out, err) = (text(&r.stdout), text(&r.stderr));
        if !r.status.success() || !left.is_empty() {
            failures.push(format!(
                "{shape}: exit {:?}, processes that stayed {left:?}, out {out:?}, err {err:?}",
                r.status.code()
            ));
            continue;
        }
        if !err.contains("mode=namespace") {
            eprintln!(
                "OR-B40-05: {shape} does not enter the namespace rung here; the count is skipped"
            );
            continue;
        }
        if out != "init=podvm-init zombies=0 locks=0\n" {
            failures.push(format!("{shape}: out {out:?}"));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
