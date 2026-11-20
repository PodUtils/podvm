//! TODO/enter.md T-1985: under the write table of the confine layer, each
//! entry point of a change of mode, owner or times answers `EACCES` on a
//! file that no path of the table covers (also through a link), and runs
//! on one that a path covers, and with no table.

use super::*;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::os::unix::io::AsRawFd;

type Call = fn(&std::ffi::CStr, c_int) -> c_int;

#[test]
fn a_change_of_mode_owner_or_times_takes_the_write_table() {
    let dir = std::env::temp_dir().join(format!("podvm-meta-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let root = dir.join("root");
    for sub in ["no", "tmp"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    for f in ["no/s", "tmp/f"] {
        std::fs::write(root.join(f), b"x").unwrap();
        std::fs::set_permissions(root.join(f), std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    symlink("/no/s", root.join("tmp/ln")).unwrap();
    let root = std::fs::canonicalize(&root).unwrap();
    // Each entry point, on a guest path and a descriptor of its host file.
    let calls: [(&str, Call); 18] = [
        ("chmod", |p, _| unsafe { crate::chmod(p.as_ptr(), 0o600) }),
        ("fchmod", |_, fd| unsafe { crate::fchmod(fd, 0o600) }),
        ("fchmodat", |p, _| unsafe {
            crate::fchmodat(AT_FDCWD, p.as_ptr(), 0o600, 0)
        }),
        ("lchmod", |p, _| unsafe { crate::lchmod(p.as_ptr(), 0o600) }),
        ("chown", |p, _| unsafe {
            crate::chown(p.as_ptr(), u32::MAX, u32::MAX)
        }),
        ("lchown", |p, _| unsafe {
            crate::lchown(p.as_ptr(), u32::MAX, u32::MAX)
        }),
        ("fchown", |_, fd| unsafe {
            crate::fchown(fd, u32::MAX, u32::MAX)
        }),
        ("fchownat", |p, _| unsafe {
            crate::fchownat(AT_FDCWD, p.as_ptr(), u32::MAX, u32::MAX, 0)
        }),
        ("fchownat empty", |_, fd| unsafe {
            crate::fchownat(fd, c"".as_ptr(), u32::MAX, u32::MAX, 0x1000)
        }),
        ("utime", |p, _| unsafe {
            crate::utime(p.as_ptr(), core::ptr::null())
        }),
        ("utimes", |p, _| unsafe {
            crate::utimes(p.as_ptr(), core::ptr::null())
        }),
        ("lutimes", |p, _| unsafe {
            crate::lutimes(p.as_ptr(), core::ptr::null())
        }),
        ("utimensat", |p, _| unsafe {
            crate::utimensat(AT_FDCWD, p.as_ptr(), core::ptr::null(), 0)
        }),
        ("utimensat fd", |_, fd| unsafe {
            crate::utimensat(fd, core::ptr::null(), core::ptr::null(), 0)
        }),
        ("futimesat", |p, _| unsafe {
            crate::futimesat(AT_FDCWD, p.as_ptr(), core::ptr::null())
        }),
        ("futimens", |_, fd| unsafe {
            crate::futimens(fd, core::ptr::null())
        }),
        ("futimes", |_, fd| unsafe {
            crate::futimes(fd, core::ptr::null())
        }),
        ("fchmodat nofollow", |p, _| unsafe {
            crate::fchmodat(AT_FDCWD, p.as_ptr(), 0o600, 0x100)
        }),
    ];
    let run = |call: Call, guest: &std::ffi::CStr, host: &std::path::Path| {
        let f = std::fs::File::open(host).unwrap();
        clear_errno();
        let rc = call(guest, f.as_raw_fd());
        (rc, errno())
    };
    let table = format!("{}/tmp", root.display());
    // SAFETY: the interposer tests run with one thread (gate.yml).
    unsafe {
        std::env::set_var("PODVM_MAPS", format!("/:{}", root.display()));
        std::env::set_var("PODVM_WRITE_HOST", &table);
    }
    let (out, inside) = (root.join("no/s"), root.join("tmp/f"));
    for (name, call) in calls {
        assert_eq!(run(call, c"/no/s", &out), (-1, 13), "{name} outside");
        let (rc, e) = run(call, c"/tmp/f", &inside);
        assert!(rc == 0 || e != 13, "{name} inside: errno {e}");
    }
    // A link in a path with write names a file outside: a call that
    // follows it is refused; `lchown` changes the link itself.
    assert_eq!(
        run(calls[0].1, c"/tmp/ln", &out),
        (-1, 13),
        "chmod of the link"
    );
    assert_eq!(
        run(calls[10].1, c"/tmp/ln", &out),
        (-1, 13),
        "utimes of the link"
    );
    assert_eq!(run(calls[5].1, c"/tmp/ln", &out).0, 0, "lchown of the link");
    // With no table, each call runs.
    unsafe { std::env::remove_var("PODVM_WRITE_HOST") };
    for (name, call) in calls {
        let (rc, e) = run(call, c"/no/s", &out);
        assert!(rc == 0 || e != 13, "{name} with no table: errno {e}");
    }
    let mode = std::fs::metadata(&out).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    unsafe { std::env::remove_var("PODVM_MAPS") };
    let _ = std::fs::remove_dir_all(&dir);
}
