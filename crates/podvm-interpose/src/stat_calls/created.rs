//! TODO/enter.md T-1972: a file that a path call makes in a volume under
//! `--podvm-volume-base`, where the check found the name absent.

use super::*;

/// `creat(2)` is `open(path, O_CREAT | O_WRONLY | O_TRUNC, mode)`: the
/// `open` shape holds a new file under the volume base with no link to
/// follow (TODO/enter.md T-1972).
///
/// # Safety
/// The payload's own contract for `creat(2)`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn creat(path: *const c_char, mode: c_uint) -> c_int {
    use crate::oflags::{O_CREAT, O_TRUNC, O_WRONLY};
    unsafe { crate::open(path, O_CREAT | O_WRONLY | O_TRUNC, mode) }
}

/// `creat64`, as [`creat`].
///
/// # Safety
/// The payload's own contract for `creat64`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn creat64(path: *const c_char, mode: c_uint) -> c_int {
    use crate::oflags::{O_CREAT, O_LARGEFILE, O_TRUNC, O_WRONLY};
    unsafe { crate::open64(path, O_CREAT | O_WRONLY | O_TRUNC | O_LARGEFILE, mode) }
}
/// TODO/enter.md T-1972: the name for an `fopen` shape, where the check of
/// the volume base found its file absent and gave the directory and the
/// name ([`crate::map::take_by_name`]). A mode of `r` answers `ENOENT`; a
/// mode with `x` opens with `O_EXCL`, which follows no link; `w` and `a`
/// make the file here with `O_NOFOLLOW` and give its descriptor's name, so
/// that a link that the host makes at the name is no way out of the base.
/// The descriptor (or -1) closes after the call.
///
/// # Safety
/// `mode` is null or a C string; `p` is a NUL-ended path.
pub(super) unsafe fn by_name(
    mode: *const c_char,
    p: *const c_char,
    made: &mut [u8; 32],
) -> Result<(*const c_char, c_int), ()> {
    if !crate::map::take_by_name() {
        return Ok((p, -1));
    }
    let m = if mode.is_null() {
        &b""[..]
    } else {
        let n = crate::map::strnlen(mode, 16);
        unsafe { core::slice::from_raw_parts(mode as *const u8, n) }
    };
    let m = m.split(|&c| c == b',').next().unwrap_or(b"");
    if !matches!(m.first(), Some(b'w') | Some(b'a')) {
        set_errno(crate::procfs::ENOENT);
        return Err(());
    }
    if m.contains(&b'x') {
        return Ok((p, -1));
    }
    use crate::oflags::{O_CLOEXEC, O_CREAT, O_NOFOLLOW, O_WRONLY};
    let Some(open) = next_open() else {
        set_errno(EINVAL);
        return Err(());
    };
    let fd = unsafe { open(p, O_WRONLY | O_CREAT | O_NOFOLLOW | O_CLOEXEC, 0o666) };
    if fd < 0 {
        return Err(());
    }
    const P: &[u8] = b"/proc/self/fd/";
    let num = crate::say::Num::new(fd as u64);
    let num = num.as_bytes();
    made[..P.len()].copy_from_slice(P);
    made[P.len()..P.len() + num.len()].copy_from_slice(num);
    made[P.len() + num.len()] = 0;
    Ok((made.as_ptr() as *const c_char, fd))
}
