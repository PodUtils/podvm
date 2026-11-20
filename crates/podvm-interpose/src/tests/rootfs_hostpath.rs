//! TODO/enter.md T-2045: a host path of the rootfs (T-1705) walks its
//! links from the rootfs. One rule for the class: no path that names the
//! host path of the rootfs, through a link or as given, with `..` or with
//! a link in it, reads a file out of the rootfs; the paths of the loader
//! and of `/.podvm/` still open with one prefix.

use super::*;

unsafe extern "C" {
    fn close(fd: c_int) -> c_int;
    fn read(fd: c_int, buf: *mut c_void, n: usize) -> isize;
}

const O_WRONLY: c_int = 0o1;
const O_CREAT: c_int = 0o100;
const ENOENT: c_int = 2;

/// The first bytes of `p` through the interposer, or the errno.
fn cat(p: &str) -> Result<String, c_int> {
    let p = CString::new(p).unwrap();
    clear_errno();
    let fd = unsafe { crate::open(p.as_ptr(), 0, 0) };
    if fd < 0 {
        return Err(errno());
    }
    let mut b = [0u8; 64];
    let n = unsafe { read(fd, b.as_mut_ptr() as *mut c_void, 64) };
    unsafe { close(fd) };
    Ok(String::from_utf8_lossy(&b[..n.max(0) as usize]).into_owned())
}

#[test]
fn a_host_path_of_the_rootfs_reads_no_file_out_of_the_rootfs() {
    let dir = std::env::temp_dir().join(format!("podvm-t2045-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for d in [
        "root/etc",
        "root/tmp",
        "root/lib",
        "root/usr/lib",
        "root/.podvm",
        "etc",
        "usr/lib",
    ] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    let root = dir.join("root");
    let r = root.display().to_string();
    for (f, text) in [
        ("root/etc/hostname", "image"),
        ("root/lib/libx.so", "libx"),
        ("root/usr/lib/liby.so", "liby"),
        ("root/.podvm/obj", "object"),
        ("etc/hostname", "HOST"),
        ("usr/lib/liby.so", "HOST"),
    ] {
        std::fs::write(dir.join(f), text).unwrap();
    }
    let ln = |target: &str, at: &str| std::os::unix::fs::symlink(target, root.join(at)).unwrap();
    ln(&format!("{r}/../etc"), "tmp/esc");
    ln(&format!("{r}/.podvm/../../etc"), "tmp/pv");
    ln(&format!("{r}/tmp/../../../../../../../../etc"), "tmp/deep");
    ln("../../etc", "tmp/rel");
    ln("../etc", "tmp/ok");
    ln("/usr/lib", "lib64");
    // The host side of the rootfs as a link out of it: the guest's `/usr`.
    ln(&format!("{}/usr", dir.display()), "usr2");
    // SAFETY: the interposer tests run with one thread (gate.yml).
    unsafe { std::env::set_var("PODVM_MAPS", format!("/:{r}")) };
    let cases: [(&str, String, Result<&str, c_int>); 13] = [
        ("the guest path", "/etc/hostname".into(), Ok("image")),
        (
            "a link to ROOTFS/..",
            "/tmp/esc/hostname".into(),
            Ok("image"),
        ),
        (
            "a link to ROOTFS/.podvm/..",
            "/tmp/pv/hostname".into(),
            Ok("image"),
        ),
        (
            "a link to ROOTFS/tmp/../..",
            "/tmp/deep/hostname".into(),
            Ok("image"),
        ),
        (
            "as given, a relative link",
            format!("{r}/tmp/rel/hostname"),
            Ok("image"),
        ),
        (
            "as given, a link to ROOTFS/..",
            format!("{r}/tmp/esc/hostname"),
            Ok("image"),
        ),
        (
            "as given, ROOTFS/..",
            format!("{r}/../etc/hostname"),
            Err(ENOENT),
        ),
        (
            "as given, an absolute link",
            format!("{r}/lib64/liby.so"),
            Ok("liby"),
        ),
        (
            "as given, a link to a host path",
            format!("{r}/usr2/lib/liby.so"),
            Err(ENOENT),
        ),
        (
            "a relative link in the rootfs",
            format!("{r}/tmp/ok/hostname"),
            Ok("image"),
        ),
        (
            "T-1705: a path of the loader",
            format!("{r}/lib/libx.so"),
            Ok("libx"),
        ),
        (
            "T-1705: ROOTFS/.podvm",
            format!("{r}/.podvm/obj"),
            Ok("object"),
        ),
        (
            "T-1705: the rootfs itself",
            format!("{r}/etc/../etc/hostname"),
            Ok("image"),
        ),
    ];
    let mut failed = Vec::new();
    for (name, p, want) in &cases {
        let got = cat(p);
        if got.as_deref().map_err(|e| *e) != *want {
            failed.push(format!("{name} ({p}): {got:?}, want {want:?}"));
        }
    }
    // A write by the host path of the rootfs lands in the rootfs.
    let made = CString::new(format!("{r}/tmp/made")).unwrap();
    let fd = unsafe { crate::open(made.as_ptr(), O_WRONLY | O_CREAT, 0o644) };
    if fd < 0 {
        failed.push(format!("a create by the host path: errno {}", errno()));
    } else {
        unsafe { close(fd) };
    }
    unsafe { std::env::remove_var("PODVM_MAPS") };
    assert!(failed.is_empty(), "{failed:#?}");
    assert!(root.join("tmp/made").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}
