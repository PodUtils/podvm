//! The host's view of a run in the user namespace: its pids and its
//! device nodes (TODO/probe.md T-1909).

use super::*;

/// A descriptor of the host's `/proc`, opened in pid 2 before its own
/// `/proc` covers it.
pub(super) static HOST_PROC: AtomicI32 = AtomicI32::new(-1);

/// The pid that the host gives the process `pid` of this namespace, or
/// `pid` where podvm entered no namespace. A pidfd of the process, read
/// through the `fdinfo` of the host's `/proc`, gives its `Pid:` in the
/// namespace of that `/proc`.
pub fn host_pid(pid: i64) -> Option<i64> {
    let host = HOST_PROC.load(Ordering::Relaxed);
    if !active() || host < 0 {
        return Some(pid);
    }
    const PIDFD_OPEN: i64 = 434;
    // SAFETY: pidfd_open takes two integers.
    let fd = unsafe { syscall(PIDFD_OPEN, pid, 0) };
    if fd < 0 {
        return None;
    }
    let text = std::fs::read_to_string(format!("/proc/self/fd/{host}/self/fdinfo/{fd}"));
    // SAFETY: fd is the pidfd of this call.
    unsafe { close(fd as i32) };
    text.ok()?
        .lines()
        .find_map(|l| l.strip_prefix("Pid:"))
        .and_then(|v| v.trim().parse().ok())
        .filter(|p: &i64| *p > 0)
}

/// The device nodes of the host on the tree of a run in the user
/// namespace: `mknod` is refused there, and the fixups of `pod-rootfs`
/// would leave a regular file in their place (a `/dev/urandom` that ends
/// after 1 MiB). A bind in the mount namespace of pid 2, which the mount
/// namespace of the payload copies; the removal of the tree takes it off
/// first (`podvm_extract::tree::unbind_devices`). A path that is a link or
/// no regular file of the tree is left alone.
pub fn bind_devices(rootfs: &std::path::Path) {
    if !active() {
        return;
    }
    const MS_BIND: u64 = 0x1000;
    let dev = rootfs.join("dev");
    if !std::fs::symlink_metadata(&dev).is_ok_and(|m| m.is_dir()) {
        return;
    }
    for d in podvm_extract::tree::BOUND_DEVICES {
        let target = rootfs.join(d);
        let host = std::path::Path::new("/").join(d);
        match std::fs::symlink_metadata(&target) {
            Ok(m) if !m.is_file() => continue,
            Ok(_) => {}
            Err(_) => {
                if std::fs::File::create(&target).is_err() {
                    continue;
                }
            }
        }
        let (Ok(src), Ok(dst)) = (
            std::ffi::CString::new(host.as_os_str().as_encoded_bytes()),
            std::ffi::CString::new(target.as_os_str().as_encoded_bytes()),
        ) else {
            continue;
        };
        // SAFETY: NUL-ended paths; a bind takes no type and no data.
        unsafe {
            mount(
                src.as_ptr(),
                dst.as_ptr(),
                core::ptr::null(),
                MS_BIND,
                core::ptr::null(),
            )
        };
    }
    new_pts(&dev);
}

/// PodUtils/podrunner#216: a new devpts instance for the tree and its
/// `ptmx` on `/dev/ptmx`, so that `openpty` works in a foreground run, with
/// the options of pod-enter's own instance (`pod_enter::DEVPTS_OPTIONS`)
/// that `run -d` gets. A link at the `/dev/ptmx` of the image is left
/// alone; a refusal of the kernel is one line.
///
/// The instance goes on the `/dev/pts` of the mount namespace of pid 2,
/// which the user namespace owns, so no mount reaches the host: pod-enter
/// binds that `/dev/pts` on the tree, and its `ptmx` goes on the `/dev/ptmx`
/// of the tree here.
fn new_pts(dev: &std::path::Path) {
    const MS_BIND: u64 = 0x1000;
    let pts = std::path::Path::new("/dev/pts");
    let ptmx = dev.join("ptmx");
    if !std::fs::symlink_metadata(pts).is_ok_and(|m| m.is_dir()) {
        return;
    }
    let (Ok(target), Ok(opts)) = (
        std::ffi::CString::new(pts.as_os_str().as_encoded_bytes()),
        std::ffi::CString::new(podvm_enter::DEVPTS_OPTIONS),
    ) else {
        return;
    };
    // SAFETY: NUL-ended strings; devpts reads its options as a C string.
    let r = unsafe {
        mount(
            c"devpts".as_ptr(),
            target.as_ptr(),
            c"devpts".as_ptr(),
            podvm_enter::DEVPTS_FLAGS,
            opts.as_ptr().cast(),
        )
    };
    if r != 0 {
        eprintln!(
            "podvm: the run has no terminal of its own: the mount of a new devpts \
             instance failed: {}",
            std::io::Error::last_os_error()
        );
        return;
    }
    match std::fs::symlink_metadata(&ptmx) {
        Ok(m) if m.file_type().is_symlink() => return,
        Ok(_) => {}
        Err(_) => {
            if std::fs::File::create(&ptmx).is_err() {
                return;
            }
        }
    }
    let (Ok(src), Ok(dst)) = (
        std::ffi::CString::new(pts.join("ptmx").as_os_str().as_encoded_bytes()),
        std::ffi::CString::new(ptmx.as_os_str().as_encoded_bytes()),
    ) else {
        return;
    };
    // SAFETY: NUL-ended paths; a bind takes no type and no data.
    let r = unsafe {
        mount(
            src.as_ptr(),
            dst.as_ptr(),
            core::ptr::null(),
            MS_BIND,
            core::ptr::null(),
        )
    };
    if r != 0 {
        eprintln!(
            "podvm: the run has no /dev/ptmx: the bind of the ptmx of its devpts failed: {}",
            std::io::Error::last_os_error()
        );
    }
}

/// TODO/enter.md T-1920: bind each volume onto the tree, in the mount
/// namespace of pid 2, so that the run keeps the namespace rung. The mounts
/// of pid 2 reach pod-enter's mount namespace, which copies them. A `:ro`
/// volume is remounted read-only, with the flags that the kernel locks in a
/// user namespace kept.
///
/// ⛔ No component of the guest path in the tree may be a symbolic link: the
/// tree came from a hostile archive, and a bind through a link would land
/// outside it. The removal of a tree unmounts each mount under it first
/// (`podvm_extract::live::unmount_under`).
pub fn bind_volumes(
    rootfs: &std::path::Path,
    volumes: &[podvm_enter::userland::Volume],
    ro: &[String],
    base: Option<&str>,
) -> Result<(), String> {
    if !active() {
        return Ok(());
    }
    for v in volumes {
        // PodUtils/podrunner#277: a source under `--podvm-volume-base`
        // opens under it with no symbolic link; the bind takes that
        // descriptor, so a link made after the check redirects nothing.
        let held = match base {
            Some(b) => {
                beneath(b, &v.host).map_err(|why| format!("-v {}:{}: {why}", v.host, v.guest))?
            }
            None => None,
        };
        let host = std::path::Path::new(&v.host);
        let meta = std::fs::metadata(host).map_err(|e| format!("-v {}: {e}", v.host))?;
        let rel = v.guest.trim_start_matches('/');
        let mut target = rootfs.to_path_buf();
        let parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty()).collect();
        for (i, part) in parts.iter().enumerate() {
            if matches!(*part, "." | "..") {
                return Err(format!("-v {}: the guest path names {part}", v.guest));
            }
            target.push(part);
            let last = i + 1 == parts.len();
            match std::fs::symlink_metadata(&target) {
                Ok(m) if m.file_type().is_symlink() => {
                    return Err(format!(
                        "-v {}: {} of the image is a symbolic link; podvm binds no volume through one",
                        v.guest,
                        target.strip_prefix(rootfs).unwrap_or(&target).display()
                    ));
                }
                Ok(_) => {}
                Err(_) if last && !meta.is_dir() => {
                    std::fs::File::create(&target).map_err(|e| format!("-v {}: {e}", v.guest))?;
                }
                Err(_) => {
                    std::fs::create_dir(&target).map_err(|e| format!("-v {}: {e}", v.guest))?
                }
            }
        }
        // PodUtils/podvm#565: a bind needs the same kind on both sides; the
        // kernel's "Not a directory" named neither path.
        if let Ok(t) = std::fs::metadata(&target)
            && t.is_dir() != meta.is_dir()
        {
            let kind = |d: bool| if d { "a directory" } else { "not a directory" };
            return Err(format!(
                "-v {}:{}: the host path is {} and {} of the image is {}; a volume \
                 binds a directory over a directory, or a file over a file or a new path",
                v.host,
                v.guest,
                kind(meta.is_dir()),
                v.guest,
                kind(t.is_dir())
            ));
        }
        let source = match &held {
            Some(fd) => std::path::PathBuf::from(format!("/proc/self/fd/{}", fd.0)),
            None => host.to_path_buf(),
        };
        let (Ok(src), Ok(dst)) = (
            std::ffi::CString::new(source.as_os_str().as_encoded_bytes()),
            std::ffi::CString::new(target.as_os_str().as_encoded_bytes()),
        ) else {
            return Err(format!("-v {}: a NUL byte in a path", v.guest));
        };
        const MS_RDONLY: u64 = 1;
        const MS_REMOUNT: u64 = 32;
        const MS_BIND: u64 = 0x1000;
        const MS_REC: u64 = 0x4000;
        // SAFETY: NUL-ended paths; a bind takes no type and no data.
        let r = unsafe {
            mount(
                src.as_ptr(),
                dst.as_ptr(),
                core::ptr::null(),
                MS_BIND | MS_REC,
                core::ptr::null(),
            )
        };
        if r != 0 {
            return Err(format!(
                "-v {}: bind: {}",
                v.guest,
                std::io::Error::last_os_error()
            ));
        }
        if ro.iter().any(|g| g == &v.guest) {
            let locked = locked_flags(&target);
            // SAFETY: as above.
            let r = unsafe {
                mount(
                    core::ptr::null(),
                    dst.as_ptr(),
                    core::ptr::null(),
                    MS_REMOUNT | MS_BIND | MS_RDONLY | locked,
                    core::ptr::null(),
                )
            };
            if r != 0 {
                return Err(format!(
                    "-v {}:ro: {}",
                    v.guest,
                    std::io::Error::last_os_error()
                ));
            }
        }
    }
    Ok(())
}

/// The flags of the mount at `p` that a remount in a user namespace must
/// keep (`nosuid`, `nodev`, `noexec` and the atime flags), as mount flags.
fn locked_flags(p: &std::path::Path) -> u64 {
    let info = std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
    let Ok(d) = std::fs::canonicalize(p) else {
        return 0;
    };
    let opts = info
        .lines()
        .rev()
        .find(|l| l.split(' ').nth(4).map(std::path::Path::new) == Some(d.as_path()))
        .and_then(|l| l.split(' ').nth(5).map(str::to_string))
        .unwrap_or_default();
    let mut f = 0;
    for (word, bit) in [
        ("nosuid", 2u64),
        ("nodev", 4),
        ("noexec", 8),
        ("noatime", 1024),
        ("nodiratime", 2048),
        ("relatime", 1 << 21),
    ] {
        if opts.split(',').any(|o| o == word) {
            f |= bit;
        }
    }
    f
}

/// A descriptor that closes when it goes.
struct Held(i32);

impl Drop for Held {
    fn drop(&mut self) {
        // SAFETY: the descriptor of this value, closed once.
        unsafe { close(self.0) };
    }
}

/// TODO/enter.md T-1972: the check at the start of the loader family of
/// the userland rung, whose interposer opens each volume under the base
/// again at each call. `Ok(())` where `host` is not under `base`, or opens
/// under it as a directory with no symbolic link; else the refusal.
pub fn check_volume_under_base(base: &str, host: &str) -> Result<(), String> {
    let Some(held) = beneath(base, host)? else {
        return Ok(());
    };
    let dir = std::fs::metadata(format!("/proc/self/fd/{}", held.0)).map(|m| m.is_dir());
    match dir {
        Ok(true) => Ok(()),
        Ok(false) => Err(format!(
            "the volume is refused: the loader family holds a directory under its base {} \
             (--podvm-volume-base), and its host path is not one",
            base.trim_end_matches('/')
        )),
        Err(e) => Err(format!(
            "the volume is refused: its host path under its base {} does not answer ({e})",
            base.trim_end_matches('/')
        )),
    }
}

/// `host` opened with `O_PATH` under `base` with no symbolic link
/// (`openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)`), or `None` where
/// `host` is not under `base` (PodUtils/podrunner#277).
fn beneath(base: &str, host: &str) -> Result<Option<Held>, String> {
    let b = base.trim_end_matches('/');
    let rel = if host == b {
        "."
    } else {
        match host.strip_prefix(b).and_then(|r| r.strip_prefix('/')) {
            Some(r) if !r.is_empty() => r,
            _ => return Ok(None),
        }
    };
    const O_PATH: i32 = 0o10000000;
    const O_DIRECTORY: i32 = 0o200000;
    const O_CLOEXEC: i32 = 0o2000000;
    const RESOLVE_NO_SYMLINKS: u64 = 0x04;
    const RESOLVE_BENEATH: u64 = 0x08;
    const SYS_OPENAT2: i64 = 437;
    let (Ok(cb), Ok(cr)) = (std::ffi::CString::new(b), std::ffi::CString::new(rel)) else {
        return Err("a NUL byte in a path".to_string());
    };
    // SAFETY: a NUL-ended path and integer flags.
    let bfd = unsafe { open(cb.as_ptr(), O_PATH | O_DIRECTORY | O_CLOEXEC) };
    if bfd < 0 {
        return Err(format!(
            "the volume base {b} does not open: {}",
            std::io::Error::last_os_error()
        ));
    }
    let bfd = Held(bfd);
    let how: [u64; 3] = [
        (O_PATH | O_CLOEXEC) as u64,
        0,
        RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS,
    ];
    // SAFETY: openat2 reads the NUL-ended path and the 24-byte `open_how`.
    let fd = unsafe {
        syscall(
            SYS_OPENAT2,
            bfd.0 as i64,
            cr.as_ptr(),
            how.as_ptr(),
            24usize,
        )
    };
    if fd < 0 {
        let e = std::io::Error::last_os_error();
        let why = match e.raw_os_error() {
            Some(40) | Some(18) => "holds a symbolic link".to_string(), // ELOOP, EXDEV
            _ => format!("does not open ({e})"),
        };
        return Err(format!(
            "the volume is refused: its host path {why} under its base {b} (--podvm-volume-base)"
        ));
    }
    Ok(Some(Held(fd as i32)))
}

#[cfg(test)]
mod tests {
    /// T-1972: at the start of the loader family, each volume under the
    /// base opens under it as a directory with no symbolic link; a path out
    /// of the base is no business of the base.
    #[test]
    fn a_volume_of_the_loader_family_opens_under_its_base_as_a_directory() {
        let d = std::env::temp_dir().join(format!("podvm-hostbase-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (base, out) = (d.join("base"), d.join("out"));
        std::fs::create_dir_all(base.join("work/sub")).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(base.join("file"), b"f").unwrap();
        std::os::unix::fs::symlink(&out, base.join("away")).unwrap();
        std::os::unix::fs::symlink(base.join("work"), base.join("near")).unwrap();
        let b = base.to_str().unwrap();
        let s = |p: &str| format!("{b}/{p}");
        for (host, ok, why) in [
            (s("work"), true, ""),
            (s("work/sub"), true, ""),
            (b.to_string(), true, ""),
            (out.to_str().unwrap().to_string(), true, ""),
            (s("away"), false, "symbolic link"),
            (s("near/sub"), false, "symbolic link"),
            (s("file"), false, "directory"),
            (s("absent"), false, "does not open"),
        ] {
            let r = super::check_volume_under_base(b, &host);
            assert_eq!(r.is_ok(), ok, "{host}: {r:?}");
            if let Err(e) = r {
                assert!(e.contains(why) && e.contains(b), "{host}: {e}");
            }
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
