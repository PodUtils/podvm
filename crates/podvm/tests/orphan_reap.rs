//! TODO/cli.md T-2087 (OR-B40-05): PID 1 of the PID namespace of a
//! foreground run reaps each orphan of the payload as it exits, as docker's
//! PID 1 does, and the end of the run still ends each process of the
//! payload (TODO/supervise.md T-1708).
//!
//! The payload starts three orphans that exit at once and one that stays,
//! waits, and counts the processes in state `Z`. After the run, no process
//! of the payload is left on the host. The image is an OCI layout with the
//! host's `dash` as `/bin/sh` and `sleep`, `cat`. A host with no glibc or no
//! `dash` at the x86_64 paths skips the test with a line on stderr. Where
//! PID 1 of the payload is no init of podvm (a rung with no PID
//! namespace), the test holds the end of the run only.

mod host_image;
use host_image::{podvm, text};

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
    let Some(tar) = host_image::layout(IMAGE, &["sleep", "cat"]) else {
        eprintln!("OR-B40-05: this host has no glibc or dash at the x86_64 paths; skipped");
        return;
    };
    let (dir, store) = host_image::store_with("orphanreap", tar);
    // An argument that no other process of the host has: the orphan that
    // stays.
    let stays = format!("4{}", std::process::id());
    let script = format!(
        "for i in 1 2 3; do (sleep 0.1 &); done; (sleep {stays} &); sleep 1.5; \
         z=0; for f in /proc/[0-9]*/stat; do read -r l 2>/dev/null <\"$f\" || continue; \
         case \"$l\" in *'(sleep) Z '*) z=$((z+1));; esac; done; \
         printf 'init=%s zombies=%s\\n' \"$(cat /proc/1/comm)\" \"$z\""
    );
    let r = podvm(&store, &["run", "--rm", IMAGE, "sh", "-c", &script]);
    // The end of the run: no process of the payload is left when podvm
    // returns (T-1708).
    let left = host_pids(format!("sleep\0{stays}\0").as_bytes());
    let out = text(&r.stdout);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        r.status.success(),
        "run: exit {:?}, out {out:?}, err {:?}",
        r.status.code(),
        text(&r.stderr)
    );
    assert!(
        left.is_empty(),
        "processes of the payload stayed after the run: {left:?}"
    );
    if !out.starts_with("init=podvm-init ") {
        eprintln!(
            "OR-B40-05: PID 1 of the payload is no init of podvm ({out:?}); the count is skipped"
        );
        return;
    }
    assert_eq!(
        out,
        "init=podvm-init zombies=0\n",
        "err {:?}",
        text(&r.stderr)
    );
}
