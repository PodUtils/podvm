//! The device nodes of the host on the tree of a run whose mount namespace a
//! user namespace owns (TODO/probe.md T-1909, TODO/enter.md T-2107).
//!
//! `mknod` is refused in a user namespace, so a tree that `pod-rootfs`
//! completed there holds regular files at `/dev/null`, `/dev/urandom` and
//! the others: a read of `/dev/urandom` ends after the bytes of the file.
//! [`bind`] binds the host's character devices on them and gives the tree a
//! new devpts instance with its `ptmx` on `/dev/ptmx`, in the mount
//! namespace of the caller, which the mount namespace of the payload
//! copies. No mount reaches the host.
//!
//! Two callers: pid 2 of a foreground run (`podvm::userns::bind_devices`)
//! and the launcher of `run -d`, `create` and `start` with an `:overlay`
//! volume ([`crate::overlay::prepare`]), whose namespace is its own: there
//! `pod-enter` makes no bind of the devices, because its own `unshare`
//! works and it takes the user-namespace path (`spawn_own`, `m.devices`)
//! only at `EPERM`. The choice at each path is `pod_enter::choose`, the
//! one of `pod-enter`; the list is the caller's (`pod_enter::DEVICES` for
//! the launcher).

use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::Path;

use pod_enter::{Choice, Kind};
use pod_hostprobe::sys::{self, CBuf};

/// `ptmx` of the new devpts instance, the name under `/dev` of the image.
const PTMX: &str = "ptmx";

/// The kind of `path` in the image, not followed.
fn kind_in_image(path: &Path) -> Kind {
    match std::fs::symlink_metadata(path) {
        Ok(m) => pod_enter::kind_of(m.mode()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Kind::Missing,
        Err(_) => Kind::Unreadable,
    }
}

/// A bind of `source` on `target`, or the line of its refusal.
fn bind_one(source: &Path, target: &Path) -> Result<(), String> {
    let (src, dst) = match (
        source.to_str().and_then(CBuf::new),
        target.to_str().and_then(CBuf::new),
    ) {
        (Some(s), Some(d)) => (s, d),
        _ => return Err("a path is not UTF-8 or holds a NUL byte".to_string()),
    };
    sys::mount_data(&src, &dst, &sys::cempty(), sys::MS_BIND, &sys::cempty())
        .map(drop)
        .map_err(|e| format!("{} ({})", std::io::Error::from_raw_os_error(e.0), e.name()))
}

/// Bind the host's device `/dev/NAME` on `rootfs/dev/NAME` for each name of
/// `names`, then the new devpts instance ([`new_pts`]). Each line of the
/// result tells a path that stays as the image has it, with the cause; the
/// caller writes them. `rootfs/dev` must be a directory of the image, not
/// a link. The caller owns its mount namespace and its user namespace: the
/// mounts are not undone here.
pub fn bind(rootfs: &Path, names: &[&str]) -> Vec<String> {
    let dev = rootfs.join("dev");
    let mut lines = nodes(rootfs, names);
    if std::fs::symlink_metadata(&dev).is_ok_and(|m| m.is_dir()) {
        lines.extend(new_pts(&dev));
    }
    lines
}

/// The binds of the devices of [`bind`], with no devpts.
fn nodes(rootfs: &Path, names: &[&str]) -> Vec<String> {
    let dev = rootfs.join("dev");
    if !std::fs::symlink_metadata(&dev).is_ok_and(|m| m.is_dir()) {
        let line = pod_enter::no_dev_line(&crate::NAMES);
        return vec![line.trim_end().to_string()];
    }
    let mut lines = Vec::new();
    for name in names {
        let source = Path::new("/dev").join(name);
        let target = dev.join(name);
        let host_char = std::fs::metadata(&source).is_ok_and(|m| m.file_type().is_char_device());
        match pod_enter::choose(kind_in_image(&target), host_char) {
            Choice::Skip(why) => {
                let line = pod_enter::ns_dev_skipped_line(&crate::NAMES, name, why);
                lines.push(line.trim_end().to_string());
            }
            Choice::Bind { create } => {
                // `create_new` is `O_CREAT | O_EXCL`: it never follows a link.
                if create
                    && std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&target)
                        .is_err()
                {
                    lines.push(refused(name, "the mount point could not be made"));
                    continue;
                }
                if let Err(why) = bind_one(&source, &target) {
                    lines.push(refused(name, &format!("the bind failed: {why}")));
                }
            }
        }
    }
    lines
}

fn refused(name: &str, why: &str) -> String {
    format!("podvm: the namespace rung keeps the image's /dev/{name}: {why}")
}

/// PodUtils/podrunner#216: a new devpts instance for the tree and its
/// `ptmx` on `/dev/ptmx`, so that `openpty` works, with the options of
/// pod-enter's own instance (`pod_enter::DEVPTS_OPTIONS`). A link at the
/// `/dev/ptmx` of the image is left alone.
///
/// The instance goes on the `/dev/pts` of the mount namespace of the
/// caller, which its user namespace owns, so no mount reaches the host:
/// pod-enter binds that `/dev/pts` on the tree, and its `ptmx` goes on the
/// `/dev/ptmx` of the tree here.
fn new_pts(dev: &Path) -> Vec<String> {
    let pts = Path::new("/dev/pts");
    let ptmx = dev.join(PTMX);
    if !std::fs::symlink_metadata(pts).is_ok_and(|m| m.is_dir()) {
        return Vec::new();
    }
    let (Some(target), Some(opts), Some(devpts)) = (
        pts.to_str().and_then(CBuf::new),
        CBuf::new(pod_enter::DEVPTS_OPTIONS),
        CBuf::new("devpts"),
    ) else {
        return Vec::new();
    };
    if let Err(e) = sys::mount_data(&devpts, &target, &devpts, pod_enter::DEVPTS_FLAGS, &opts) {
        return vec![format!(
            "podvm: the run has no terminal of its own: the mount of a new devpts instance \
             failed: {}",
            std::io::Error::from_raw_os_error(e.0)
        )];
    }
    match std::fs::symlink_metadata(&ptmx) {
        Ok(m) if m.file_type().is_symlink() => return Vec::new(),
        Ok(_) => {}
        Err(_) => {
            if std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&ptmx)
                .is_err()
            {
                return Vec::new();
            }
        }
    }
    match bind_one(&pts.join(PTMX), &ptmx) {
        Ok(()) => Vec::new(),
        Err(why) => vec![format!(
            "podvm: the run has no /dev/ptmx: the bind of the ptmx of its devpts failed: {why}"
        )],
    }
}

#[cfg(test)]
mod tests;
