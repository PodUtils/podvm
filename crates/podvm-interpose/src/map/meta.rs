//! TODO/enter.md T-1985: a change of mode, owner or times under
//! `--podvm-confine`.
//!
//! Landlock governs no change of mode, owner or times (no access right for
//! them up to ABI 7), so `chmod 666 /no/s` changed a file that no rule of
//! the layer names. On the userland rung podvm writes the host path of
//! each rule with write in `PODVM_WRITE_HOST` (`:` separated, as the exec
//! table), and a call of the family on a file that no path of the table
//! covers answers `EACCES`, as a rule of the kernel would. The file is the
//! one that the kernel would change: the path opens `O_PATH` (the last
//! link followed or not, as the call does), and the link of the descriptor
//! names it, so a link, `..` and `/proc/self/fd/N` count.
//!
//! ⚠ The interposer is not a boundary: a raw system call (Go, a static
//! program) passes. The handler of the syscall tier and of the
//! native-static rung holds the same rule for each call.
//!
//! ⛔ No allocation: each buffer is on the caller's stack.

use super::*;

/// The variable of the write table of the confine layer: the host paths
/// of each rule with write, `:` separated.
pub const WRITE_VAR: &[u8] = b"PODVM_WRITE_HOST";

const EACCES: c_int = 13;
const O_PATH: c_int = 0o10_000_000;
const O_CLOEXEC: c_int = 0o2_000_000;
const O_NOFOLLOW: c_int = 0o400_000;

/// The calls that change the mode, the owner or the times of a file.
pub fn is_meta_call(what: &[u8]) -> bool {
    matches!(
        what,
        b"chmod"
            | b"fchmod"
            | b"fchmodat"
            | b"lchmod"
            | b"chown"
            | b"fchown"
            | b"lchown"
            | b"fchownat"
            | b"utime"
            | b"utimes"
            | b"utimensat"
            | b"futimens"
            | b"futimes"
            | b"futimesat"
            | b"lutimes"
    )
}

/// The host path that a descriptor link names.
pub enum Link {
    /// The path does not open: the real call answers its own error.
    Absent,
    /// The resolved host path, of this many bytes in the buffer.
    At(usize),
    /// The path opened, and its link did not read: fail safe.
    Unknown,
}

/// The link of `/proc/self/fd/N` (or `/proc/self/cwd` for `AT_FDCWD`) into
/// `out`, with a NUL.
///
/// # Safety
/// Calls the real `readlink`.
pub unsafe fn link_of_fd(fd: c_int, out: &mut [u8; OUT]) -> Link {
    let mut name = [0u8; 32];
    if fd == AT_FDCWD {
        name[..15].copy_from_slice(b"/proc/self/cwd\0");
    } else {
        crate::exec::fd_path(b"/proc/self/fd/", fd, &mut name);
    }
    let Some(readlink) = crate::next_readlink() else {
        return Link::Unknown;
    };
    let n = unsafe {
        readlink(
            name.as_ptr() as *const c_char,
            out.as_mut_ptr() as *mut c_char,
            OUT - 1,
        )
    };
    match usize::try_from(n) {
        Ok(n) if n > 0 && n < OUT - 1 => {
            out[n] = 0;
            Link::At(n)
        }
        // A descriptor that is not open: the real call answers `EBADF`.
        _ if crate::errno() == ENOENT => Link::Absent,
        _ => Link::Unknown,
    }
}

/// The host path that `(dirfd, path)` names, as the kernel resolves it,
/// into `out`.
///
/// # Safety
/// `path` is a C string; calls the real `openat`, `readlink` and `close`.
pub unsafe fn link_of_path(
    dirfd: c_int,
    path: *const c_char,
    follow: bool,
    out: &mut [u8; OUT],
) -> Link {
    let (Some(openat), Some(close)) = (crate::next_openat(), crate::next_close()) else {
        return Link::Unknown;
    };
    let flags = O_PATH | O_CLOEXEC | if follow { 0 } else { O_NOFOLLOW };
    let fd = unsafe { openat(dirfd, path, flags, 0) };
    if fd < 0 {
        return Link::Absent;
    }
    let l = unsafe { link_of_fd(fd, out) };
    unsafe { close(fd) };
    match l {
        Link::Absent => Link::Unknown,
        l => l,
    }
}

/// The write table, or None where the run has no confine layer.
///
/// # Safety
/// As [`lookup_env`].
unsafe fn write_table() -> Option<&'static [u8]> {
    let (v, n) = unsafe { lookup_env(WRITE_VAR) }?;
    Some(unsafe { core::slice::from_raw_parts(v, n) })
}

/// Answer for one link: true where the call runs; else `EACCES` and one
/// line.
fn judge(what: &[u8], table: &[u8], link: Link, out: &[u8; OUT]) -> bool {
    let ok = match link {
        Link::Absent => true,
        Link::At(n) => exec_allowed_in(table, &out[..n]),
        Link::Unknown => false,
    };
    if !ok {
        crate::say::line(&[
            what,
            b": --podvm-confine denies a change of mode, owner or times outside each write= path (TODO/enter.md T-1985)",
        ]);
        crate::set_errno(EACCES);
    }
    ok
}

/// TODO/enter.md T-1985: true where the confine layer lets the call
/// `what` change the file that `(dirfd, path)` names; else `errno` is
/// `EACCES` and one line is said. True with no layer, and for each call
/// that is not of the family.
///
/// # Safety
/// `path` is null or a C string.
pub unsafe fn meta_allowed(what: &[u8], dirfd: c_int, path: *const c_char, follow: bool) -> bool {
    if path.is_null() || !is_meta_call(what) {
        return true;
    }
    let Some(table) = (unsafe { write_table() }) else {
        return true;
    };
    let mut out = [0u8; OUT];
    let link = unsafe { link_of_path(dirfd, path, follow, &mut out) };
    judge(what, table, link, &out)
}

/// [`meta_allowed`] for the file of the descriptor `fd` (`fchmod`,
/// `futimens`, a null or an empty path with `AT_EMPTY_PATH`).
///
/// # Safety
/// Calls the real `readlink`.
pub unsafe fn meta_fd_allowed(what: &[u8], fd: c_int) -> bool {
    let Some(table) = (unsafe { write_table() }) else {
        return true;
    };
    let mut out = [0u8; OUT];
    let link = unsafe { link_of_fd(fd, &mut out) };
    judge(what, table, link, &out)
}

/// [`prepare_mode`] where the caller says whether the call follows the
/// link of its last component: `O_NOFOLLOW`, `AT_SYMLINK_NOFOLLOW` and
/// `AT_SYMLINK_FOLLOW` are flags of the call (TODO/interpose.md T-1725).
///
/// # Safety
/// As [`prepare`].
pub unsafe fn prepare_follow(
    what: &[u8],
    dirfd: c_int,
    path: *const c_char,
    writes: bool,
    follow: bool,
    out: &mut [u8; OUT],
) -> *const c_char {
    let p = unsafe { super::prepare::prepare_inner(what, dirfd, path, writes, follow, out) };
    // TODO/enter.md T-1985: the confine layer on a change of mode, owner
    // or times, on the path that the call passes on.
    let at = if p == path { dirfd } else { AT_FDCWD };
    if !p.is_null() && !unsafe { meta_allowed(what, at, p, follow) } {
        return core::ptr::null();
    }
    p
}
