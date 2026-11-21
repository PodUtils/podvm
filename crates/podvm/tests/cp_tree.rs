//! TODO/supervise.md T-1822: `cp` of a file onto an existing directory
//! puts the file into it under its own name, in both directions; `cp -r`
//! keeps the directory modes 700 and 711; a containment refusal of `cp`
//! says that it refuses a copy.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct Gone(PathBuf);

impl Drop for Gone {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn podvm(base: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_podvm"))
        .args(args)
        .current_dir(base)
        .env("HOME", base)
        .env("PODVM_STORE", base.join("store"))
        .stdin(Stdio::null())
        .output()
        .expect("podvm runs")
}

fn ok(base: &Path, args: &[&str]) -> String {
    let o = podvm(base, args);
    let err = String::from_utf8_lossy(&o.stderr).to_string();
    assert!(o.status.success(), "{args:?}: {err}");
    err
}

fn mode(p: &Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o7777
}

#[test]
fn cp_onto_a_directory_keeps_modes_and_names_a_refused_copy() {
    let base = std::env::temp_dir().join(format!("podvm-t1822-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("r/etc")).unwrap();
    let _gone = Gone(base.clone());
    std::fs::write(base.join("r/etc/os-release"), "x\n").unwrap();
    // The payload is this static test binary: it runs on each rung, also
    // where chroot is denied (CI), and an image has no `/bin/true`.
    std::fs::create_dir_all(base.join("r/bin")).unwrap();
    std::fs::copy(std::env::current_exe().unwrap(), base.join("r/bin/hello")).unwrap();
    let mut b = tar::Builder::new(Vec::new());
    b.append_dir_all("etc", base.join("r/etc")).unwrap();
    b.append_dir_all("bin", base.join("r/bin")).unwrap();
    std::fs::write(base.join("a.tar"), b.into_inner().unwrap()).unwrap();
    ok(&base, &["import", "a.tar", "t1822:1"]);
    ok(
        &base,
        &["create", "--name", "c1", "t1822:1", "/bin/hello", "--list"],
    );

    // A file onto an existing directory, into the container and out of it.
    std::fs::write(base.join("f.txt"), "hi\n").unwrap();
    ok(&base, &["cp", "f.txt", "c1:/etc"]);
    ok(&base, &["cp", "c1:/etc/f.txt", "c1-out-check.txt"]);
    let back = std::fs::read_to_string(base.join("c1-out-check.txt")).unwrap();
    assert_eq!(back, "hi\n");
    std::fs::create_dir(base.join("out")).unwrap();
    ok(&base, &["cp", "c1:/etc/os-release", "out"]);
    assert_eq!(
        std::fs::read_to_string(base.join("out/os-release")).unwrap(),
        "x\n"
    );

    // `cp -r` keeps 700 and 711, in and out.
    let d = base.join("d");
    std::fs::create_dir_all(d.join("a")).unwrap();
    std::fs::create_dir_all(d.join("b")).unwrap();
    std::fs::write(d.join("a/z"), "y\n").unwrap();
    std::fs::set_permissions(d.join("a"), std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::set_permissions(d.join("b"), std::fs::Permissions::from_mode(0o711)).unwrap();
    ok(&base, &["cp", "-r", "d", "c1:/etc/d"]);
    ok(&base, &["cp", "-r", "c1:/etc/d", "back"]);
    assert_eq!(mode(&base.join("back/a")), 0o700);
    assert_eq!(mode(&base.join("back/b")), 0o711);
    assert_eq!(
        std::fs::read_to_string(base.join("back/a/z")).unwrap(),
        "y\n"
    );

    // A refusal of containment names the copy, in both directions.
    for args in [
        &["cp", "c1:/etc/../etc/os-release", "x"][..],
        &["cp", "f.txt", "c1:/../../x"],
    ] {
        let o = podvm(&base, args);
        let err = String::from_utf8_lossy(&o.stderr);
        // A refusal after the parse exits 1 (T-2062, OQ-10).
        assert_eq!(o.status.code(), Some(1), "{args:?}: {err}");
        assert!(err.contains("copy"), "{args:?}: {err}");
        assert!(!err.contains("delete"), "{args:?}: {err}");
    }
}
