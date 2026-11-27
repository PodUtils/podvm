//! TODO/enter.md T-2105: `exec` into a run whose payload is in a user
//! namespace under the one that owns the mounts of the run
//! (`podvm_enter::lock`).
//!
//! The kernel joins a mount namespace only for a holder of `CAP_SYS_ADMIN`
//! over its owner. The nested namespace of the payload holds none there,
//! so a join of the payload's user namespace first and of its mount
//! namespace after it gets `EPERM`. This process joins the owner of the
//! mounts first; pod-enter then joins the payload's user namespace and its
//! mount namespace in one call (`setns(pidfd, CLONE_NEWUSER |
//! CLONE_NEWNS)`, `ns_join`), which the owner holds the rights for.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::MetadataExt;

unsafe extern "C" {
    fn ioctl(fd: i32, request: std::ffi::c_ulong, ...) -> i32;
}

/// `NS_GET_USERNS` of `<linux/nsfs.h>`: the owner of a namespace.
const NS_GET_USERNS: std::ffi::c_ulong = 0xb701;

/// The user namespace that a join of the run of `pid` enters first.
#[derive(Debug)]
pub(crate) enum Owner {
    /// The payload's own: its mount namespace belongs to it (as before).
    Payload,
    /// This process's own: no user namespace to join.
    Ours,
    /// Another one, which owns the mounts of the run.
    Other(OwnedFd),
}

/// The (device, inode) of the namespace file `path`.
fn id_of(path: &str) -> Option<(u64, u64)> {
    std::fs::metadata(path).ok().map(|m| (m.dev(), m.ino()))
}

/// The owner of the mount namespace of `pid` (a pid of this `/proc`). A read
/// that fails gives [`Owner::Payload`]: the join then goes as before.
pub(crate) fn mount_owner(pid: i64) -> Owner {
    let user = id_of(&format!("/proc/{pid}/ns/user"));
    let Ok(mnt) = std::fs::File::open(format!("/proc/{pid}/ns/mnt")) else {
        return Owner::Payload;
    };
    // SAFETY: an open descriptor of a namespace and a request with no
    // argument; the kernel returns a new descriptor or -1.
    let fd = unsafe { ioctl(mnt.as_raw_fd(), NS_GET_USERNS) };
    if fd < 0 {
        return Owner::Payload;
    }
    // SAFETY: the ioctl gave the descriptor, and nothing else owns it.
    let owner = unsafe { OwnedFd::from_raw_fd(fd) };
    let id = id_of(&format!("/proc/self/fd/{}", owner.as_raw_fd()));
    match id {
        None => Owner::Payload,
        Some(i) if Some(i) == user => Owner::Payload,
        Some(i) if Some(i) == id_of("/proc/self/ns/user") => Owner::Ours,
        Some(_) => Owner::Other(owner),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This process: its mount namespace belongs to its own user namespace;
    /// a pid that names no process reads as before.
    #[test]
    fn locked_owner_of_this_process_is_its_own_namespace() {
        let me = i64::from(std::process::id());
        assert!(
            matches!(mount_owner(me), Owner::Payload),
            "{:?}",
            mount_owner(me)
        );
        assert!(matches!(mount_owner(i64::from(i32::MAX)), Owner::Payload));
    }
}
