//! TODO/cli.md T-2071 (OR-B38-01 of the seventh outside pass): a
//! job-control shell through the pty of `exec -it` and `run -it` gets each
//! line that the caller types, in order.
//!
//! Before, the payload of `exec -it` into a `run -d` of a user namespace
//! was a member of podvm's session, and `/dev/tty` of the container was the
//! host's: `sh` (job control on) made its process group the foreground
//! group of the caller's terminal, and the relay of podvm got no input
//! after the first chunk. The test is the caller: it opens a pty, makes it
//! the controlling terminal of podvm's session, as a terminal emulator
//! does, and types into the master.
//!
//! The image is an OCI layout with the host's `dash` as `/bin/sh`, `cat`,
//! `sleep`, the loader and libc. A host with no glibc or no `dash` at the
//! x86_64 paths skips the test with a line on stderr. The fault showed as
//! a user with no root (the join of the user namespace); as root the test
//! checks the same rule on the rung of root.

const IMAGE: &str = "localhost/t2071:1";

mod host_image;
mod tty_session;
use host_image::{podvm, text};
use tty_session::{pty, session, Steps};

/// T-2071: one rule for each entry and each job-control shell: three lines,
/// each answered in order, and the exit code of the end.
#[test]
fn a_job_control_shell_on_the_pty_of_exec_it_and_run_it_gets_each_line() {
    let Some(tar) = host_image::layout(IMAGE, &["cat", "sleep"]) else {
        eprintln!("T-2071: this host has no glibc or dash at the x86_64 paths; skipped");
        return;
    };
    if pty().is_none() {
        eprintln!("T-2071: /dev/ptmx does not open here; skipped");
        return;
    }
    let (dir, store) = host_image::store_with("t2071", tar);
    let d = podvm(
        &store,
        &["run", "-d", "--name", "t2071", IMAGE, "sleep", "300"],
    );
    assert!(d.status.success(), "run -d: {}", text(&d.stderr));

    // `echo A$((40+N))` prints A4N: the echo of the pty shows the command,
    // and only the shell prints the sum. `cat` gives each line twice: the
    // echo of the pty and the copy of cat.
    let interactive: &[(&[u8], &str, usize)] = &[
        (b"echo A$((40+1))\n", "A41", 1),
        (b"echo A$((40+2))\n", "A42", 1),
        (b"echo A$((40+3))\n", "A43", 1),
        (b"exit 7\n", "exit 7", 1),
    ];
    let job_cat: &[(&[u8], &str, usize)] = &[
        (b"L1-x\n", "L1-x", 2),
        (b"L2-x\n", "L2-x", 2),
        (b"L3-x\n", "L3-x", 2),
        (b"\x04", "", 0),
    ];
    let cases: &[(&[&str], Steps, i32)] = &[
        (&["exec", "-it", "t2071", "sh", "-i"], interactive, 7),
        (&["exec", "-it", "t2071", "sh"], interactive, 7),
        (
            &["exec", "-it", "t2071", "sh", "-c", "set -m; cat"],
            job_cat,
            0,
        ),
        (&["run", "--rm", "-it", IMAGE, "sh", "-i"], interactive, 7),
        (
            &["run", "--rm", "-it", IMAGE, "sh", "-c", "set -m; cat"],
            job_cat,
            0,
        ),
    ];
    let mut failures = Vec::new();
    for (args, steps, want) in cases {
        match session(&store, args, steps) {
            Ok((code, _)) if code == *want => {}
            Ok((code, _)) => failures.push(format!("{args:?}: exit {code}, want {want}")),
            Err(e) => failures.push(e),
        }
    }
    let _ = podvm(&store, &["rm", "-f", "t2071"]);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
