//! The path of a Unix socket on the syscall tier (PodUtils/podrunner#301,
//! TODO/enter.md T-1973). `connect(2)` and `bind(2)` of an `AF_UNIX`
//! address with a path name the path in the guest, as each other path call
//! does; the kernel would look it up on the host. The handler resolves the
//! directory of the socket in the root or its volume (`calls::split`) and
//! gives the kernel `/proc/self/fd/N/NAME`, which stays short and inside
//! the root. An abstract address (a NUL first) and an unnamed one pass as
//! they are.

use crate::sys::{self, sc};

const CONNECT: i64 = 42;
const BIND: i64 = 49;
const AF_UNIX: u16 = 1;
/// `sizeof(struct sockaddr_un)`: the family and 108 bytes of path.
const SUN_LEN: usize = 110;

/// The result of `connect` or `bind` of a Unix socket with a path, or
/// `None` for each other call and address.
pub fn unix(n: i64, a: [i64; 6]) -> Option<i64> {
    if n != CONNECT && n != BIND {
        return None;
    }
    let (addr, len) = (a[1], a[2]);
    if addr == 0 || !(3..=SUN_LEN as i64).contains(&len) {
        return None;
    }
    // SAFETY: the payload gave `len` bytes of a socket address at `addr`.
    let sa = unsafe { core::slice::from_raw_parts(addr as *const u8, len as usize) };
    if u16::from_ne_bytes([sa[0], sa[1]]) != AF_UNIX || sa[2] == 0 {
        return None;
    }
    let raw = &sa[2..];
    let path = &raw[..raw.iter().position(|b| *b == 0).unwrap_or(raw.len())];
    // The path as a C string for `split`.
    let mut cpath = [0u8; SUN_LEN];
    cpath[..path.len()].copy_from_slice(path);
    let mut name = [0u8; 256];
    let (par, np) = match crate::calls::split(sys::AT_FDCWD, cpath.as_ptr() as i64, &mut name) {
        Ok(v) => v,
        Err(e) => return Some(e),
    };
    if np == cpath.as_ptr() as i64 {
        // A path of the host (not in the root or a volume): as given.
        return None;
    }
    let nlen = name.iter().position(|b| *b == 0).unwrap_or(0);
    let Some(out) = proc_addr(par.fd, &name[..nlen]) else {
        return Some(-sys::ENAMETOOLONG);
    };
    // SAFETY: the new address lives for the call; the socket is the payload's.
    Some(unsafe { sc(n, a[0], out.as_ptr() as i64, SUN_LEN as i64) })
}

/// `struct sockaddr_un` of `/proc/self/fd/FD/NAME`, or `None` where it
/// does not fit in 108 bytes with its NUL.
fn proc_addr(fd: i64, name: &[u8]) -> Option<[u8; SUN_LEN]> {
    let mut out = [0u8; SUN_LEN];
    out[..2].copy_from_slice(&AF_UNIX.to_ne_bytes());
    let mut digits = [0u8; 20];
    let mut i = digits.len();
    let mut v = fd.max(0) as u64;
    loop {
        i -= 1;
        digits[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    let parts: [&[u8]; 4] = [b"/proc/self/fd/", &digits[i..], b"/", name];
    let mut at = 2;
    for p in parts {
        if at + p.len() >= SUN_LEN {
            return None;
        }
        out[at..at + p.len()].copy_from_slice(p);
        at += p.len();
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_socket_path_of_the_guest_becomes_a_short_proc_path() {
        let a = super::proc_addr(1001, b"ctl.sock").unwrap();
        let path = &a[2..a[2..].iter().position(|b| *b == 0).unwrap() + 2];
        assert_eq!(path, b"/proc/self/fd/1001/ctl.sock");
        assert!(super::proc_addr(7, &[b'x'; 100]).is_none());
    }
}
