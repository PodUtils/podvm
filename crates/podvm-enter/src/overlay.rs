//! TODO/enter.md T-2101 (PodUtils/podrunner#368): `-v HOST:GUEST:overlay`
//! on the namespace rung. HOST is the read-only lower layer of an overlayfs
//! mount on GUEST. The upper and the work directory are in the tree of the
//! container (`<store>/trees/<id>/overlay/<n>/`), beside its rootfs, so the
//! removal of the tree (`rm`, the end of `run --rm`) removes them too. The
//! kernel writes no file of HOST: a write, a `chmod`, an `unlink` or a
//! `rename` changes the upper layer, and a whiteout there hides a removed
//! file of HOST.
//!
//! podcrates' `pod-enter` binds a volume (`Plan::volumes`) and mounts an
//! overlay root, but no overlay volume. Until it does (the ask of T-2101),
//! podvm mounts each layer in a mount namespace that it owns, before the
//! entry of pod-enter, whose mount namespace copies the mount: pid 2 of a
//! foreground run ([`mount_all`]), and the launcher of `run -d` and `start`
//! ([`enter_own_namespace`], then [`mount_all`]). The options are pod-enter's
//! own ([`pod_enter::overlay_options`], [`pod_enter::USERXATTR`]): the lower
//! layer is `.`, the working directory, which is a held descriptor of HOST.

use std::path::{Path, PathBuf};

use pod_hostprobe::sys::{self, CBuf, Errno, OpenHow};

use crate::userland::Volume;

/// The word of the mode in `-v` and in `PODVM_BINDS`.
pub const MODE: &str = "overlay";

/// The directory of the layers in the tree of a container.
pub const DIR: &str = "overlay";

/// The directory of the layers of the overlay volume number `n` of the
/// tree whose rootfs is `rootfs`: `<tree>/overlay/<n>`.
pub fn layer_dir(rootfs: &Path, n: usize) -> Option<PathBuf> {
    Some(rootfs.parent()?.join(DIR).join(n.to_string()))
}

/// The guest paths under which the namespace rung mounts its own file
/// systems after the volumes: a layer there would be hidden.
const HIDDEN: [&str; 4] = ["/tmp", "/dev", "/proc", "/sys"];

fn at_or_under(path: &str, top: &str) -> bool {
    let (p, t) = (path.trim_end_matches('/'), top.trim_end_matches('/'));
    p == t || p.strip_prefix(t).is_some_and(|r| r.starts_with('/')) || t.is_empty()
}

/// Why the guest paths of a run cannot take an overlay volume, or `Ok`.
/// `overlays` are the guest paths of the overlay volumes, `all` the guest
/// paths of each volume. A layer shares no path with another volume: a
/// bind under a layer would be hidden by it on one route and not on the
/// other, and the payload never runs with a volume missing.
pub fn check_guests(overlays: &[String], all: &[String]) -> Result<(), String> {
    for g in overlays {
        if g.trim_end_matches('/').is_empty() {
            return Err(format!(
                "{g}: an overlay volume cannot replace the root `/`"
            ));
        }
        if let Some(top) = HIDDEN.iter().find(|t| at_or_under(g, t)) {
            return Err(format!(
                "{g}: the namespace rung mounts its own {top} after the volumes, which would \
                 hide the layer; give a guest path out of /tmp, /dev, /proc and /sys"
            ));
        }
        let shared = all
            .iter()
            .filter(|o| *o != g)
            .find(|o| at_or_under(o, g) || at_or_under(g, o));
        if let Some(o) = shared {
            return Err(format!(
                "{g}: the overlay volume shares a path with the volume at {o}; an overlay \
                 volume takes a guest path that no other volume is at, over or under"
            ));
        }
    }
    Ok(())
}

/// The line of an errno of a call.
fn errno(what: &str, e: Errno) -> String {
    format!(
        "{what}: {} ({})",
        std::io::Error::from_raw_os_error(e.0),
        e.name()
    )
}

/// A descriptor that closes when it goes.
struct Fd(i64);

impl Drop for Fd {
    fn drop(&mut self) {
        let _ = sys::close(self.0);
    }
}

fn cbuf(s: &str) -> Result<CBuf, String> {
    CBuf::new(s).ok_or_else(|| format!("{s:?} holds a NUL byte"))
}

/// Is this process in a user namespace that is not the initial one? Its
/// overlay mount then needs `userxattr`.
fn in_user_namespace() -> bool {
    std::fs::read_to_string("/proc/self/uid_map")
        .is_ok_and(|m| !pod_hostprobe::userns::is_initial_map(&m))
}

/// The launcher of `run -d` and `start`: a mount namespace of its own, with
/// each mount private, so that no layer reaches the host. A caller that is
/// not uid 0 of the initial user namespace first enters a user namespace
/// of its own, where it is uid 0 (`unshare -Ur`), and the namespace rung of
/// pod-enter then runs in it. The process must have one thread.
pub fn enter_own_namespace() -> Result<(), String> {
    // SAFETY: geteuid takes no argument and cannot fail.
    let euid = unsafe { sys::sys(sys::SYS_GETEUID, [0; 6]) }.unwrap_or(-1);
    if euid != 0 || in_user_namespace() {
        let maps = pod_hostprobe::userns::Maps::of_caller();
        pod_hostprobe::userns::enter(&maps).map_err(|(step, e)| errno(step.name(), e))?;
    }
    sys::unshare(sys::CLONE_NEWNS).map_err(|e| errno("unshare(CLONE_NEWNS)", e))?;
    let (none, slash) = (cbuf("none")?, cbuf("/")?);
    sys::mount_data(
        &none,
        &slash,
        &none,
        sys::MS_REC | sys::MS_PRIVATE,
        &sys::cempty(),
    )
    .map_err(|e| errno("the mounts of / as private", e))?;
    Ok(())
}

/// Mount each layer of `layers` on the tree `rootfs`, in order; the layer
/// `n` takes the directory [`layer_dir`] `n`. `base` is the base of
/// `--podvm-volume-base`: a HOST under it opens under it with no symbolic
/// link (`pod-beneath`), and the lower layer is that descriptor. The
/// caller owns its mount namespace.
pub fn mount_all(rootfs: &Path, layers: &[Volume], base: Option<&str>) -> Result<(), String> {
    let userxattr = in_user_namespace();
    for (n, v) in layers.iter().enumerate() {
        mount_one(rootfs, n, v, base, userxattr).map_err(|why| {
            format!(
                "-v {}:{}:{MODE}: {why} (TODO/enter.md T-2101)",
                v.host, v.guest
            )
        })?;
    }
    Ok(())
}

/// A held descriptor of HOST, a directory.
fn lower(host: &str, base: Option<&str>) -> Result<Fd, String> {
    let flags = sys::O_PATH | sys::O_DIRECTORY | sys::O_CLOEXEC;
    if let Some(b) = base {
        let mut buf = [0u8; 4096];
        match pod_beneath::open_under_base(
            &sys::Beneath,
            b.as_bytes(),
            host.as_bytes(),
            flags,
            &mut buf,
        ) {
            Ok(fd) => return Ok(Fd(i64::from(fd))),
            Err(pod_beneath::Refusal::NotUnderBase) => {}
            Err(r) => {
                return Err(format!(
                    "the lower layer does not open under its base {b} (--podvm-volume-base): {r}"
                ))
            }
        }
    }
    sys::open(&cbuf(host)?, flags, 0)
        .map(Fd)
        .map_err(|e| errno("the lower layer does not open as a directory", e))
}

/// A directory of the layers: made where it is missing; a link or a file
/// in its place is a refusal.
fn layer_part(p: &Path, mode: u32) -> Result<(), String> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => Ok(()),
        Ok(_) => Err(format!("{} is not a directory", p.display())),
        Err(_) => {
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(p)
                .map_err(|e| format!("{}: {e}", p.display()))?;
            // The mode of `mkdir` passes the umask.
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode))
                .map_err(|e| format!("{}: {e}", p.display()))
        }
    }
}

/// The mount point of `guest` in `rootfs`, one component at a time, with
/// no link of the image on the way: the tree came from an archive.
fn point(rootfs: &Path, guest: &str) -> Result<Fd, String> {
    let dir_flags = sys::O_PATH | sys::O_DIRECTORY | sys::O_CLOEXEC;
    let root = cbuf(&rootfs.to_string_lossy())?;
    let mut at =
        Fd(sys::open(&root, dir_flags | sys::O_NOFOLLOW, 0).map_err(|e| errno("the rootfs", e))?);
    let how = OpenHow {
        flags: dir_flags,
        mode: 0,
        resolve: sys::RESOLVE_IN_ROOT | sys::RESOLVE_NO_SYMLINKS,
    };
    for part in guest.split('/').filter(|p| !p.is_empty()) {
        if part == "." || part == ".." {
            return Err(format!("the guest path names {part}"));
        }
        let name = cbuf(part)?;
        let opened = match sys::openat2(at.0, &name, &how) {
            Err(e) if e == sys::ENOENT => {
                match sys::mkdirat(at.0, &name, 0o755) {
                    Err(e) if e != sys::EEXIST => {
                        return Err(errno(&format!("making {part} of {guest}"), e))
                    }
                    _ => {}
                }
                sys::openat2(at.0, &name, &how)
            }
            r => r,
        };
        let fd = opened.map_err(|e| {
            if e == sys::ELOOP || e == sys::EXDEV {
                format!("{part} of {guest} in the image is a symbolic link; podvm mounts no layer through one")
            } else {
                errno(&format!("{part} of {guest} is no directory of the image"), e)
            }
        })?;
        at = Fd(fd);
    }
    Ok(at)
}

fn mount_one(
    rootfs: &Path,
    n: usize,
    v: &Volume,
    base: Option<&str>,
    userxattr: bool,
) -> Result<(), String> {
    let low = lower(&v.host, base)?;
    let st = sys::fstatat(low.0, &sys::cempty(), sys::AT_EMPTY_PATH)
        .map_err(|e| errno("fstat of the lower layer", e))?;
    let dir = layer_dir(rootfs, n).ok_or("the rootfs has no parent directory")?;
    let top = dir.parent().ok_or("the layers have no parent directory")?;
    layer_part(top, 0o700)?;
    layer_part(&dir, 0o700)?;
    // The root of GUEST takes the mode of the upper directory.
    let (upper, work) = (dir.join("upper"), dir.join("work"));
    layer_part(&upper, st.st_mode & 0o7777)?;
    layer_part(&work, 0o700)?;
    let target = point(rootfs, &v.guest)?;
    let mut options = pod_enter::overlay_options(&upper.to_string_lossy(), &work.to_string_lossy());
    // In a user namespace the attributes are `user.overlay.*`: with no
    // `userxattr` the kernel reads no whiteout and no opaque directory.
    // There is no second try without it.
    if userxattr {
        options.push_str(pod_enter::USERXATTR);
    }
    if options.len() > pod_enter::OVERLAY_OPTIONS_MOST {
        return Err(format!(
            "the options of the mount are {} bytes, and mount(2) takes at most {}",
            options.len(),
            pod_enter::OVERLAY_OPTIONS_MOST
        ));
    }
    let (fstype, data) = (cbuf(MODE)?, cbuf(&options)?);
    let at = cbuf(&format!("/proc/self/fd/{}", target.0))?;
    // `lowerdir=.`: the working directory is the held lower descriptor for
    // the mount, and the old one comes back after it.
    let cwd = Fd(sys::open(
        &cbuf(".")?,
        sys::O_PATH | sys::O_DIRECTORY | sys::O_CLOEXEC,
        0,
    )
    .map_err(|e| errno("the working directory", e))?);
    sys::fchdir(low.0).map_err(|e| errno("fchdir to the lower layer", e))?;
    let mounted = sys::mount_data(&fstype, &at, &fstype, 0, &data);
    let back = sys::fchdir(cwd.0);
    mounted.map_err(|e| {
        // EBUSY (16): an upper directory in use, or overlapping layers.
        let hint = if e == sys::ELOOP || e == Errno(16) {
            "; the store and HOST must not hold each other"
        } else if e == sys::EINVAL || e == sys::EPERM {
            "; the kernel needs Linux 5.11 for an overlay in a user namespace, and the file \
             system of the store must take user.* attributes"
        } else {
            ""
        };
        format!("{}{hint}", errno("the overlayfs mount", e))
    })?;
    back.map(drop).map_err(|e| errno("fchdir back", e))
}

#[cfg(test)]
mod tests;
