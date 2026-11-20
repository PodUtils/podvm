//! TODO/interpose.md T-1725. A path of a path call resolves its links in
//! guest names before the real call.
//!
//! ⛔ The table rewrites a path string; the kernel then follows each link
//! of the host path from the host's root. An absolute target is a guest
//! path: `/home/link -> /opt/f` of an image opened `/opt/f` of the host
//! (measured, PodUtils/podrunner#61). So the funnel walks the path in guest
//! names with the walk of the exec routes (T-1646): each directory, and the
//! last component where the call follows it. A call that does not follow
//! its last link keeps that name.

use super::*;
use crate::exec::walk;

/// The calls that do not follow the link of their last component. Each
/// other call follows it, unless its flags say no ([`prepare_follow`]).
const NOFOLLOW: &[&[u8]] = &[
    b"lstat",
    b"lstat64",
    b"__lxstat",
    b"__lxstat64",
    b"lchown",
    b"readlink",
    b"readlinkat",
    b"unlink",
    b"unlinkat",
    b"rmdir",
    b"remove",
    b"rename",
    b"renameat",
    b"renameat2",
    b"link",
    b"linkat",
    b"symlink",
    b"symlinkat",
    b"lsetxattr",
    b"lgetxattr",
    b"llistxattr",
    b"lremovexattr",
    b"lutimes",
    b"lchmod",
    b"mkdir",
    b"mkdirat",
    b"mknod",
    b"mknodat",
    b"mkfifo",
    b"mkfifoat",
    b"bind",
];

/// Whether the call `what` follows the link of its last component.
pub fn follows(what: &[u8]) -> bool {
    !NOFOLLOW.contains(&what)
}

/// The host path of the absolute guest path `g` into `out` (with a NUL),
/// each link resolved in guest names: all of it where `follow`, else its
/// directory, with the last name kept. The resolved guest path goes into
/// `guest` (TODO/enter.md T-2040: the read-only table reads it). The two
/// lengths, guest then host, or an errno.
///
/// # Safety
/// Calls the real `readlink`.
pub unsafe fn resolve_links(
    t: &Table,
    g: &[u8],
    follow: bool,
    guest: &mut [u8; OUT],
    out: &mut [u8; OUT],
) -> Result<(usize, usize), c_int> {
    let mut end = g.len();
    while end > 1 && g[end - 1] == b'/' {
        end -= 1;
    }
    let cut = g[..end].iter().rposition(|&c| c == b'/').unwrap_or(0);
    let last = &g[cut + 1..end];
    // A mount point is never a link: the guest path of a volume names the
    // volume's own host path, also for a call that does not follow
    // (`lstat("/workspace")` of node --permission, T-1661).
    let mount = is_mount_point(t, &g[..end]);
    if follow || mount || last.is_empty() || last == b"." || last == b".." {
        return unsafe { walk::resolve(t, g, guest, out) };
    }
    let parent = if cut == 0 { &g[..1] } else { &g[..cut] };
    let (gn, hn) = unsafe { walk::resolve(t, parent, guest, out) }?;
    Ok((join_last(guest, gn, last)?, join_last(out, hn, last)?))
}

/// `buf[..n]`, a `/` where it does not end in one, and `last`, with a NUL:
/// the new length.
fn join_last(buf: &mut [u8; OUT], n: usize, last: &[u8]) -> Result<usize, c_int> {
    let mut at = n;
    if at + 1 + last.len() >= OUT {
        return Err(ENAMETOOLONG);
    }
    if at == 0 || buf[at - 1] != b'/' {
        buf[at] = b'/';
        at += 1;
    }
    buf[at..at + last.len()].copy_from_slice(last);
    at += last.len();
    buf[at] = 0;
    Ok(at)
}

/// Whether `g` is the guest side of a pair other than `/`.
fn is_mount_point(t: &Table, g: &[u8]) -> bool {
    t.pairs[..t.n.min(MAX_PAIRS)].iter().any(|p| {
        // SAFETY: each pair borrows environ memory, valid for the process.
        let from = unsafe { core::slice::from_raw_parts(p.from_p, p.from_n) };
        let mut n = from.len();
        while n > 1 && from[n - 1] == b'/' {
            n -= 1;
        }
        n > 1 && &from[..n] == g
    })
}
