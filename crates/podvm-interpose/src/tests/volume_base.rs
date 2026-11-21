//! TODO/enter.md T-1972 (PodUtils/podrunner#277, #300 ask 3): under
//! `PODVM_VOLUME_BASE`, a path of a volume under the base reaches no file
//! out of the base. One rule for the class: a link under the base that the
//! host makes after the start (also between the walk and the call) gives
//! the call ELOOP, EXDEV or ENOENT, never the file out of the base; a plain
//! path reads and writes.

use super::*;
use crate::map::base::{Pin, pin};

unsafe extern "C" {
    fn close(fd: c_int) -> c_int;
    fn read(fd: c_int, buf: *mut c_void, n: usize) -> isize;
    fn write(fd: c_int, buf: *const c_void, n: usize) -> isize;
    fn fcntl(fd: c_int, cmd: c_int, arg: c_int) -> c_int;
    fn fclose(f: *mut c_void) -> c_int;
}

const O_WRONLY: c_int = 0o1;
const O_CREAT: c_int = 0o100;
const ELOOP: c_int = 40;
const EXDEV: c_int = 18;
const ENOENT: c_int = 2;

/// One case of the check: the host path, whether the call follows its last
/// link, whether it can make the file, the call, and the answer.
type Case = (String, bool, bool, &'static [u8], Result<(), c_int>);

/// The text of `p` through the interposer, or the errno.
fn cat(p: &core::ffi::CStr) -> Result<String, c_int> {
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

/// `f` as a process of the host: the test binary exports the interposed
/// calls, so the variables leave for the time of `f`.
fn host<T>(f: impl FnOnce() -> T) -> T {
    let keep: Vec<_> = ["PODVM_MAPS", "PODVM_VOLUME_BASE"]
        .iter()
        .map(|k| (k, std::env::var_os(k)))
        .collect();
    // SAFETY: the interposer tests run with one thread (gate.yml).
    unsafe {
        std::env::remove_var("PODVM_MAPS");
        std::env::remove_var("PODVM_VOLUME_BASE");
    }
    let r = f();
    for (k, v) in keep {
        if let Some(v) = v {
            unsafe { std::env::set_var(k, v) };
        }
    }
    r
}

/// `pin` of the host path `h` for one call, in a scope of its own: the
/// result, and the name it gave.
fn pin_of(h: &str, what: &[u8], follow: bool, writes: bool) -> Result<String, c_int> {
    let _s = crate::map::Scope::new();
    let t = unsafe { crate::map::table() };
    let mut out = [0u8; crate::map::OUT];
    out[..h.len()].copy_from_slice(h.as_bytes());
    match unsafe { pin(&t, what, follow, writes, &mut out, h.len()) } {
        Ok(Pin::Held(m)) => Ok(String::from_utf8_lossy(&out[..m]).into_owned()),
        Ok(Pin::Checked) => Ok("checked".into()),
        Ok(Pin::Apart) => Ok("apart".into()),
        Err(e) => Err(e),
    }
}

#[test]
fn a_volume_under_the_base_reaches_no_file_out_of_the_base() {
    let dir = std::env::temp_dir().join(format!("podvm-vbase-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (root, base, out) = (dir.join("root"), dir.join("base"), dir.join("out"));
    for d in ["root/etc", "base/w/sub", "base/w2", "out/sub"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    std::fs::write(base.join("w/file"), b"inside").unwrap();
    std::fs::write(base.join("w/sub/f"), b"inside-sub").unwrap();
    std::fs::write(base.join("w2/f"), b"inside-w2").unwrap();
    std::fs::write(out.join("f"), b"OUTSIDE").unwrap();
    std::fs::write(out.join("sub/f"), b"OUTSIDE").unwrap();
    std::os::unix::fs::symlink("file", base.join("w/rel")).unwrap();
    let (b, o) = (base.display().to_string(), out.display().to_string());
    // SAFETY: the interposer tests run with one thread (gate.yml).
    unsafe {
        std::env::set_var(
            "PODVM_MAPS",
            format!("/:{},/w:{b}/w,/w2:{b}/w2", root.display()),
        );
        std::env::set_var("PODVM_VOLUME_BASE", &b);
    }

    // A plain path reads and writes; a link inside the volume resolves.
    assert_eq!(cat(c"/w/file"), Ok("inside".into()), "a plain file");
    assert_eq!(cat(c"/w/sub/f"), Ok("inside-sub".into()), "a directory");
    assert_eq!(cat(c"/w/rel"), Ok("inside".into()), "a link in the volume");
    let fd = unsafe { crate::open(c"/w/new".as_ptr(), O_WRONLY | O_CREAT, 0o644) };
    assert!(fd >= 0, "a new file: errno {}", errno());
    assert_eq!(
        unsafe { write(fd, b"made".as_ptr() as *const c_void, 4) },
        4
    );
    unsafe { close(fd) };
    assert_eq!(host(|| std::fs::read(base.join("w/new")).unwrap()), b"made");
    let f = unsafe { crate::fopen(c"/w/fnew".as_ptr(), c"w".as_ptr()) };
    assert!(!f.is_null(), "fopen makes a file: errno {}", errno());
    unsafe { fclose(f) };
    assert!(
        base.join("w/fnew").is_file(),
        "fopen made the file in the volume"
    );
    assert_eq!(unsafe { crate::mkdir(c"/w/d".as_ptr(), 0o755) }, 0, "mkdir");
    assert_eq!(
        unsafe { crate::rename(c"/w/d".as_ptr(), c"/w/d2".as_ptr()) },
        0,
        "rename"
    );
    assert!(host(|| base.join("w/d2").is_dir()));
    let mut st = [0u8; 256];
    assert_eq!(
        unsafe { crate::lstat(c"/w".as_ptr(), st.as_mut_ptr() as *mut c_void) },
        0,
        "lstat of the volume itself"
    );
    let mut lb = [0u8; 64];
    let n = unsafe { crate::readlink(c"/w/rel".as_ptr(), lb.as_mut_ptr() as *mut c_char, 64) };
    assert_eq!(&lb[..n.max(0) as usize], b"file", "readlink keeps the link");

    // The host makes links under the base after the start.
    host(|| {
        std::os::unix::fs::symlink(&out, base.join("w/away")).unwrap();
        std::fs::remove_dir_all(base.join("w2")).unwrap();
        std::os::unix::fs::symlink(&out, base.join("w2")).unwrap();
    });
    for p in [c"/w/away/f", c"/w/away/sub/f", c"/w2/f", c"/w2/sub/f"] {
        assert_ne!(cat(p), Ok("OUTSIDE".into()), "{p:?}");
    }

    // OQ-11 of the sixth outside pass (TODO/enter.md T-2055): a link in a
    // volume to an absolute path resolves in guest names, as the kernel
    // resolves it in the root of the namespace rung: the link of the host
    // and the link of the payload to `/etc` read the image's file.
    host(|| {
        std::fs::write(root.join("etc/hostname"), b"image").unwrap();
        std::os::unix::fs::symlink("/etc", base.join("w/etcl")).unwrap();
    });
    assert_eq!(
        unsafe { crate::symlink(c"/etc".as_ptr(), c"/w/esc".as_ptr()) },
        0
    );
    for p in [c"/w/etcl/hostname", c"/w/esc/hostname"] {
        assert_eq!(cat(p), Ok("image".into()), "{p:?}");
    }

    // The class at the check itself, with each link a change of the host
    // between the walk and the call: (host path, follow, writes, call,
    // the answer).
    let held =
        |r: &Result<String, c_int>| r.as_ref().is_ok_and(|s| s.starts_with("/proc/self/fd/"));
    let cases: &[Case] = &[
        (format!("{b}/w/file"), true, false, b"open", Ok(())),
        (format!("{b}/w/sub/f"), false, false, b"lstat", Ok(())),
        (format!("{b}/w"), false, false, b"lstat", Ok(())),
        (format!("{b}/w/absent"), true, true, b"open", Ok(())),
        (format!("{b}/w/absent"), true, false, b"stat", Err(ENOENT)),
        (
            format!("{b}/w/absent"),
            true,
            true,
            b"truncate",
            Err(ENOENT),
        ),
        (format!("{b}/w/away/f"), true, false, b"open", Err(ELOOP)),
        (format!("{b}/w/away/f"), false, false, b"unlink", Err(ELOOP)),
        (format!("{b}/w/away"), true, false, b"stat", Err(ELOOP)),
        (format!("{b}/w2/f"), true, false, b"open", Err(ELOOP)),
        (format!("{b}/w2"), false, false, b"lstat", Err(ELOOP)),
        (format!("{b}/w/../w2/f"), true, false, b"open", Err(ELOOP)),
    ];
    for (h, follow, writes, what, want) in cases {
        let got = pin_of(h, what, *follow, *writes);
        let what = String::from_utf8_lossy(what);
        match want {
            Ok(()) => assert!(held(&got), "{what} {h}: {got:?}"),
            Err(e) => assert!(
                got == Err(*e) || got == Err(EXDEV),
                "{what} {h}: {got:?}, want errno {e}"
            ),
        }
    }
    // Out of the base, or a call that gives the name on: no pin.
    assert_eq!(
        pin_of(&format!("{o}/f"), b"open", true, false),
        Ok("apart".into())
    );
    assert_eq!(
        pin_of(&format!("{}/etc", root.display()), b"open", true, false),
        Ok("apart".into())
    );
    assert_eq!(
        pin_of(&format!("{b}/w/file"), b"execve", true, false),
        Ok("checked".into())
    );
    assert_eq!(
        pin_of(&format!("{b}/w/away/f"), b"execve", true, false),
        Err(ELOOP)
    );

    // An absent name opens by its directory, and a link that the host
    // makes there before the call is not followed (O_NOFOLLOW).
    {
        let _s = crate::map::Scope::new();
        let t = unsafe { crate::map::table() };
        let mut name = [0u8; crate::map::OUT];
        let h = format!("{b}/w/late");
        name[..h.len()].copy_from_slice(h.as_bytes());
        let r = unsafe { pin(&t, b"open", true, true, &mut name, h.len()) };
        assert!(matches!(r, Ok(Pin::Held(_))), "{r:?}");
        assert!(crate::map::take_by_name(), "the name of an absent file");
        host(|| std::os::unix::fs::symlink(out.join("late"), base.join("w/late")).unwrap());
        let fd = unsafe {
            crate::next_open().unwrap()(
                name.as_ptr() as *const c_char,
                O_WRONLY | O_CREAT | crate::oflags::O_NOFOLLOW,
                0o644,
            )
        };
        assert!(fd < 0 && errno() == ELOOP, "fd {fd}, errno {}", errno());
        assert!(
            !host(|| out.join("late").exists()),
            "no file out of the base"
        );
    }

    // The scope closes each descriptor that it held.
    let fd = {
        let _s = crate::map::Scope::new();
        let t = unsafe { crate::map::table() };
        let mut name = [0u8; crate::map::OUT];
        let h = format!("{b}/w/file");
        name[..h.len()].copy_from_slice(h.as_bytes());
        let _ = unsafe { pin(&t, b"open", true, false, &mut name, h.len()) };
        let s = String::from_utf8_lossy(&name[14..]).into_owned();
        let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        let fd: c_int = digits.parse().unwrap();
        assert!(unsafe { crate::map::base::is_held(name.as_ptr() as *const c_char) });
        assert!(unsafe { fcntl(fd, 1, 0) } >= 0, "held in the scope");
        fd
    };
    assert!(unsafe { fcntl(fd, 1, 0) } < 0, "closed after the scope");

    unsafe {
        std::env::remove_var("PODVM_MAPS");
        std::env::remove_var("PODVM_VOLUME_BASE");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
