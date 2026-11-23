//! TODO/cli.md T-2075 (OR-B38-03 of the seventh outside pass): `exec` into
//! a running container keeps the `/etc/hosts` of the run: the lines of
//! `--add-host` and of `--hostname`. Before, each `exec` rewrote the file
//! with the name that the caller typed and dropped those lines, also for
//! the payload of the run.

mod host_image;
use host_image::{podvm, text};

const IMAGE: &str = "localhost/t2075:1";

#[test]
fn exec_keeps_the_hosts_of_the_run() {
    let Some(tar) = host_image::layout(IMAGE, &["cat", "sleep"]) else {
        eprintln!("T-2075: this host has no glibc or dash at the x86_64 paths; skipped");
        return;
    };
    let (dir, store) = host_image::store_with("t2075", tar);
    let d = podvm(
        &store,
        &[
            "run",
            "-d",
            "--name",
            "t2075",
            "--hostname",
            "hx",
            "--add-host",
            "a.example:192.0.2.1",
            IMAGE,
            "sleep",
            "60",
        ],
    );
    assert!(d.status.success(), "run -d: {}", text(&d.stderr));
    let want = ["192.0.2.1\ta.example", "127.0.0.1\thx"];
    let mut failures = Vec::new();
    // Each exec completes the tree again: the second and the third read
    // what the first wrote.
    for i in 0..3 {
        let r = podvm(&store, &["exec", "t2075", "cat", "/etc/hosts"]);
        let hosts = text(&r.stdout);
        for w in want {
            if !hosts.contains(w) {
                failures.push(format!(
                    "exec {i}: no {w:?} in {hosts:?} {}",
                    text(&r.stderr)
                ));
            }
        }
    }
    let _ = podvm(&store, &["rm", "-f", "t2075"]);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
