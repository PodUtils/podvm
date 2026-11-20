//! The funnel of each path call: absolutize, the read-only layer, the
//! rewrite, the walk of links and the volume base (T-0703, T-1655, T-1725,
//! T-1972, TODO/enter.md T-2040).

use super::*;

/// The pointer one path-taking call passes on.
///
/// ⭐ One funnel for every entry point in `lib.rs`: absolutize against the
/// table, and hand back either the rewritten buffer or the caller's own
/// pointer. A null return means the call does not happen: `errno` is set
/// and one line is said.
///
/// ⭐ A descriptor-relative path the table cannot be checked against goes
/// through UNCHANGED, not failed. The kernel resolves it through a directory
/// that is already real -- every directory descriptor the payload holds came
/// from a call this object already rewrote -- so forwarding is kernel-correct
/// and failing it would break every tool that walks with `openat`, like `tar`
/// and `ls -R`, on any run with a table set. Only `AT_FDCWD`, which resolves
/// through the working directory, can fail here, and only where `getcwd`
/// itself fails or its guest name does not fit. T-1643: that failure never
/// falls back to the unmapped relative path.
///
/// Three fast paths that never touch the table: a null pointer reaches the
/// real call (which answers `EFAULT` as it always did), an empty table
/// rewrites nothing (so an unmapped payload cannot fail here at all), and an
/// overlong path reaches the real call (which answers `ENAMETOOLONG`).
///
/// # Safety
/// As [`absolutize`]: a readable path and a writable `OUT`-byte buffer.
pub unsafe fn prepare(
    what: &[u8],
    dirfd: c_int,
    path: *const c_char,
    out: &mut [u8; OUT],
) -> *const c_char {
    unsafe { prepare_mode(what, dirfd, path, is_write_call(what), out) }
}

/// [`prepare`] where the caller says whether the call writes: `open` and
/// `fopen` write only with some flags. A write under a read-only prefix of
/// `PODVM_RO` answers `EROFS` (TODO/cli.md T-1655).
///
/// # Safety
/// As [`prepare`].
pub unsafe fn prepare_mode(
    what: &[u8],
    dirfd: c_int,
    path: *const c_char,
    writes: bool,
    out: &mut [u8; OUT],
) -> *const c_char {
    unsafe { prepare_follow(what, dirfd, path, writes, follows(what), out) }
}

/// [`prepare_follow`] with no check of the confine layer
/// (`meta::prepare_follow` adds it, TODO/enter.md T-1985).
///
/// # Safety
/// As [`prepare`].
pub(super) unsafe fn prepare_inner(
    what: &[u8],
    dirfd: c_int,
    path: *const c_char,
    writes: bool,
    follow: bool,
    out: &mut [u8; OUT],
) -> *const c_char {
    if path.is_null() {
        return path;
    }
    let t = unsafe { table() };
    if t.n == 0 {
        return path;
    }
    // An empty path reaches the kernel, which answers ENOENT for it: the
    // .NET runtime asks `realpath("")` and `access("")` at each start, and
    // a line of podvm for each would be noise (TODO/cli.md T-1706).
    let n = strnlen(path, OUT);
    if n == 0 || n >= OUT {
        return path;
    }
    let relative = unsafe { *(path as *const u8) } != b'/';
    let mut abs = [0u8; OUT];
    if unsafe { absolutize(&t, dirfd, path, &mut abs) } < 0 {
        // ⭐ Unresolvable against a descriptor: forward, per above. Unresolvable
        // against the working directory: the process is genuinely lost, fail it.
        if relative && dirfd != AT_FDCWD {
            return path;
        }
        crate::say::line(&[what, b": podvm cannot resolve the path, so the call fails"]);
        crate::set_errno(ENOENT);
        return core::ptr::null();
    }
    let mut abs_n = strnlen(abs.as_ptr() as *const c_char, OUT);
    // ⭐ TODO/enter.md T-2040: a host path of a volume goes on as its guest
    // path, so the read-only table, the walk and the base hold for it.
    if let Some(m) = unsafe { to_volume_guest(&t, &mut abs, abs_n) } {
        abs_n = m;
    }
    if writes && unsafe { ro_denied(&t, &abs[..abs_n]) } {
        crate::set_errno(EROFS);
        return core::ptr::null();
    }
    let mut guest = [0u8; OUT];
    match unsafe { rewrite(&t, abs.as_ptr(), abs_n, out) } {
        // ⭐ TODO/enter.md T-2045: a host path of the rootfs (T-1705) walks
        // its links from the rootfs, so a link in it names no host file.
        -1 if unsafe { under_host_side(&t, abs.as_ptr(), abs_n) } => {
            let walked =
                unsafe { walk_links(&t, what, &abs[..abs_n], follow, writes, &mut guest, out) };
            walked.unwrap_or_else(|| {
                out[..abs_n].copy_from_slice(&abs[..abs_n]);
                out[abs_n] = 0;
                match unsafe { super::base::held(&t, what, follow, writes, out, abs_n) } {
                    Ok(_) => out.as_ptr() as *const c_char,
                    Err(()) => core::ptr::null(),
                }
            })
        }
        // ⭐ Unmatched resolves back to the caller's own pointer, never the
        // absolutized copy: the bytes the kernel sees stay exactly the
        // payload's, including any symlinks in the working directory prefix.
        // The kernel then resolves a relative path against the real base.
        // T-1972: a host path in a volume under the base is held as well.
        -1 => {
            out[..abs_n].copy_from_slice(&abs[..abs_n]);
            match unsafe { super::base::held(&t, what, follow, writes, out, abs_n) } {
                Ok(true) => out.as_ptr() as *const c_char,
                Ok(false) => path,
                Err(()) => core::ptr::null(),
            }
        }
        -2 => {
            crate::say::line(&[what, b": the mapped path does not fit, so the call fails"]);
            crate::set_errno(ENAMETOOLONG);
            core::ptr::null()
        }
        // Where the walk cannot run (no real `readlink`), the rewrite
        // stands.
        n => {
            let walked =
                unsafe { walk_links(&t, what, &abs[..abs_n], follow, writes, &mut guest, out) };
            walked.unwrap_or_else(|| {
                let n = unsafe { rewrite(&t, abs.as_ptr(), abs_n, out) }.max(n);
                out[n as usize] = 0;
                match unsafe { super::base::held(&t, what, follow, writes, out, n as usize) } {
                    Ok(_) => out.as_ptr() as *const c_char,
                    Err(()) => core::ptr::null(),
                }
            })
        }
    }
}

/// ⭐ T-1725: the path `abs` resolves its links in guest names into `out`,
/// so an absolute link target stays in the rootfs; then the base holds it
/// (T-1972). T-2040: the read-only table holds for the path that the walk
/// reached (a link to a `:ro` volume, by its guest or host path). The
/// pointer to call with (null with `errno` set), or `None` where the walk
/// cannot run (no real `readlink`). The NUL ends the name here: a buffer
/// used again (the per-thread buffer of `dlopen`, T-1645) holds the tail
/// of a longer name.
///
/// # Safety
/// As [`prepare`].
unsafe fn walk_links(
    t: &Table,
    what: &[u8],
    abs: &[u8],
    follow: bool,
    writes: bool,
    guest: &mut [u8; OUT],
    out: &mut [u8; OUT],
) -> Option<*const c_char> {
    match unsafe { resolve_links(t, abs, follow, guest, out) } {
        Ok((gn, _)) if writes && unsafe { ro_denied(t, &guest[..gn]) } => {
            crate::set_errno(EROFS);
            Some(core::ptr::null())
        }
        Ok((_, m)) => {
            out[m] = 0;
            Some(
                match unsafe { super::base::held(t, what, follow, writes, out, m) } {
                    Ok(_) => out.as_ptr() as *const c_char,
                    Err(()) => core::ptr::null(),
                },
            )
        }
        Err(EINVAL) => None,
        Err(e) => {
            crate::say::line(&[what, b": a link of the path does not resolve inside the rootfs, so the call fails (TODO/interpose.md T-1725)"]);
            crate::set_errno(e);
            Some(core::ptr::null())
        }
    }
}
