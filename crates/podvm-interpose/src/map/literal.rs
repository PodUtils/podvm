//! The rewrite of a literal path, with no `dirfd` and no walk.

use super::*;

/// Match and rewrite a path that is already absolute-or-literal, without any
/// `dirfd` resolution. `symlink`'s target takes this shape: the content is
/// resolved when the link is used, not when it is made, so resolving it now
/// would answer about the wrong directory.
///
/// # Safety
/// As [`absolutize`]: a readable path and a writable `OUT`-byte buffer.
pub unsafe fn prepare_literal(
    what: &[u8],
    path: *const c_char,
    out: &mut [u8; OUT],
) -> *const c_char {
    if path.is_null() {
        return path;
    }
    let t = unsafe { table() };
    if t.n == 0 {
        return path;
    }
    let n = strnlen(path, OUT);
    if n >= OUT {
        return path;
    }
    match unsafe { rewrite(&t, path as *const u8, n, out) } {
        -1 => path,
        -2 => {
            crate::say::line(&[what, b": the mapped path does not fit, so the call fails"]);
            crate::set_errno(ENAMETOOLONG);
            core::ptr::null()
        }
        n => {
            out[n as usize] = 0;
            out.as_ptr() as *const c_char
        }
    }
}

/// OS-02 of the outside pass of 2026-11-18: whether the host path `host`
/// (NUL-ended, as [`super::prepare`] gives it) lies in a volume, a pair
/// whose guest side is not `/`. A link made there outlives the run, so its
/// target stays the guest's: the interposer resolves a guest target in
/// each later run, and a target rewritten to the tree of this run would
/// dangle on the host once the tree is gone.
///
/// # Safety
/// `host` is null or a readable NUL-ended path.
pub unsafe fn in_volume(host: *const c_char) -> bool {
    if host.is_null() {
        return false;
    }
    // T-1972: a name under a descriptor that the volume base holds.
    if unsafe { super::base::is_held(host) } {
        return true;
    }
    let n = strnlen(host, OUT);
    // SAFETY: `n` bytes of `host` are readable.
    let h = unsafe { core::slice::from_raw_parts(host as *const u8, n) };
    if h.first() != Some(&b'/') {
        return false;
    }
    in_volume_of(&unsafe { table() }, h)
}

/// [`in_volume`] against the table `t`.
pub fn in_volume_of(t: &Table, h: &[u8]) -> bool {
    t.pairs[..t.n].iter().any(|p| {
        // SAFETY: each pair borrows environ memory of its lengths.
        let (from, to) = unsafe {
            (
                core::slice::from_raw_parts(p.from_p, p.from_n),
                core::slice::from_raw_parts(p.to_p, p.to_n),
            )
        };
        from != b"/"
            && !from.is_empty()
            && to.len() > 1
            && h.starts_with(to)
            && (h.len() == to.len() || h[to.len()] == b'/')
    })
}
