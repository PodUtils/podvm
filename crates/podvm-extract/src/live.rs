//! What tells a removal that a tree is in use, across PID namespaces
//! (podvm's TODO/probe.md T-1909: a foreground run can live in a user and
//! a PID namespace of its own).
//!
//! * A fill (`.fill-PID-ID`) holds an `flock` on `.fill-PID-ID.lock` beside
//!   it while it copies. A sweep of another PID namespace cannot read the
//!   pid of the name, and took a live fill for a dead one.
//! * A run in a user namespace binds four device nodes of the host onto
//!   its tree; a removal takes them off first.

use std::fs;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

unsafe extern "C" {
    fn flock(fd: i32, op: i32) -> i32;
    fn umount2(target: *const std::ffi::c_char, flags: i32) -> i32;
}
const LOCK_EX: i32 = 2;
const LOCK_NB: i32 = 4;

/// The lock of a fill, held until drop.
pub struct FillLock {
    _file: fs::File,
    path: PathBuf,
}

fn lock_path(partial: &Path) -> PathBuf {
    let mut p = partial.as_os_str().to_owned();
    p.push(".lock");
    PathBuf::from(p)
}

impl FillLock {
    /// Make and lock the file of the fill `partial`.
    pub fn take(partial: &Path) -> std::io::Result<FillLock> {
        let path = lock_path(partial);
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        // SAFETY: a descriptor of this call.
        if unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(FillLock { _file: file, path })
    }

    /// `Some(true)` where a process holds the lock of the fill `partial`,
    /// `Some(false)` where its lock file is free, `None` where it has none.
    pub fn held(partial: &Path) -> Option<bool> {
        let file = fs::OpenOptions::new()
            .write(true)
            .open(lock_path(partial))
            .ok()?;
        // SAFETY: a descriptor of this call; a free lock is released with it.
        Some(unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) } != 0)
    }

    /// Remove the lock file of a fill that a sweep removed.
    pub fn forget(partial: &Path) {
        let _ = fs::remove_file(lock_path(partial));
    }
}

impl Drop for FillLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// The device nodes that a run in a user namespace binds from the host
/// onto its tree.
pub const BOUND_DEVICES: [&str; 4] = ["dev/null", "dev/zero", "dev/random", "dev/urandom"];

/// Take the binds of [`BOUND_DEVICES`] off the tree `dir` (its `rootfs/`)
/// before its removal: a mount point refuses `unlink` with `EBUSY`. Where
/// nothing is mounted, or this process may not unmount, it does nothing.
/// `UMOUNT_NOFOLLOW`: a link of the payload never aims it out of the tree.
pub fn unbind_devices(dir: &Path) {
    unmount_under(dir);
    unbind_devices_at(&dir.join("rootfs"));
}

/// Take the binds of [`BOUND_DEVICES`] off the root `rootfs` itself, in
/// the mount namespace of this process. TODO/cli.md T-2087 (OR-B40-03):
/// the copy of the tree for the rundir, cache and tmpfs rungs reads a
/// bound device as the file that it covers, and `copy` of a device fails.
pub fn unbind_devices_at(rootfs: &Path) {
    const MNT_DETACH: i32 = 2;
    const UMOUNT_NOFOLLOW: i32 = 8;
    // `dev/ptmx`: the bind of the `ptmx` of the run's own devpts
    // (`userns::new_pts`, T-1909).
    for d in BOUND_DEVICES.iter().copied().chain(["dev/ptmx"]) {
        let path = rootfs.join(d);
        let Ok(c) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) else {
            continue;
        };
        // SAFETY: a NUL-ended path.
        unsafe { umount2(c.as_ptr(), MNT_DETACH | UMOUNT_NOFOLLOW) };
    }
}

/// TODO/probe.md T-1911: the removal of a tree in a user namespace with
/// the subordinate range of the caller, which podvm registers. A run with
/// that range leaves directories of subordinate ids, whose entries the
/// caller cannot remove from outside.
pub static RANGE_REMOVER: std::sync::OnceLock<fn(&std::path::Path) -> bool> =
    std::sync::OnceLock::new();

/// Remove the tree `dir` (the owner's bits added to each directory first);
/// where that fails, through [`RANGE_REMOVER`].
pub fn remove_any(dir: &std::path::Path) -> std::io::Result<bool> {
    match remove_plain(dir) {
        Ok(b) => Ok(b),
        Err(e) => match RANGE_REMOVER.get() {
            Some(f) if f(dir) => Ok(true),
            _ => Err(e),
        },
    }
}

/// The mount points of this process at `dir` or under it, from
/// `/proc/self/mountinfo` (field 5, with its octal escapes undone).
pub fn mounts_under(dir: &Path) -> Vec<std::path::PathBuf> {
    let Ok(d) = std::fs::canonicalize(dir) else {
        return Vec::new();
    };
    let info = std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
    info.lines()
        .filter_map(|l| l.split(' ').nth(4).map(unescape))
        .map(std::path::PathBuf::from)
        .filter(|m| m.starts_with(&d))
        .collect()
}

/// A path of mountinfo: `\040` is a space, and each `\NNN` an octal byte.
fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\'
            && i + 3 < b.len()
            && b[i + 1..i + 4].iter().all(|c| (b'0'..=b'7').contains(c))
        {
            out.push((b[i + 1] - b'0') * 64 + (b[i + 2] - b'0') * 8 + (b[i + 3] - b'0'));
            i += 4;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// TODO/enter.md T-1920: detach each mount at `dir` or under it, the
/// deepest first. True where none stays. A volume of a run is a bind of a
/// host directory into the tree: a removal that walked into it would remove
/// the files of the host.
///
/// TODO/cli.md T-2108: a mount can hide another at the same path (the
/// read-only bind of a root carries a copy of each mount under it), and an
/// unmount by path reaches the top one only. The passes go on while each
/// one detaches a mount, at most [`PASSES`] times.
pub fn unmount_under(dir: &Path) -> bool {
    const MNT_DETACH: i32 = 2;
    const UMOUNT_NOFOLLOW: i32 = 8;
    let mut m = mounts_under(dir);
    for _ in 0..PASSES {
        if m.is_empty() {
            return true;
        }
        m.sort_by_key(|p| std::cmp::Reverse(p.as_os_str().len()));
        for p in &m {
            let Ok(c) = std::ffi::CString::new(p.as_os_str().as_encoded_bytes()) else {
                continue;
            };
            // SAFETY: a NUL-ended path.
            unsafe { umount2(c.as_ptr(), MNT_DETACH | UMOUNT_NOFOLLOW) };
        }
        let left = mounts_under(dir);
        if left.len() >= m.len() {
            return left.is_empty();
        }
        m = left;
    }
    m.is_empty()
}

/// The most passes of [`unmount_under`].
const PASSES: usize = 16;

/// Remove the tree `dir`, the owner's bits added to each directory first.
/// ⛔ Not while a mount stays under it (T-1920).
pub fn remove_plain(dir: &std::path::Path) -> std::io::Result<bool> {
    if !unmount_under(dir) {
        return Err(std::io::Error::other(format!(
            "{}: a mount stays under the tree, which is not removed (TODO/enter.md T-1920)",
            dir.display()
        )));
    }
    crate::tree::open_dirs(dir)
        .and_then(|()| pod_extract::remove_tree(dir).map_err(|e| std::io::Error::other(e.chain())))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_path_of_mountinfo_is_unescaped() {
        assert_eq!(super::unescape("/a\\040b/c"), "/a b/c");
        assert_eq!(super::unescape("/plain"), "/plain");
        assert_eq!(super::unescape("/end\\04"), "/end\\04");
    }

    #[test]
    fn nothing_is_mounted_under_a_fresh_directory() {
        let mut r = [0u8; 8];
        std::io::Read::read_exact(&mut std::fs::File::open("/dev/urandom").unwrap(), &mut r)
            .unwrap();
        let tag: String = r.iter().map(|b| format!("{b:02x}")).collect();
        let d = std::env::temp_dir().join(format!("podvm-mnt-{tag}"));
        std::fs::create_dir(&d).unwrap();
        assert!(super::mounts_under(&d).is_empty());
        assert!(super::unmount_under(&d));
        std::fs::remove_dir(&d).unwrap();
    }

    use super::*;

    #[test]
    fn a_fill_is_live_while_its_lock_is_held() {
        let d = std::env::temp_dir().join(format!("podvm-live-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        let partial = d.join(".fill-1-abc");
        assert_eq!(FillLock::held(&partial), None, "no lock file");
        let lock = FillLock::take(&partial).unwrap();
        assert_eq!(FillLock::held(&partial), Some(true));
        drop(lock);
        assert_eq!(FillLock::held(&partial), None, "the drop removes the file");
        let _ = fs::remove_dir_all(&d);
    }
}
