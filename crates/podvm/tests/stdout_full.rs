//! TODO/cli.md T-1816: with stdout on `/dev/full`, a verb exits 125 and
//! names the error on stderr: no SIGABRT (134), and no exit 0 with its
//! answer lost. TODO/cli.md T-2100 (OR-B41-17): one failure gives one
//! line, also for a verb that writes through its own writer (`save`).

use std::process::{Command, Stdio};

fn full(args: &[&str], store: &std::path::Path) -> std::process::Output {
    let dev_full = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .expect("/dev/full opens");
    Command::new(env!("CARGO_BIN_EXE_podvm"))
        .args(args)
        .env("PODVM_STORE", store)
        .stdin(Stdio::null())
        .stdout(Stdio::from(dev_full))
        .stderr(Stdio::piped())
        .output()
        .expect("podvm runs")
}

#[test]
fn stdout_full_is_125_with_the_errno_named() {
    let store = std::env::temp_dir().join(format!("podvm-full-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    // An image of one file, for `save`.
    let mut b = tar::Builder::new(Vec::new());
    let mut h = tar::Header::new_gnu();
    h.set_size(4);
    h.set_mode(0o644);
    h.set_cksum();
    b.append_data(&mut h, "f", &b"abcd"[..]).unwrap();
    std::fs::create_dir_all(&store).unwrap();
    let tar = store.join("f.tar");
    std::fs::write(&tar, b.into_inner().unwrap()).unwrap();
    let i = Command::new(env!("CARGO_BIN_EXE_podvm"))
        .args(["import", tar.to_str().unwrap(), "full:1"])
        .env("PODVM_STORE", &store)
        .output()
        .unwrap();
    assert!(i.status.success(), "{}", String::from_utf8_lossy(&i.stderr));
    for args in [
        &["version"][..],
        &["ps", "-a"],
        // The table of the store, with its header.
        &["images"],
        &["probe"],
        &["save", "full:1"],
    ] {
        let o = full(args, &store);
        let err = String::from_utf8_lossy(&o.stderr);
        assert_eq!(o.status.code(), Some(125), "{args:?}: {err}");
        let lines: Vec<&str> = err
            .lines()
            .filter(|l| l.contains("No space left on device"))
            .collect();
        assert_eq!(lines.len(), 1, "{args:?}: {err}");
        assert!(lines[0].contains(": stdout: "), "{args:?}: {err}");
    }
    let _ = std::fs::remove_dir_all(&store);
}
