//! TODO/cli.md T-2108 (OR-B42-02): `--read-only` on the namespace rung.
//!
//! The root of the payload is a bind of the tree onto itself, remounted
//! `MS_RDONLY`, in a mount namespace that podvm owns: pid 2 of a foreground
//! run of a caller that is not uid 0 (podvm's user namespace, call C21), a
//! foreground run of uid 0 (a mount namespace that podvm opens for it), and
//! the launcher of `run -d`, `create` and `start` (its own namespace, as for
//! an overlay volume). The namespace rung of pod-enter copies the mount, and
//! enters the tree by its path, so the payload lands on the read-only mount.
//! Each mount of the rung goes on top of it: the tmpfs on `/tmp`, `/proc`,
//! the binds of the volumes and of the devices, each `--tmpfs`, each with
//! its own mode. A mount point that pod-enter would make after the seal (a
//! volume or a `--tmpfs` that the image does not have) is made before it.
//! `/dev` of the tree stays writable, as docker's tmpfs on `/dev` is.
//!
//! The kernel locks the flag against the payload only where the payload is
//! in a user namespace under the one that owns the mount
//! (`crate::lock`, TODO/enter.md T-2105): a payload of root of the initial
//! user namespace can remount `/` writable.
//!
//! podcrates' `pod-enter` (`v0.86.0`) has no read-only root in its `Plan`.
//! The ask is a `Plan::read_only_root`, applied by its child after its own
//! mounts and before the nested user namespace and the root change, as
//! runc does. podvm takes it then, and drops [`seal`].

use std::path::Path;

use pod_hostprobe::sys;

use crate::overlay::{cbuf, errno, point, Fd};
use crate::{TmpfsMount, VolumeBind};

/// The variable of the record: `PODVM_RO_ROOT=1` where the entry makes the
/// root read-only by a mount. The entry removes it from the payload's
/// environment ([`take`]).
pub const VAR: &str = "PODVM_RO_ROOT";

/// True where `env` asks for the read-only root; the variable leaves `env`.
pub fn take(env: &mut Vec<String>) -> bool {
    let wanted = wanted(env);
    env.retain(|e| !e.starts_with(&format!("{VAR}=")));
    wanted
}

/// True where `env` asks for the read-only root.
pub fn wanted(env: &[String]) -> bool {
    env.iter().any(|e| e.strip_prefix(VAR) == Some("=1"))
}

/// The kind of a mount point that the seal makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Point {
    Dir(String),
    File(String),
}

fn at_or_under_tmp(guest: &str) -> bool {
    let g = guest.trim_end_matches('/');
    g == "/tmp" || g.starts_with("/tmp/")
}

/// The mount points of pod-enter's own mounts after the seal: each volume
/// and each `--tmpfs` of the plan, less those at or under `/tmp`, which go
/// on the tmpfs of the rung. A volume is a file where its source is no
/// directory (`is_dir` of the host path); a source that `is_dir` cannot
/// read gets no point, and pod-enter names it.
pub fn points(
    volumes: &[VolumeBind],
    tmpfs: &[TmpfsMount],
    is_dir: impl Fn(&str) -> Option<bool>,
) -> Vec<Point> {
    let mut out = Vec::new();
    for v in volumes.iter().filter(|v| !at_or_under_tmp(&v.guest)) {
        match is_dir(&v.host) {
            Some(true) => out.push(Point::Dir(v.guest.clone())),
            Some(false) => out.push(Point::File(v.guest.clone())),
            None => {}
        }
    }
    for t in tmpfs.iter().filter(|t| !at_or_under_tmp(&t.guest)) {
        out.push(Point::Dir(t.guest.clone()));
    }
    out
}

/// The mount namespace of a caller of uid 0 that has none of its own: a
/// copy of its current one, with each mount private, so that the bind of
/// [`seal`] reaches no other process. pid 2 of podvm's user namespace and
/// the launcher have one already.
pub fn own_mounts() -> Result<(), String> {
    sys::unshare(sys::CLONE_NEWNS).map_err(|e| errno("unshare(CLONE_NEWNS)", e))?;
    let (none, slash) = (cbuf("none")?, cbuf("/")?);
    sys::mount(&none, &slash, &none, sys::MS_REC | sys::MS_PRIVATE)
        .map(drop)
        .map_err(|e| errno("the mounts of / as private", e))
}

/// Make the root read-only: each mount point of [`points`], then a bind of
/// `rootfs` onto itself with each mount under it, remounted read-only with
/// each flag of the mount that a user namespace locks
/// (`pod_enter::volume_remount_flags`). The caller owns its mount
/// namespace, writes nothing to the tree after it, and opens the root of
/// the entry after it. An error is one line; the run must not start then.
pub fn seal(rootfs: &Path, volumes: &[VolumeBind], tmpfs: &[TmpfsMount]) -> Result<(), String> {
    let is_dir = |h: &str| std::fs::metadata(h).ok().map(|m| m.is_dir());
    for p in points(volumes, tmpfs, is_dir) {
        match p {
            Point::Dir(g) => drop(point(rootfs, &g)?),
            Point::File(g) => file_point(rootfs, &g)?,
        }
    }
    keep_dev_writable(rootfs)?;
    let path = cbuf(&rootfs.to_string_lossy())?;
    let dir = sys::O_PATH | sys::O_DIRECTORY | sys::O_NOFOLLOW | sys::O_CLOEXEC;
    let held = Fd(sys::open(&path, dir, 0).map_err(|e| errno("the rootfs", e))?);
    let source = cbuf(&format!("/proc/self/fd/{}", held.0))?;
    let empty = sys::cempty();
    sys::mount(&source, &path, &empty, sys::MS_BIND | sys::MS_REC)
        .map_err(|e| errno("the bind of the rootfs onto itself", e))?;
    let st = sys::statfs(&path).map_err(|e| errno("statfs of the bind", e))?;
    sys::mount(
        &empty,
        &path,
        &empty,
        pod_enter::volume_remount_flags(st.f_flags),
    )
    .map_err(|e| errno("the read-only remount of the rootfs", e))?;
    // Fail safe: a root that is not read-only after the remount stops the run.
    match sys::statfs(&path).map(|s| s.flag(sys::ST_RDONLY)) {
        Ok(Some(true)) => Ok(()),
        Ok(_) => Err("the rootfs is not read-only after its remount".to_string()),
        Err(e) => Err(errno("statfs after the remount", e)),
    }
}

/// `/dev` of the tree stays writable, as docker's tmpfs on `/dev` is and as
/// the read-only table of the interposer has it (`rw:/dev`, T-0401): a bind
/// of it onto itself, with the binds of the devices under it, which the
/// bind of the root carries and its remount leaves as they are. A tree
/// with no directory `/dev` gets none.
fn keep_dev_writable(rootfs: &Path) -> Result<(), String> {
    if !std::fs::symlink_metadata(rootfs.join("dev")).is_ok_and(|m| m.is_dir()) {
        return Ok(());
    }
    let dev = point(rootfs, "/dev")?;
    let at = cbuf(&format!("/proc/self/fd/{}", dev.0))?;
    sys::mount(&at, &at, &sys::cempty(), sys::MS_BIND | sys::MS_REC)
        .map(drop)
        .map_err(|e| errno("the bind of /dev onto itself", e))
}

/// The mount point of a volume of a file: its directory as [`point`] makes
/// it, then an empty file, never through a link of the image.
fn file_point(rootfs: &Path, guest: &str) -> Result<(), String> {
    let g = guest.trim_end_matches('/');
    let (parent, name) = g.rsplit_once('/').unwrap_or(("", g));
    if name.is_empty() || name == "." || name == ".." {
        return Err(format!("{guest}: no file name for the mount point"));
    }
    let dir = point(rootfs, parent)?;
    let create = sys::O_CREAT | sys::O_EXCL | sys::O_NOFOLLOW | sys::O_WRONLY | sys::O_CLOEXEC;
    match sys::openat(dir.0, &cbuf(name)?, create, 0o644) {
        Ok(fd) => drop(Fd(fd)),
        // A file of the image, or a link: pod-enter's open decides.
        Err(e) if e == sys::EEXIST => {}
        Err(e) => return Err(errno(&format!("making the mount point {guest}"), e)),
    }
    Ok(())
}

#[cfg(test)]
mod tests;
