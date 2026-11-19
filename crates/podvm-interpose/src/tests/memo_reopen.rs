//! TODO/interpose.md T-2000: a process whose parent closed the memo
//! descriptor (apt's `ExecFork`, npm 6) opens the memo from
//! `PODVM_MEMO_PATH`, and only where the file is the one of
//! `PODVM_MEMO_ID`. Each row is one state of the three variables.

use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;

fn set(name: &[u8], v: Option<&str>) {
    let n = std::str::from_utf8(name).unwrap();
    // SAFETY: the interposer tests run with one thread (gate.yml).
    unsafe {
        match v {
            Some(v) => std::env::set_var(n, v),
            None => std::env::remove_var(n),
        }
    }
}

#[test]
fn a_closed_memo_descriptor_reopens_the_file_of_its_identity() {
    use crate::memo::{MEMO_FD_VAR, MEMO_ID_VAR, MEMO_PATH_VAR};
    let dir = std::env::temp_dir().join(format!("podvm-memo-reopen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let memo = dir.join("ownership.memo");
    let other = dir.join("other");
    std::fs::write(&memo, b"").unwrap();
    std::fs::write(&other, b"").unwrap();
    let held = std::fs::OpenOptions::new()
        .append(true)
        .read(true)
        .open(&memo)
        .unwrap();
    let m = std::fs::metadata(&memo).unwrap();
    let id = format!("{}:{}", m.dev(), m.ino());
    let fd = held.as_raw_fd().to_string();
    let (path, wrong) = (memo.to_str().unwrap(), other.to_str().unwrap());
    // (descriptor, path, the memo is found, it is the handed descriptor)
    for (fdv, pathv, found, handed, what) in [
        (
            Some(fd.as_str()),
            Some(path),
            true,
            true,
            "the handed descriptor",
        ),
        (
            Some("1017"),
            Some(path),
            true,
            false,
            "closed: the path reopens it",
        ),
        (None, Some(path), true, false, "no descriptor: the path"),
        (
            Some("1017"),
            Some(wrong),
            false,
            false,
            "a path of another file",
        ),
        (
            Some("1017"),
            Some("ownership.memo"),
            false,
            false,
            "a relative path",
        ),
        (Some("1017"), None, false, false, "no path"),
    ] {
        crate::memo::forget_reopened();
        set(MEMO_ID_VAR, Some(&id));
        set(MEMO_FD_VAR, fdv);
        set(MEMO_PATH_VAR, pathv);
        let got = crate::memo::memo_fd();
        assert_eq!(got.is_some(), found, "{what}: {got:?}");
        if let Some(g) = got {
            assert_eq!(g == held.as_raw_fd(), handed, "{what}: {g}");
            let o = crate::memo::Owner {
                uid: 5,
                gid: 6,
                set: crate::memo::SET_UID | crate::memo::SET_GID,
            };
            assert!(crate::memo::record(m.dev(), m.ino(), o), "{what}: record");
            assert_eq!(
                crate::memo::lookup(m.dev(), m.ino()),
                crate::memo::Lookup::Hit(o),
                "{what}: lookup"
            );
        }
    }
    crate::memo::forget_reopened();
    for v in [MEMO_ID_VAR, MEMO_FD_VAR, MEMO_PATH_VAR] {
        set(v, None);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
