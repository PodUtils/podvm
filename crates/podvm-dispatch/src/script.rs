//! TODO/enter.md T-2015: the `#!` chain of an exec under `exec=` of
//! `--podvm-confine`.
//!
//! On the syscall tier each program runs through podvm, so Landlock never
//! checks EXECUTE on it: the handler checks the exec table itself. The
//! kernel opens the interpreter of a `#!` line for execution, and Landlock
//! checks it on the namespace rung; so the handler checks each interpreter
//! of the chain, and, where the last one is a dynamic loader, each argument
//! that the loader can take as its program ([`pod_confine::loader::each_program`]),
//! with the arguments that the kernel gives it.
//!
//! ⛔ No allocation: the heads of the chain are in one buffer of the pool.

use crate::parse::{shebang, HEAD};
use crate::path;
use crate::sys::{self, nr, sc6, AT_FDCWD};

/// The `#!` levels that the kernel follows before it answers ELOOP
/// (`fs/exec.c`), as the interposer and `enter::resolve` count them: the
/// one number of the interposer's `parse.rs`.
pub const DEPTH: usize = crate::parse::DEPTH;

const X_OK: i64 = 1;

/// One level of the chain: the interpreter and its argument, as ranges of
/// the buffer of the heads.
#[derive(Clone, Copy, Default)]
pub struct Level {
    pub interp: (usize, usize),
    pub arg: Option<(usize, usize)>,
}

/// The arguments that the kernel gives the last interpreter before
/// `file0` and the caller's `argv[1..]`: the argument of the last level,
/// then the interpreter and the argument of each level above it, from the
/// deepest (`fs/binfmt_script.c` puts each level's in front). The count of
/// `out`.
pub fn lead<'a>(heads: &'a [u8], levels: &[Level], out: &mut [&'a [u8]; 2 * DEPTH]) -> usize {
    let mut k = 0usize;
    let span = |r: (usize, usize)| &heads[r.0..r.1];
    if let Some(last) = levels.last() {
        if let Some(a) = last.arg {
            out[k] = span(a);
            k += 1;
        }
        for l in levels.iter().rev().skip(1) {
            out[k] = span(l.interp);
            k += 1;
            if let Some(a) = l.arg {
                out[k] = span(a);
                k += 1;
            }
        }
    }
    k
}

/// The head of `fd` into `out`: the bytes read, or `-errno`.
fn read_head(fd: i64, out: &mut [u8]) -> i64 {
    // SAFETY: out lives for the call.
    unsafe {
        sc6(
            nr::PREAD64,
            fd,
            out.as_mut_ptr() as i64,
            out.len() as i64,
            0,
            0,
            0,
        )
    }
}

/// The interpreter `name` of a `#!` line, judged as the kernel opens it:
/// it resolves in the root (relative to the working directory), it is
/// executable, and the exec table covers it. Its host path goes to `last`
/// with its length; the result is a descriptor to read its head, or
/// `-errno`. 0 is a file that this cannot read: the kernel runs it with
/// execute alone, so it ends the chain.
fn open_interp(name: &[u8], last: &mut [u8; crate::state::PATH_MAX], len: &mut usize) -> i64 {
    let t = match path::target(AT_FDCWD, name, last) {
        Ok(t) => t,
        Err(e) => return e,
    };
    let fd = path::open_path(AT_FDCWD, name, t, &last[..], true, false);
    if fd < 0 {
        return fd;
    }
    let mut link = [0u8; 32];
    let pp = path::fd_link(fd, &mut link);
    // SAFETY: the proc path lives for the call.
    let r = unsafe { sc6(nr::FACCESSAT2, AT_FDCWD, pp.as_ptr() as i64, X_OK, 0, 0, 0) };
    if r < 0 {
        sys::close(fd);
        return r;
    }
    let n = sys::readlinkat(AT_FDCWD, pp, &mut last[..]);
    if n <= 0 || !crate::exec::exec_allowed(pp) {
        sys::close(fd);
        crate::meta::say(b"podvm: syscall tier: execve: --podvm-confine denies the exec of the interpreter of the script (exec=, TODO/enter.md T-2015)\n");
        return -sys::EACCES;
    }
    *len = n as usize;
    let rd = sys::openat2(AT_FDCWD, pp, sys::O_RDONLY | sys::O_CLOEXEC, 0, 0);
    sys::close(fd);
    match rd {
        r if r == -sys::EACCES => 0,
        r => r,
    }
}

/// Whether `exec=` lets the script of the descriptor `rd` run: 0, or
/// `-errno` as the kernel or the confine layer answers. `file0` is the
/// guest path of the script, and `argv` the payload's array. Call it where
/// the exec table is on.
pub fn chain(rd: i64, file0: &[u8], argv: i64) -> i64 {
    let Some(mut hb) = crate::pool::Buf::get() else {
        return -sys::ENOMEM;
    };
    let Some(mut lb) = crate::pool::Buf::get() else {
        return -sys::ENOMEM;
    };
    let heads = hb.buf();
    let last = lb.buf();
    let mut last_n = 0usize;
    let mut levels = [Level::default(); DEPTH];
    let mut depth = 0usize;
    let mut fd = rd;
    loop {
        let off = depth * HEAD;
        let got = read_head(fd, &mut heads[off..off + HEAD]);
        if fd != rd {
            sys::close(fd);
        }
        if got < 0 {
            return got;
        }
        let got = got as usize;
        let h = &heads[off..off + got];
        if got < 2 || h[0] != b'#' || h[1] != b'!' {
            break;
        }
        if depth == DEPTH {
            return -sys::ELOOP;
        }
        let Some(sb) = shebang(h, got < HEAD) else {
            return -sys::ENOEXEC;
        };
        levels[depth] = Level {
            interp: (off + sb.interp.0, off + sb.interp.1),
            arg: sb.arg.map(|(a, b)| (off + a, off + b)),
        };
        depth += 1;
        let (a, b) = levels[depth - 1].interp;
        let next = open_interp(&heads[a..b], last, &mut last_n);
        if next < 0 {
            return next;
        }
        if next == 0 {
            break;
        }
        fd = next;
    }
    if depth == 0 || !pod_confine::loader::is_loader(&last[..last_n]) {
        return 0;
    }
    let heads: &[u8] = &heads[..];
    let mut front: [&[u8]; 2 * DEPTH] = [&[]; 2 * DEPTH];
    let k = lead(heads, &levels[..depth], &mut front);
    let rest = (1..)
        .map(|i| {
            if argv == 0 {
                return 0;
            }
            // SAFETY: the payload gave argv as a NULL-ended array.
            unsafe { *(argv as *const i64).add(i) }
        })
        .take_while(|&p| p != 0)
        // SAFETY: the payload gave each entry as a C string. One that does
        // not fit is empty, and an empty program is refused.
        .map(|p| unsafe { sys::cstr(p, crate::state::PATH_MAX) }.unwrap_or(&[]));
    let args = front[..k].iter().copied().chain([file0]).chain(rest);
    if pod_confine::loader::each_program(args, crate::exec::program_allowed) {
        return 0;
    }
    crate::meta::say(b"podvm: syscall tier: execve: --podvm-confine denies the exec of the program that the dynamic loader of the script would run (exec=, TODO/enter.md T-2015)\n");
    -sys::EACCES
}
