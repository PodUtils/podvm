//! TODO/enter.md T-1985: a change of mode, owner or times under
//! `--podvm-confine`.
//!
//! Landlock governs no change of mode, owner or times (no access right for
//! them up to ABI 7), so a payload changed the mode and the times of a
//! file that no rule of the layer names. The handler sees each system
//! call of the payload, so it holds the rule for each call of the family:
//! the file that the call would change (the path opens `O_PATH` in the
//! root, the last link followed or not, as the call does; or the file of
//! the descriptor) must be under a path of the write table
//! (`PODVM_WRITE_HOST`, the host path of each rule with write), else the
//! call answers `EACCES`, as a rule of the kernel would. A path that does
//! not open runs the call, which answers its own error.
//!
//! ⛔ The handler shares the payload's address space: it is a
//! compatibility layer, not a boundary (TODO/enter.md T-1657).

use core::sync::atomic::Ordering;

use crate::path::{self, target};
use crate::state::{self, PATH_MAX};
use crate::sys::{self, nr, AT_EMPTY_PATH, AT_FDCWD, AT_SYMLINK_NOFOLLOW};

/// The line of a refusal of the family.
const REFUSED: &[u8] = b"podvm: syscall tier: --podvm-confine denies a change of mode, owner or times outside each write= path (TODO/enter.md T-1985)\n";

/// Write one line of a refusal on the payload's stderr, as the
/// interposer does: a raw `write`, with no allocation.
pub fn say(line: &[u8]) {
    // SAFETY: the line lives for the call.
    let _ = unsafe { sys::sc(nr::WRITE, 2, line.as_ptr() as i64, line.len() as i64) };
}

/// The variable of the write table, with its `=`.
pub const WRITE_TABLE_VAR: &[u8] = b"PODVM_WRITE_HOST=";

// The calls of x86_64 that change a file through a descriptor.
const FCHMOD: i64 = 91;
const FCHOWN: i64 = 93;

/// What the gate judges: the file of a descriptor, or of a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Object {
    /// The file of this descriptor.
    Fd(i64),
    /// The file of `(dirfd, path)`, the last link followed or not.
    Path(i64, i64, bool),
}

/// The file that the call `n` with the arguments `a` changes, where it is
/// a change of mode, owner or times; None for each other call.
pub fn object(n: i64, a: [i64; 6]) -> Option<Object> {
    let f = AT_FDCWD;
    let at = |dirfd: i64, p: i64, flags: i64| {
        if p == 0 || (flags & AT_EMPTY_PATH != 0 && empty(p)) {
            Object::Fd(dirfd)
        } else {
            Object::Path(dirfd, p, flags & AT_SYMLINK_NOFOLLOW == 0)
        }
    };
    Some(match n {
        nr::CHMOD | nr::CHOWN | nr::UTIME | nr::UTIMES => Object::Path(f, a[0], true),
        nr::LCHOWN => Object::Path(f, a[0], false),
        FCHMOD | FCHOWN => Object::Fd(a[0]),
        // The system call of `fchmodat` has no flags.
        nr::FCHMODAT | nr::FUTIMESAT => at(a[0], a[1], 0),
        nr::FCHMODAT2 | nr::UTIMENSAT => at(a[0], a[1], a[3]),
        nr::FCHOWNAT => at(a[0], a[1], a[4]),
        _ => return None,
    })
}

/// True where the payload gave an empty path.
fn empty(p: i64) -> bool {
    // SAFETY: the payload gave p as a path; see `sys::cstr`.
    unsafe { sys::cstr(p, 1) }.is_some_and(<[u8]>::is_empty)
}

/// Keep the write table of the confine layer from the environment of the
/// entry. A table that does not fit is a refusal: a run never goes on
/// without its table.
pub(crate) fn table(envp: &[std::ffi::OsString]) -> Result<(), String> {
    use std::os::unix::ffi::OsStrExt;
    let Some(t) = envp
        .iter()
        .find_map(|e| e.as_bytes().strip_prefix(WRITE_TABLE_VAR))
    else {
        return Ok(());
    };
    if !state::WRITE_TABLE.set(t) {
        return Err(format!(
            "the write table of the confine layer has {} bytes, over the {} that the handler holds",
            t.len(),
            PATH_MAX - 1
        ));
    }
    state::WRITE_ON.store(true, Ordering::Release);
    Ok(())
}

/// `Some(-EACCES)` for a change of mode, owner or times of a file that no
/// path of the write table covers; `None` for each other call, which runs
/// on.
pub fn gate(n: i64, a: [i64; 6]) -> Option<i64> {
    if !state::WRITE_ON.load(Ordering::Acquire) {
        return None;
    }
    let o = object(n, a)?;
    let Some(mut g) = crate::pool::Buf::get() else {
        say(REFUSED);
        return Some(-sys::EACCES);
    };
    let host = g.buf();
    let len = match o {
        Object::Fd(fd) => {
            let mut link = [0u8; 32];
            let r = sys::readlinkat(AT_FDCWD, path::fd_link(fd, &mut link), &mut host[..]);
            // A descriptor that is not open: the call answers `EBADF`.
            if r == -sys::ENOENT {
                return None;
            }
            r
        }
        Object::Path(dirfd, p, follow) => {
            // SAFETY: the payload gave p as a path; see `sys::cstr`. One
            // that does not fit reaches the kernel: `ENAMETOOLONG`.
            let path = unsafe { sys::cstr(p, PATH_MAX) }?;
            let Some(mut tb) = crate::pool::Buf::get() else {
                say(REFUSED);
                return Some(-sys::EACCES);
            };
            let buf = tb.buf();
            let Ok(t) = target(dirfd, path, buf) else {
                return None;
            };
            let fd = path::open_path(dirfd, path, t, &buf[..], follow, false);
            if fd < 0 {
                return None;
            }
            let mut link = [0u8; 32];
            let r = sys::readlinkat(AT_FDCWD, path::fd_link(fd, &mut link), &mut host[..]);
            sys::close(fd);
            r
        }
    };
    let ok = usize::try_from(len).is_ok_and(|n| {
        n > 0 && n < PATH_MAX && pod_confine::exec::allows_in(state::WRITE_TABLE.get(), &host[..n])
    });
    if ok {
        return None;
    }
    say(REFUSED);
    Some(-sys::EACCES)
}
