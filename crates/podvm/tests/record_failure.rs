//! TODO/cli.md T-2073 (OR-B38-02 of the seventh outside pass): a foreground
//! `run` whose record fails gives the true cause in one line.
//!
//! Before, where the store root belonged to another uid (a root that root
//! owns with mode 777, as a shared `/var/tmp` store), each failure of the
//! record read "... is owned by uid 65534, ... use a store of your own":
//! a name in use and a table that does not parse too. As root the test
//! gives the store root to uid 65534 with mode 777; as another user it
//! checks the same texts on a store of its own.

mod host_image;
use host_image::{podvm, text};

const IMAGE: &str = "localhost/t2073:1";

#[test]
fn a_failed_record_of_a_foreground_run_names_its_cause() {
    let Some(tar) = host_image::layout(IMAGE, &[]) else {
        eprintln!("T-2073: this host has no glibc or dash at the x86_64 paths; skipped");
        return;
    };
    let (dir, store) = host_image::store_with("t2073", tar);
    if std::fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .any(|l| l.starts_with("Uid:") && l.split_whitespace().nth(1) == Some("0"))
    {
        use std::os::unix::fs::PermissionsExt;
        let c = std::ffi::CString::new(store.to_str().unwrap()).unwrap();
        unsafe extern "C" {
            fn chown(path: *const std::ffi::c_char, uid: u32, gid: u32) -> i32;
        }
        // SAFETY: a NUL-ended path and two ids.
        assert_eq!(unsafe { chown(c.as_ptr(), 65534, 65534) }, 0);
        std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o777)).unwrap();
    }
    let run = |args: &[&str]| {
        let mut a = vec!["run", "--rm"];
        a.extend_from_slice(args);
        a.extend_from_slice(&[IMAGE, "sh", "-c", "exit 0"]);
        podvm(&store, &a)
    };
    let first = podvm(
        &store,
        &["run", "--name", "dup", IMAGE, "sh", "-c", "exit 0"],
    );
    assert!(first.status.success(), "{}", text(&first.stderr));
    // (the flags, what the line must name, why)
    let table = store.join("containers.json");
    let cases: &[(&[&str], &[u8], &str)] = &[
        (&["--name", "dup"], b"", "is already used"),
        (&[], b"not json", "move it aside"),
        (&[], b"", "does not parse"),
        (
            &["--name", "x"],
            b"{\"podvm_containers\":1,",
            "move it aside",
        ),
    ];
    let mut failures = Vec::new();
    for (flags, bytes, want) in cases {
        if !bytes.is_empty() || *want == "does not parse" {
            std::fs::write(&table, bytes).unwrap();
        }
        let r = run(flags);
        let err = text(&r.stderr);
        if r.status.code() != Some(125)
            || !err.contains(want)
            || err.contains("is owned by uid")
            || err.lines().count() != 1
        {
            failures.push(format!("{flags:?} {bytes:?}: {:?}: {err}", r.status));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
