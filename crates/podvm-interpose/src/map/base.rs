//! TODO/enter.md T-1972 (PodUtils/podrunner#277, #300 ask 3):
//! `--podvm-volume-base` on the loader family.
//!
//! ⛔ The table maps a guest path to a host path string, and the walk of
//! T-1725 reads each link of that string in guest names. The kernel then
//! resolves the string again at the real call. A link that a process of
//! the host makes under the base between the walk and the call (a step of
//! a job, PodUtils/podrunner#277) moves the call out of the base.
//!
//! ⭐ So a path in a volume whose host side is under `PODVM_VOLUME_BASE`
//! opens again at each call: the base by its path, the volume under the
//! base, then the rest of the path under the volume, each with
//! `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)`. After the walk the
//! rest holds no link of its own, so a link there is a change of the host:
//! the call fails with `ELOOP` (or `EXDEV`) and one line. The real call
//! then takes `/proc/self/fd/N` (the object) or `/proc/self/fd/N/NAME`
//! (the directory and the last name, for a call that does not follow the
//! last link), and the descriptor stays open until the call returns
//! ([`Scope`]).
//!
//! ⚠ No descriptor stays open between two calls, so `close_range` of the
//! payload closes nothing of podvm's, and a volume that the host replaces
//! by a link after the start is a refusal, not a way out. A call that gives
//! the name on after it returns (an exec route, `dlopen`, `glob`, the `ftw`
//! family, a socket address) or that runs with no [`Scope`] gets the check
//! only: the same opens, then the host path as before.

use super::*;
use core::cell::Cell;
use core::ffi::c_long;

/// The variable of the base: the canonical host directory.
pub const BASE_VAR: &[u8] = b"PODVM_VOLUME_BASE";

const ENOENT: c_int = 2;
const EXDEV: c_int = 18;
const ELOOP: c_int = 40;
const RESOLVE_NO_SYMLINKS: u64 = 0x04;
const RESOLVE_BENEATH: u64 = 0x08;
const PINS: usize = 8;

/// The calls that give the name on after they return, or that take flags
/// of their own for the resolution: the check only, no pinned name.
const CHECK_ONLY: &[&[u8]] = &[
    b"execv",
    b"execve",
    b"execveat",
    b"execvp",
    b"execvpe",
    b"fexecve",
    b"posix_spawn",
    b"posix_spawnp",
    b"dlopen",
    b"dlmopen",
    b"glob",
    b"glob64",
    b"ftw",
    b"ftw64",
    b"nftw",
    b"nftw64",
    b"openat2",
    b"mount",
    b"bind",
    b"connect",
];

/// The calls that can make the file of their last name: where the name is
/// absent at the check, they take the directory and the name. The `open`
/// shapes then add `O_NOFOLLOW`, and the `fopen` shapes make the file with
/// `O_NOFOLLOW` first ([`take_by_name`]).
const CREATORS: &[&[u8]] = &[
    b"open",
    b"open64",
    b"openat",
    b"openat64",
    b"fopen",
    b"fopen64",
    b"freopen",
    b"freopen64",
];

struct Held {
    fd: [Cell<c_int>; PINS],
    n: Cell<usize>,
    scopes: Cell<usize>,
    by_name: Cell<bool>,
}

thread_local! {
    /// The descriptors of the calls of this thread that run now. A const
    /// initializer and no destructor: no allocation and no registration.
    static HELD: Held = const {
        Held {
            fd: [const { Cell::new(-1) }; PINS],
            n: Cell::new(0),
            scopes: Cell::new(0),
            by_name: Cell::new(false),
        }
    };
}

/// The life of the pinned descriptors of one call: an entry point makes
/// one before its path goes through [`super::prepare`], and the drop
/// closes each descriptor that the call pinned, after the real call.
pub struct Scope {
    start: usize,
}

impl Scope {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Scope {
        let start = HELD.with(|h| {
            h.scopes.set(h.scopes.get() + 1);
            h.by_name.set(false);
            h.n.get()
        });
        Scope { start }
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        HELD.with(|h| {
            let n = h.n.get();
            for i in self.start..n.min(PINS) {
                close(h.fd[i].get());
                h.fd[i].set(-1);
            }
            h.n.set(self.start.min(n));
            h.scopes.set(h.scopes.get().saturating_sub(1));
        });
    }
}

/// True once: the last pin of this thread gave a follow call the directory
/// and the name of an absent file.
pub fn take_by_name() -> bool {
    HELD.with(|h| h.by_name.replace(false))
}

/// Whether `p` names a place under a descriptor that a [`Scope`] of this
/// thread holds now: `/proc/self/fd/N` or `/proc/self/fd/N/...`.
///
/// # Safety
/// `p` is null or a NUL-ended path.
pub unsafe fn is_held(p: *const c_char) -> bool {
    const P: &[u8] = b"/proc/self/fd/";
    if p.is_null() {
        return false;
    }
    let n = strnlen(p, OUT);
    // SAFETY: `n` bytes of `p` are readable.
    let s = unsafe { core::slice::from_raw_parts(p as *const u8, n) };
    let Some(r) = s.strip_prefix(P) else {
        return false;
    };
    let end = r.iter().position(|&c| c == b'/').unwrap_or(r.len());
    let digits = &r[..end];
    if digits.is_empty() || digits.len() > 9 || !digits.iter().all(u8::is_ascii_digit) {
        return false;
    }
    let fd = digits.iter().fold(0i32, |a, &d| a * 10 + (d - b'0') as i32);
    HELD.with(|h| (0..h.n.get().min(PINS)).any(|i| h.fd[i].get() == fd))
}

/// A descriptor kept to the end of the [`Scope`] of this thread, or `false`
/// where no scope runs or the scope is full.
fn keep(fd: c_int) -> bool {
    HELD.with(|h| {
        let n = h.n.get();
        if h.scopes.get() == 0 || n >= PINS {
            return false;
        }
        h.fd[n].set(fd);
        h.n.set(n + 1);
        true
    })
}

fn close(fd: c_int) {
    if fd >= 0 {
        // SAFETY: closes a descriptor that this module opened.
        let _ = unsafe {
            crate::raw_calls::raw(crate::raw_calls::SYS_CLOSE, [fd as c_long, 0, 0, 0, 0, 0])
        };
    }
}

/// `openat2(dirfd, path, flags, resolve)`: a descriptor, or `-errno`.
fn openat2(dirfd: c_int, path: &[u8], flags: c_int, resolve: u64) -> c_int {
    let mut z = [0u8; OUT];
    if path.len() >= OUT {
        return -36;
    }
    z[..path.len()].copy_from_slice(path);
    let how: [u64; 3] = [(flags | crate::oflags::O_CLOEXEC) as u64, 0, resolve];
    // SAFETY: a NUL-ended path and the 24 bytes of `struct open_how`.
    let r = unsafe {
        crate::raw_calls::raw(
            crate::raw_calls::SYS_OPENAT2,
            [
                dirfd as c_long,
                z.as_ptr() as c_long,
                how.as_ptr() as c_long,
                24,
                0,
                0,
            ],
        )
    };
    r as c_int
}

/// What [`pin`] did with a host path.
#[derive(Debug, PartialEq, Eq)]
pub enum Pin {
    /// The path is not in a volume under the base: nothing to do.
    Apart,
    /// The opens passed; the host path stands (no [`Scope`], or a call of
    /// [`CHECK_ONLY`]).
    Checked,
    /// `out` holds the name under a held descriptor, of this length.
    Held(usize),
}

/// The base of the run, or `None`.
fn base() -> Option<&'static [u8]> {
    // SAFETY: environ memory, valid for the life of the process.
    let (p, n) = unsafe { lookup_env(BASE_VAR) }?;
    let b = unsafe { core::slice::from_raw_parts(p, n) };
    let b = if b.len() > 1 {
        b.strip_suffix(b"/").unwrap_or(b)
    } else {
        b
    };
    (b.first() == Some(&b'/')).then_some(b)
}

/// `rest` of `path` after the directory `dir` (empty for `dir` itself), or
/// `None` where `path` is not at or under `dir`.
fn under<'a>(dir: &[u8], path: &'a [u8]) -> Option<&'a [u8]> {
    if dir == b"/" {
        return path.strip_prefix(b"/");
    }
    let r = path.strip_prefix(dir)?;
    if r.is_empty() {
        return Some(r);
    }
    r.strip_prefix(b"/")
}

/// The host side of the volume of the host path `h` (the longest pair
/// whose guest side is not `/`), where it is under `base`: that host side
/// and its rest under the base.
fn volume_of<'a>(t: &'a Table, base: &[u8], h: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
    let mut best: Option<(&[u8], &[u8])> = None;
    for p in &t.pairs[..t.n.min(MAX_PAIRS)] {
        // SAFETY: each pair borrows environ memory of its lengths.
        let (from, to) = unsafe {
            (
                core::slice::from_raw_parts(p.from_p, p.from_n),
                core::slice::from_raw_parts(p.to_p, p.to_n),
            )
        };
        let to = if to.len() > 1 {
            to.strip_suffix(b"/").unwrap_or(to)
        } else {
            to
        };
        if from == b"/" || to.len() < 2 || under(to, h).is_none() {
            continue;
        }
        let Some(vrest) = under(base, to) else {
            continue;
        };
        if best.is_none_or(|(b, _)| to.len() > b.len()) {
            best = Some((to, vrest));
        }
    }
    best
}

/// Say the refusal of a path under the base, and give its errno.
fn refuse(what: &[u8], e: c_int, why: &[u8]) -> c_int {
    crate::say::line(&[
        what,
        b": ",
        why,
        b" (--podvm-volume-base, TODO/enter.md T-1972), so the call fails",
    ]);
    e
}

/// `/proc/self/fd/FD` and then `/NAME` where `name` is not empty, into
/// `out` with a NUL: its length, or `None` where it does not fit.
fn fd_name(fd: c_int, name: &[u8], out: &mut [u8; OUT]) -> Option<usize> {
    const P: &[u8] = b"/proc/self/fd/";
    let num = crate::say::Num::new(fd as u64);
    let num = num.as_bytes();
    let n = P.len() + num.len() + if name.is_empty() { 0 } else { 1 + name.len() };
    if n >= OUT {
        return None;
    }
    out[..P.len()].copy_from_slice(P);
    out[P.len()..P.len() + num.len()].copy_from_slice(num);
    let mut at = P.len() + num.len();
    if !name.is_empty() {
        out[at] = b'/';
        out[at + 1..at + 1 + name.len()].copy_from_slice(name);
        at += 1 + name.len();
    }
    out[at] = 0;
    Some(at)
}

/// The host path `out[..n]` of the call `what` held under the base, where
/// it lies in a volume under the base: see the module text. `follow` says
/// whether the call follows the link of its last name, `writes` whether it
/// can make the file. An `Err` is the errno of the call; a refusal of a
/// link said its line.
///
/// # Safety
/// `t` borrows the environ of the process.
pub unsafe fn pin(
    t: &Table,
    what: &[u8],
    follow: bool,
    writes: bool,
    out: &mut [u8; OUT],
    n: usize,
) -> Result<Pin, c_int> {
    let Some(base) = base() else {
        return Ok(Pin::Apart);
    };
    let mut host = [0u8; OUT];
    host[..n].copy_from_slice(&out[..n]);
    let h = &host[..n];
    let Some((vol, vrest)) = volume_of(t, base, h) else {
        return Ok(Pin::Apart);
    };
    let Some(rest) = under(vol, h) else {
        return Ok(Pin::Apart);
    };
    let vrest: &[u8] = if vrest.is_empty() { b"." } else { vrest };
    let dir = crate::oflags::O_PATH | crate::oflags::O_DIRECTORY;
    let bfd = openat2(AT_FDCWD, base, dir, 0);
    if bfd < 0 {
        return Err(refuse(what, -bfd, b"the volume base does not open"));
    }
    let vfd = openat2(bfd, vrest, dir, RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS);
    close(bfd);
    if vfd == -ELOOP || vfd == -EXDEV {
        return Err(refuse(
            what,
            -vfd,
            b"the host path of the volume holds a symbolic link under its base",
        ));
    }
    if vfd < 0 {
        return Err(refuse(
            what,
            -vfd,
            b"the host path of the volume does not open under its base",
        ));
    }
    // The last name, and the directory before it.
    let cut = rest.iter().rposition(|&c| c == b'/');
    let (parent, name): (&[u8], &[u8]) = match cut {
        Some(i) => (&rest[..i], &rest[i + 1..]),
        None => (b".", rest),
    };
    let parent: &[u8] = if parent.is_empty() { b"." } else { parent };
    let dots = name == b"." || name == b"..";
    let (fd, last): (c_int, &[u8]) = if rest.is_empty() {
        // The volume itself: a mount point of the guest, never a link.
        (vfd, b".")
    } else if follow || dots {
        let fd = openat2(
            vfd,
            rest,
            crate::oflags::O_PATH,
            RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS,
        );
        if fd == -ENOENT && writes && !dots && CREATORS.contains(&what) {
            let pfd = openat2(vfd, parent, dir, RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS);
            close(vfd);
            if pfd < 0 {
                return Err(link_or(what, -pfd));
            }
            HELD.with(|h| h.by_name.set(true));
            (pfd, name)
        } else {
            close(vfd);
            if fd < 0 {
                return Err(link_or(what, -fd));
            }
            (fd, b"")
        }
    } else {
        let pfd = openat2(vfd, parent, dir, RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS);
        close(vfd);
        if pfd < 0 {
            return Err(link_or(what, -pfd));
        }
        (pfd, name)
    };
    let by_name = HELD.with(|h| h.by_name.get());
    if CHECK_ONLY.contains(&what) || !keep(fd) {
        close(fd);
        HELD.with(|h| h.by_name.set(false));
        // A creating call of the check only would follow a link that the
        // host makes at the absent name: refused with ENOENT before it.
        if by_name {
            return Err(ENOENT);
        }
        return Ok(Pin::Checked);
    }
    match fd_name(fd, last, out) {
        Some(m) => Ok(Pin::Held(m)),
        None => Err(36),
    }
}

/// The errno of a failed open under the volume: a link said its line.
fn link_or(what: &[u8], e: c_int) -> c_int {
    match e {
        ELOOP => refuse(what, e, b"the path holds a symbolic link under its base"),
        EXDEV => refuse(what, e, b"the path leaves its volume under the base"),
        _ => e,
    }
}

/// TODO/enter.md T-1972: the host path `out[..n]` held under the volume
/// base ([`pin`]). `Ok(true)` where `out` now holds the name
/// under a held descriptor, `Ok(false)` where the host path in `out`
/// stands, and `Err` with `errno` set where the call fails.
///
/// # Safety
/// As [`super::prepare`].
pub(super) unsafe fn held(
    t: &Table,
    what: &[u8],
    follow: bool,
    writes: bool,
    out: &mut [u8; OUT],
    n: usize,
) -> Result<bool, ()> {
    match unsafe { pin(t, what, follow, writes, out, n) } {
        Ok(Pin::Held(m)) => {
            out[m] = 0;
            Ok(true)
        }
        Ok(_) => {
            out[n] = 0;
            Ok(false)
        }
        Err(e) => {
            crate::set_errno(e);
            Err(())
        }
    }
}
