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
) -> Result<(), String> {
    if !active() {
        return Ok(());
    }
    for v in volumes {
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
        let (Ok(src), Ok(dst)) = (
            std::ffi::CString::new(host.as_os_str().as_encoded_bytes()),
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
