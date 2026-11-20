//! TODO/enter.md T-2040: a host path of a volume takes the rules of that
//! volume.
//!
//! ⛔ T-1705 (`rewrite::under_host_side`) lets a path that names the host
//! side of a pair pass with no rewrite, so that a runtime can open the
//! host paths that its loader gives it. The read-only table (`PODVM_RO`)
//! is keyed on guest paths, so a write through the host source of a `:ro`
//! volume (`echo X > HOST/f`, the payload reads HOST in `PODVM_MAPS`)
//! passed it, and the kernel followed each link of such a path on the
//! host root (the sixth outside pass, OQ-01).
//!
//! ⭐ So a host path under the host side of a volume (a pair whose guest
//! side is not `/`) is read as its guest path before each rule: the
//! read-only table, the walk of links in guest names (T-1725) and the
//! volume base (T-1972). The rewrite then gives the same host path again.
//! The rootfs (the pair of `/`) keeps T-1705: its host path passes, and
//! only the read-only table reads it by its guest path ([`ro_denied`]).
//!
//! ⛔ No allocation: each buffer is on the caller's stack.

use super::*;

/// Write the guest path of the host path `buf[..n]` over it, where the
/// path lies under the host side of a volume: the new length, or `None`
/// where it lies under no such side (the buffer is not changed) or the
/// guest path does not fit. The longest host side decides, as in
/// [`longest_to`]. A host side of `/` (a volume of the host root) is left
/// out, as in [`under_host_side`]: each path is under it.
///
/// # Safety
/// `t` borrows the environ of the process.
pub unsafe fn to_volume_guest(t: &Table, buf: &mut [u8; OUT], n: usize) -> Option<usize> {
    if n == 0 || n >= OUT || buf[0] != b'/' {
        return None;
    }
    let (idx, to_n) = unsafe { longest_to(t, buf.as_ptr(), n) }?;
    let p = &t.pairs[idx];
    if to_n < 2 || p.from_n < 2 {
        return None;
    }
    // The guest side, then the rest: the rest starts with `/` or is empty
    // (a match at a component boundary), and the guest side has no
    // trailing `/` (`parse_pairs` strips it).
    let rest = n - to_n;
    let m = p.from_n + rest;
    if m >= OUT {
        return None;
    }
    buf.copy_within(to_n..n, p.from_n);
    // SAFETY: `from_n` bytes of environ memory, into a buffer of `OUT`.
    unsafe { core::ptr::copy_nonoverlapping(p.from_p, buf.as_mut_ptr(), p.from_n) };
    buf[m] = 0;
    Some(m)
}

/// True where the read-only table refuses a write to `g`: by `g` itself,
/// and, where `g` names a place under the host side of a pair (the
/// rootfs, a volume), by its guest path too. So a host path of a `:ro`
/// volume and a host path of the rootfs under `--read-only` answer
/// `EROFS` as their guest paths do.
///
/// # Safety
/// `t` borrows the environ of the process.
pub unsafe fn ro_denied(t: &Table, g: &[u8]) -> bool {
    if unsafe { lookup_env(RO_VAR) }.is_none() {
        return false;
    }
    if unsafe { denied(g.as_ptr(), g.len()) } {
        return true;
    }
    if g.len() >= OUT || !unsafe { under_host_side(t, g.as_ptr(), g.len()) } {
        return false;
    }
    let mut guest = [0u8; OUT];
    match unsafe { unrewrite(t, g.as_ptr(), g.len(), &mut guest) } {
        n if n > 0 => unsafe { denied(guest.as_ptr(), n as usize) },
        // A path that the table never maps (T-0707): the rule of `g` stands.
        -1 => false,
        // A guest path that does not fit: fail safe.
        _ => true,
    }
}
