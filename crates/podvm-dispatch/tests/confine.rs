//! The confine layer on the native-static rung: an exec of the dynamic
//! loader (TODO/enter.md T-1984) and a change of mode, owner or times
//! (TODO/enter.md T-1985).

#![cfg(target_arch = "x86_64")]

use std::process::Command;

mod common;
use common::root;

/// OT-02 of the fourth outside pass (TODO/enter.md T-1984): an exec of the
/// dynamic loader takes the exec rule of each argument that the loader can
/// take as its program, by each name of the loader. The probe stands in for
/// the loader: the handler knows a loader by the name of its file.
#[test]
fn an_exec_of_the_loader_takes_the_exec_rule_of_its_program() {
    let Some(r) = root("ldso") else { return };
    std::fs::copy(r.0.join("bin/probe"), r.0.join("tmp/probe2")).unwrap();
    for name in ["ld-linux-x86-64.so.2", "ld-2.28.so", "ld-musl-x86_64.so.1"] {
        std::fs::copy(r.0.join("bin/probe"), r.0.join("bin").join(name)).unwrap();
    }
    std::fs::create_dir_all(r.0.join("lib64")).unwrap();
    std::os::unix::fs::symlink(
        "/bin/ld-linux-x86-64.so.2",
        r.0.join("lib64/ld-linux-x86-64.so.2"),
    )
    .unwrap();
    let bin = format!("{}/bin", std::fs::canonicalize(&r.0).unwrap().display());
    let ld = "/bin/ld-linux-x86-64.so.2";
    let denied = "ERR Permission denied";
    // The stand-in runs where the rule lets it: it knows no verb of that
    // name, and exits 2.
    let ran = "spawn 2";
    for (table, args, want) in [
        (
            Some(bin.as_str()),
            &[ld, "/tmp/probe2", "cat", "/etc/podvm-marker"][..],
            denied,
        ),
        (
            Some(&bin),
            &[ld, "--library-path", "/tmp", "/tmp/probe2"],
            denied,
        ),
        (
            Some(&bin),
            &[ld, "--library-path", "/tmp", "probe2"],
            denied,
        ),
        (
            Some(&bin),
            &[ld, "--inhibit-cache", "--argv0", "x", "/tmp/probe2"],
            denied,
        ),
        (Some(&bin), &[ld, "--", "/tmp/probe2"], denied),
        (
            Some(&bin),
            &["/lib64/ld-linux-x86-64.so.2", "/tmp/probe2"],
            denied,
        ),
        (Some(&bin), &["/bin/ld-2.28.so", "/tmp/probe2"], denied),
        (
            Some(&bin),
            &["/bin/ld-musl-x86_64.so.1", "/tmp/probe2"],
            denied,
        ),
        (
            Some(&bin),
            &[ld, "/bin/probe", "cat", "/etc/podvm-marker"],
            ran,
        ),
        (Some(&bin), &[ld, "/tmp/absent"], ran),
        (Some(&bin), &[ld, "--version"], ran),
        (None, &[ld, "/tmp/probe2", "cat", "/etc/podvm-marker"], ran),
    ] {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_podvm-dispatch-run"));
        match table {
            Some(t) => cmd.env("PODVM_EXEC_HOST", t),
            None => cmd.env_remove("PODVM_EXEC_HOST"),
        };
        let out = cmd
            .env_remove(podvm_dispatch::TIER_VAR)
            .arg(&r.0)
            .arg("/bin/probe")
            .arg("probe")
            .arg("spawn")
            .args(args)
            .current_dir(&r.0)
            .output()
            .unwrap();
        let got = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
        assert!(got.starts_with(want), "{table:?} {args:?}: {got:?}");
    }
}

/// OT-03 of the fourth outside pass (TODO/enter.md T-1985): each call of a
/// change of mode, owner or times answers EACCES on a file that no path of
/// the write table covers (also through a link), and runs on one that a
/// path covers, and with no table.
#[test]
fn a_change_of_mode_owner_or_times_takes_the_write_table() {
    let Some(r) = root("meta") else { return };
    std::fs::create_dir_all(r.0.join("no")).unwrap();
    for f in ["no/s", "tmp/f"] {
        std::fs::write(r.0.join(f), b"x").unwrap();
        std::fs::set_permissions(
            r.0.join(f),
            std::os::unix::fs::PermissionsExt::from_mode(0o644),
        )
        .unwrap();
    }
    std::os::unix::fs::symlink("/no/s", r.0.join("tmp/ln")).unwrap();
    let tmp = format!("{}/tmp", std::fs::canonicalize(&r.0).unwrap().display());
    let run = |table: Option<&str>, call: &str, path: &str| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_podvm-dispatch-run"));
        match table {
            Some(t) => cmd.env("PODVM_WRITE_HOST", t),
            None => cmd.env_remove("PODVM_WRITE_HOST"),
        };
        let out = cmd
            .env_remove(podvm_dispatch::TIER_VAR)
            .arg(&r.0)
            .arg("/bin/probe")
            .arg("probe")
            .args(["meta", call, path])
            .current_dir(&r.0)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim_end().to_string()
    };
    let calls = [
        "chmod",
        "fchmod",
        "fchmodat",
        "fchmodat2",
        "chown",
        "fchown",
        "lchown",
        "fchownat",
        "fchownat-empty",
        "utime",
        "utimes",
        "utimensat",
        "utimensat-fd",
        "futimesat",
    ];
    for call in calls {
        let t = Some(tmp.as_str());
        assert_eq!(run(t, call, "/no/s"), "meta ERR 13", "{call} outside");
        let inside = run(t, call, "/tmp/f");
        assert!(!inside.ends_with(" 13"), "{call} inside: {inside}");
        let none = run(None, call, "/no/s");
        assert!(!none.ends_with(" 13"), "{call} with no table: {none}");
    }
    // A link in a path with write names a file outside: each call that
    // follows it is refused; `lchown` changes the link itself.
    for (call, want) in [
        ("chmod", "meta ERR 13"),
        ("utimes", "meta ERR 13"),
        ("lchown", "meta ok"),
    ] {
        assert_eq!(run(Some(&tmp), call, "/tmp/ln"), want, "{call} of the link");
    }
}
