//! TODO/probe.md T-1911: in a user namespace with the subordinate range,
//! the owners of the image go onto the tree, so that `/etc/shadow` is gid
//! 42 and a payload that drops to another id meets the kernel's checks.
//!
//! The extraction gives each file to the caller and records the owners
//! that the image meant in the sidecar (`.meta.jsonl` beside `rootfs`).
//! Here each entry whose owners differ gets them with `fchownat`.
//!
//! ⛔ The tree came from a hostile archive. The directory of each entry is
//! opened with `openat2(RESOLVE_IN_ROOT | RESOLVE_NO_SYMLINKS)`, and the
//! entry is changed with `AT_SYMLINK_NOFOLLOW`: no link of the image leads
//! a change out of the tree.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use podvm_probe::sys as k;

/// The highest id that the map of this namespace holds; 0 where the map
/// is the one line of the caller.
pub static TOP: AtomicU64 = AtomicU64::new(0);

/// True where the map of this namespace holds the subordinate range up to
/// `uid` and `gid`.
pub fn holds(uid: u32, gid: u32) -> bool {
    let top = TOP.load(Ordering::Relaxed);
    top > 0 && u64::from(uid.max(gid)) <= top
}

unsafe extern "C" {
    fn fchownat(dirfd: i32, path: *const std::ffi::c_char, uid: u32, gid: u32, flags: i32) -> i32;
}

/// One record of the sidecar whose owners differ: the path in the tree
/// and the ids that the image meant.
#[derive(Debug, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    pub uid: u32,
    pub gid: u32,
}

/// The changes of a sidecar for a map that holds ids 0 to `top`.
pub fn changes(text: &str, top: u64) -> Vec<Change> {
    text.lines()
        .filter_map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).ok()?;
            let (uid, gid) = (v["uid"].as_u64()?, v["gid"].as_u64()?);
            let applied = (v["applied"]["uid"].as_u64()?, v["applied"]["gid"].as_u64()?);
            let path = v["path"].as_str()?.trim_start_matches('/').to_string();
            let wanted = (uid, gid) != applied && uid <= top && gid <= top && !path.is_empty();
            wanted.then_some(Change {
                path,
                uid: uid as u32,
                gid: gid as u32,
            })
        })
        .collect()
}

/// Give the owners of the image to the tree `rootfs`, where this
/// namespace holds the range. The count changed.
pub fn apply(rootfs: &Path) -> usize {
    let top = TOP.load(Ordering::Relaxed);
    if top == 0 {
        return 0;
    }
    let Some(text) = rootfs
        .parent()
        .and_then(|d| std::fs::read_to_string(d.join(".meta.jsonl")).ok())
    else {
        return 0;
    };
    apply_text(rootfs, &text, top)
}

/// Give the owners of the sidecar `text` to the tree `rootfs`, for the ids
/// up to `top`. The count changed. A kernel of a sandbox that holds each
/// id (gVisor, TODO/enter.md T-2007) takes the same records.
pub fn apply_text(rootfs: &Path, text: &str, top: u64) -> usize {
    let Some(root) = rootfs.to_str().and_then(k::CBuf::new) else {
        return 0;
    };
    let Ok(rfd) = k::open(&root, k::O_PATH | k::O_DIRECTORY | k::O_CLOEXEC, 0) else {
        return 0;
    };
    let mut n = 0;
    for c in changes(text, top) {
        let (dir, name) = c.path.rsplit_once('/').unwrap_or(("", c.path.as_str()));
        let dfd = if dir.is_empty() {
            Some(rfd)
        } else {
            let how = k::OpenHow {
                flags: k::O_PATH | k::O_DIRECTORY | k::O_CLOEXEC,
                mode: 0,
                resolve: k::RESOLVE_IN_ROOT | k::RESOLVE_NO_SYMLINKS,
            };
            k::CBuf::new(dir).and_then(|d| k::openat2(rfd, &d, &how).ok())
        };
        let (Some(dfd), Ok(cname)) = (dfd, std::ffi::CString::new(name)) else {
            continue;
        };
        const AT_SYMLINK_NOFOLLOW: i32 = 0x100;
        // SAFETY: dfd is an open directory; cname lives for the call.
        if unsafe {
            fchownat(
                dfd as i32,
                cname.as_ptr(),
                c.uid,
                c.gid,
                AT_SYMLINK_NOFOLLOW,
            )
        } == 0
        {
            n += 1;
        }
        if dfd != rfd {
            let _ = k::close(dfd);
        }
    }
    let _ = k::close(rfd);
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_entries_whose_owners_differ_and_that_the_map_holds_change() {
        let t = concat!(
            r#"{"path":"etc/shadow","uid":0,"gid":42,"mode":"0640","applied":{"uid":0,"gid":0},"reason":"gid 42 unmapped"}"#,
            "\n",
            r#"{"path":"bin/sh","uid":0,"gid":0,"mode":"0755","applied":{"uid":0,"gid":0}}"#,
            "\n",
            r#"{"path":"home/far","uid":70000,"gid":70000,"mode":"0755","applied":{"uid":0,"gid":0}}"#,
            "\nnot json\n"
        );
        assert_eq!(
            changes(t, 65536),
            vec![Change {
                path: "etc/shadow".into(),
                uid: 0,
                gid: 42
            }]
        );
        assert!(changes(t, 0).is_empty());
    }
}
