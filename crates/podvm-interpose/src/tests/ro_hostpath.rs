//! TODO/enter.md T-2040 (the sixth outside pass, OQ-01): a host path of a
//! volume takes the rules of that volume. One rule for the class: each
//! call that changes a file under the host path of a `:ro` volume (also
//! through a link, a relative name after `chdir`, or a template) answers
//! `EROFS` and leaves the host file as it was, as the guest path does; the
//! host path of a rw volume writes; a host path of the rootfs or of a
//! library directory passes with no second prefix (T-1705), and takes
//! `--read-only` by its guest path.

use super::*;

unsafe extern "C" {
    fn close(fd: c_int) -> c_int;
    fn read(fd: c_int, buf: *mut c_void, n: usize) -> isize;
    fn fclose(f: *mut c_void) -> c_int;
}

const O_WRONLY: c_int = 0o1;
const O_RDWR: c_int = 0o2;
const O_CREAT: c_int = 0o100;
const O_APPEND: c_int = 0o2000;
const EROFS: c_int = crate::map::EROFS;

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

/// `f` as a process of the host: the test binary exports the interposed
/// calls, so the variables leave for the time of `f`.
fn host<T>(f: impl FnOnce() -> T) -> T {
    let keep: Vec<_> = ["PODVM_MAPS", "PODVM_RO"]
        .iter()
        .map(|k| (k, std::env::var_os(k)))
        .collect();
    // SAFETY: the interposer tests run with one thread (gate.yml).
    unsafe {
        std::env::remove_var("PODVM_MAPS");
        std::env::remove_var("PODVM_RO");
    }
    let r = f();
    for (k, v) in keep {
        if let Some(v) = v {
            unsafe { std::env::set_var(k, v) };
        }
    }
    r
}

/// An `open` of `p` through the interposer: the descriptor closed, or the
/// errno.
fn open(p: &str, flags: c_int) -> Result<(), c_int> {
    clear_errno();
    let fd = unsafe { crate::open(c(p).as_ptr(), flags, 0o644) };
    if fd < 0 {
        return Err(errno());
    }
    unsafe { close(fd) };
    Ok(())
}

/// The first bytes of `p` through the interposer, or the errno.
fn cat(p: &str) -> Result<String, c_int> {
    clear_errno();
    let fd = unsafe { crate::open(c(p).as_ptr(), 0, 0) };
    if fd < 0 {
        return Err(errno());
    }
    let mut b = [0u8; 64];
    let n = unsafe { read(fd, b.as_mut_ptr() as *mut c_void, 64) };
    unsafe { close(fd) };
    Ok(String::from_utf8_lossy(&b[..n.max(0) as usize]).into_owned())
}

/// A call that answers an `int`: `Ok` at 0 or more, else the errno.
fn int(r: c_int) -> Result<(), c_int> {
    if r < 0 {
        Err(errno())
    } else {
        Ok(())
    }
}

/// One case: its name, the call, and the answer.
type Case = (
    &'static str,
    Box<dyn Fn() -> Result<(), c_int>>,
    Result<(), c_int>,
);

#[test]
fn a_host_path_of_a_volume_takes_the_rules_of_the_volume() {
    let dir = std::env::temp_dir().join(format!("podvm-t2040-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for d in ["root/etc", "root/tmp", "root/lib", "ro/sub", "rw"] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
    }
    std::fs::write(dir.join("root/etc/hostname"), b"image").unwrap();
    std::fs::write(dir.join("root/lib/libx.so"), b"lib").unwrap();
    std::fs::write(dir.join("ro/f"), b"orig").unwrap();
    std::fs::write(dir.join("ro/k"), b"keep").unwrap();
    let (root, ro, rw) = (dir.join("root"), dir.join("ro"), dir.join("rw"));
    let (r, h, w) = (
        root.display().to_string(),
        ro.display().to_string(),
        rw.display().to_string(),
    );
    let cwd = std::env::current_dir().unwrap();
    // SAFETY: the interposer tests run with one thread (gate.yml).
    unsafe {
        std::env::set_var("PODVM_MAPS", format!("/:{r},/cfg:{h},/w:{w}"));
        std::env::set_var("PODVM_RO", "ro:/cfg");
    }
    let f = format!("{h}/f");
    let cases: Vec<Case> = vec![
        // The host path of the `:ro` volume: each call that changes it.
        (
            "open for write",
            {
                let f = f.clone();
                Box::new(move || open(&f, O_WRONLY))
            },
            Err(EROFS),
        ),
        (
            "open for append",
            {
                let f = f.clone();
                Box::new(move || open(&f, O_WRONLY | O_APPEND))
            },
            Err(EROFS),
        ),
        (
            "create",
            {
                let h = h.clone();
                Box::new(move || open(&format!("{h}/new"), O_WRONLY | O_CREAT))
            },
            Err(EROFS),
        ),
        (
            "create in a directory",
            {
                let h = h.clone();
                Box::new(move || open(&format!("{h}/sub/new"), O_RDWR | O_CREAT))
            },
            Err(EROFS),
        ),
        (
            "fopen w",
            {
                let f = f.clone();
                Box::new(move || {
                    let p = unsafe { crate::fopen(c(&f).as_ptr(), c"w".as_ptr()) };
                    if p.is_null() {
                        return Err(errno());
                    }
                    unsafe { fclose(p) };
                    Ok(())
                })
            },
            Err(EROFS),
        ),
        (
            "mkstemp",
            {
                let h = h.clone();
                Box::new(move || {
                    let mut t = format!("{h}/tXXXXXX\0").into_bytes();
                    int(unsafe { crate::mkstemp(t.as_mut_ptr() as *mut c_char) })
                })
            },
            Err(EROFS),
        ),
        (
            "mkdir",
            {
                let h = h.clone();
                Box::new(move || int(unsafe { crate::mkdir(c(&format!("{h}/d")).as_ptr(), 0o755) }))
            },
            Err(EROFS),
        ),
        (
            "unlink",
            {
                let h = h.clone();
                Box::new(move || int(unsafe { crate::unlink(c(&format!("{h}/k")).as_ptr()) }))
            },
            Err(EROFS),
        ),
        (
            "rename out",
            {
                let h = h.clone();
                Box::new(move || {
                    int(unsafe { crate::rename(c(&format!("{h}/k")).as_ptr(), c"/tmp/k".as_ptr()) })
                })
            },
            Err(EROFS),
        ),
        (
            "rename in",
            {
                let h = h.clone();
                Box::new(move || {
                    int(unsafe { crate::rename(c"/w/x".as_ptr(), c(&format!("{h}/x")).as_ptr()) })
                })
            },
            Err(EROFS),
        ),
        (
            "chmod",
            {
                let f = f.clone();
                Box::new(move || int(unsafe { crate::chmod(c(&f).as_ptr(), 0o600) }))
            },
            Err(EROFS),
        ),
        (
            "utimes",
            {
                let f = f.clone();
                Box::new(move || int(unsafe { crate::utimes(c(&f).as_ptr(), core::ptr::null()) }))
            },
            Err(EROFS),
        ),
        (
            "truncate",
            {
                let f = f.clone();
                Box::new(move || int(unsafe { crate::truncate(c(&f).as_ptr(), 0) }))
            },
            Err(EROFS),
        ),
        (
            "link from",
            {
                let f = f.clone();
                Box::new(move || int(unsafe { crate::link(c(&f).as_ptr(), c"/tmp/hard".as_ptr()) }))
            },
            Err(EROFS),
        ),
        (
            "link into",
            {
                let h = h.clone();
                Box::new(move || {
                    int(unsafe { crate::link(c"/w/x".as_ptr(), c(&format!("{h}/hard")).as_ptr()) })
                })
            },
            Err(EROFS),
        ),
        (
            "symlink into",
            {
                let h = h.clone();
                Box::new(move || {
                    int(unsafe { crate::symlink(c"/etc".as_ptr(), c(&format!("{h}/l")).as_ptr()) })
                })
            },
            Err(EROFS),
        ),
        // A link whose target is the host path, or the guest path.
        (
            "a link to the host path",
            {
                let f = f.clone();
                Box::new(move || {
                    int(unsafe { crate::symlink(c(&f).as_ptr(), c"/tmp/lh".as_ptr()) })?;
                    open("/tmp/lh", O_WRONLY)
                })
            },
            Err(EROFS),
        ),
        (
            "a link to the guest path",
            Box::new(|| {
                int(unsafe { crate::symlink(c"/cfg/f".as_ptr(), c"/tmp/lg".as_ptr()) })?;
                open("/tmp/lg", O_WRONLY)
            }),
            Err(EROFS),
        ),
        (
            "a link to the directory",
            Box::new(|| {
                int(unsafe { crate::symlink(c"/cfg".as_ptr(), c"/tmp/ld".as_ptr()) })?;
                int(unsafe { crate::unlink(c"/tmp/ld/k".as_ptr()) })
            }),
            Err(EROFS),
        ),
        (
            "a relative name after chdir",
            {
                let h = h.clone();
                Box::new(move || {
                    int(unsafe { crate::chdir(c(&h).as_ptr()) })?;
                    open("f", O_WRONLY)
                })
            },
            Err(EROFS),
        ),
        // The guest path, as before; a read passes.
        (
            "the guest path",
            Box::new(|| open("/cfg/f", O_WRONLY)),
            Err(EROFS),
        ),
        (
            "a read of the host path",
            {
                let f = f.clone();
                Box::new(move || cat(&f).map(|s| assert_eq!(s, "orig")))
            },
            Ok(()),
        ),
        (
            "a read of the guest path",
            Box::new(|| cat("/cfg/f").map(|s| assert_eq!(s, "orig"))),
            Ok(()),
        ),
        // The host path of a rw volume writes.
        (
            "rw: create",
            {
                let w = w.clone();
                Box::new(move || open(&format!("{w}/x"), O_WRONLY | O_CREAT))
            },
            Ok(()),
        ),
        (
            "rw: rename",
            {
                let w = w.clone();
                Box::new(move || {
                    int(unsafe {
                        crate::rename(c(&format!("{w}/x")).as_ptr(), c(&format!("{w}/y")).as_ptr())
                    })
                })
            },
            Ok(()),
        ),
        // T-1705: a host path of the rootfs and of a library directory.
        (
            "rootfs: read",
            {
                let r = r.clone();
                Box::new(move || cat(&format!("{r}/etc/hostname")).map(|s| assert_eq!(s, "image")))
            },
            Ok(()),
        ),
        (
            "library directory: read",
            {
                let r = r.clone();
                Box::new(move || cat(&format!("{r}/lib/libx.so")).map(|s| assert_eq!(s, "lib")))
            },
            Ok(()),
        ),
        (
            "rootfs: create",
            {
                let r = r.clone();
                Box::new(move || open(&format!("{r}/tmp/made"), O_WRONLY | O_CREAT))
            },
            Ok(()),
        ),
    ];
    let mut failed = Vec::new();
    for (name, call, want) in &cases {
        let got = call();
        if got != *want {
            failed.push(format!("{name}: {got:?}, want {want:?}"));
        }
    }
    host(|| std::env::set_current_dir(&cwd).unwrap());
    // `--read-only`: a host path of the rootfs takes the table by its
    // guest path; `/tmp` stays writable.
    unsafe { std::env::set_var("PODVM_RO", "ro:/,rw:/tmp,rw:/w") };
    for (name, p, want) in [
        (
            "--read-only: the rootfs by its host path",
            format!("{r}/etc/new"),
            Err(EROFS),
        ),
        (
            "--read-only: /tmp by its host path",
            format!("{r}/tmp/new"),
            Ok(()),
        ),
        (
            "--read-only: a rw volume by its host path",
            format!("{w}/z"),
            Ok(()),
        ),
    ] {
        let got = open(&p, O_WRONLY | O_CREAT);
        if got != want {
            failed.push(format!("{name}: {got:?}, want {want:?}"));
        }
    }
    host(|| {
        assert!(failed.is_empty(), "{failed:#?}");
        // The host files of the `:ro` volume are as they were.
        assert_eq!(std::fs::read(ro.join("f")).unwrap(), b"orig");
        assert_eq!(std::fs::read(ro.join("k")).unwrap(), b"keep");
        let mut names: Vec<_> = std::fs::read_dir(&ro)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, ["f", "k", "sub"], "no file made in the volume");
        assert!(std::fs::read_dir(ro.join("sub")).unwrap().next().is_none());
        // The rw volume and the rootfs took the writes, with one prefix.
        assert!(rw.join("y").is_file() && rw.join("z").is_file());
        assert!(root.join("tmp/made").is_file() && root.join("tmp/new").is_file());
        assert!(!root.join("etc/new").exists());
        let _ = std::fs::remove_dir_all(&dir);
    });
    unsafe {
        std::env::remove_var("PODVM_MAPS");
        std::env::remove_var("PODVM_RO");
    }
}
