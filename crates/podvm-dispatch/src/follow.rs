//! A symbolic link of the guest that `RESOLVE_IN_ROOT` at one base cannot
//! follow (OR-06 of the outside pass of 2026-11-18, TODO/enter.md T-1975).
//!
//! The handler resolves a path at the root or at its volume with
//! `RESOLVE_IN_ROOT`, so an absolute link inside a volume (a venv in `/tmp`
//! or in a work directory of a volume: `v/bin/python -> /usr/bin/python3`)
//! resolves against the volume, not against the root of the container. Where
//! such a resolution fails, [`walk`] follows the path one name at a time, by
//! the rules of the guest: an absolute target starts again at `/`, which the
//! longest volume prefix serves ([`crate::path::base_of`]). It runs only
//! after a failure, so a path that resolves costs nothing more.

use crate::path::base_of;
use crate::state::PATH_MAX;
use crate::sys::{self, nr, sc6};

/// The most links of one walk, as the kernel's `MAXSYMLINKS`.
const MAX_LINKS: usize = 40;
const S_IFMT: u32 = 0o170000;
const S_IFLNK: u32 = 0o120000;

/// Rewrite the guest path `buf[..n]` (absolute, NUL at `n`) to one with no
/// link that crosses a base, following the last name where `last`. The new
/// length, or `None` where a name is missing or the links loop: the caller
/// then keeps the error of its first try.
pub fn walk(buf: &mut [u8; PATH_MAX], mut n: usize, last: bool) -> Option<usize> {
    let mut links = 0usize;
    let mut tmp = crate::pool::Buf::get()?;
    let t = tmp.buf();
    'again: loop {
        // `i`: the start of the next name; the prefix `buf[..i]` has no link.
        let mut i = 1usize;
        while i < n {
            while i < n && buf[i] == b'/' {
                i += 1;
            }
            if i >= n {
                break;
            }
            let end = buf[i..n]
                .iter()
                .position(|&c| c == b'/')
                .map_or(n, |p| i + p);
            // `buf[..i]` holds no link, so `.` and `..` fold by their text:
            // `/w/..` is `/`, also where `/w` is a volume.
            if &buf[i..end] == b"." || &buf[i..end] == b".." {
                let from = if &buf[i..end] == b".." {
                    let up = i.saturating_sub(1);
                    buf[..up]
                        .iter()
                        .rposition(|&c| c == b'/')
                        .map_or(1, |p| p + 1)
                } else {
                    i
                };
                let tail = if end < n { end + 1 } else { n };
                buf.copy_within(tail..n, from);
                n -= tail - from;
                buf[n] = 0;
                i = from;
                continue;
            }
            let is_last = end == n || buf[end..n].iter().all(|&c| c == b'/');
            if is_last && !last {
                return Some(n);
            }
            let (fd, at) = base_of(&buf[..end]);
            let saved = buf[end];
            buf[end] = 0;
            let rest: &[u8] = if at >= end { b".\0" } else { &buf[at..=end] };
            let rest = if rest.first() == Some(&b'/') {
                &rest[1..]
            } else {
                rest
            };
            let rest = if rest.is_empty() || rest == b"\0" {
                &b".\0"[..]
            } else {
                rest
            };
            let o = sys::openat2(
                fd,
                rest,
                sys::O_PATH | sys::O_NOFOLLOW | sys::O_CLOEXEC,
                0,
                sys::RESOLVE_IN_ROOT,
            );
            buf[end] = saved;
            if o < 0 {
                return None;
            }
            let mut st = [0u8; 144];
            // SAFETY: fstat writes one struct stat of 144 bytes.
            let r = unsafe { sc6(nr::FSTAT, o, st.as_mut_ptr() as i64, 0, 0, 0, 0) };
            let mode = u32::from_ne_bytes([st[24], st[25], st[26], st[27]]);
            if r < 0 || mode & S_IFMT != S_IFLNK {
                sys::close(o);
                i = end;
                continue;
            }
            let tl = sys::readlinkat(o, b"\0", &mut t[..]);
            sys::close(o);
            links += 1;
            if tl <= 0 || links > MAX_LINKS {
                return None;
            }
            let tl = tl as usize;
            // The new path: the target (absolute: as it is; relative: after
            // the directory of the link), then the rest after the link.
            let tail_len = n - end;
            let head = if t[0] == b'/' { 0 } else { i };
            let new_len = head + tl + tail_len;
            if new_len >= PATH_MAX {
                return None;
            }
            buf.copy_within(end..n, head + tl);
            buf[head..head + tl].copy_from_slice(&t[..tl]);
            n = new_len;
            buf[n] = 0;
            continue 'again;
        }
        return Some(n);
    }
}
