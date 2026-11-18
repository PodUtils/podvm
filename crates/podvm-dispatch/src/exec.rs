//! `execve` of the payload: podvm executes itself again, so the kernel
//! keeps each rule of an exec (the threads end, `O_CLOEXEC` descriptors
//! close, caught signals reset), and the new podvm loads the new program.
//!
//! The argv stays the payload's. Two variables carry the rest:
//! [`ROOT_VAR`] (the host path of the root) and [`EXE_VAR`] (the guest path
//! of the program). The working directory is the process's own.

use crate::path::{self, Target};
use crate::state::{EXE, PATH_MAX, ROOT, SELF};
use crate::sys::{self, nr, sc, sc6, AT_EMPTY_PATH, AT_FDCWD};

pub const ROOT_VAR: &[u8] = b"PODVM_DISPATCH_ROOT=";
pub const EXE_VAR: &[u8] = b"PODVM_DISPATCH_EXE=";
pub const EMU_VAR: &[u8] = b"PODVM_DISPATCH_EMU=";
pub const TIER_VAR: &[u8] = b"PODVM_DISPATCH_TIER=";
/// TODO/enter.md T-1983: the exec table of the confine layer, which
/// `pod-enter` writes for the first entry and the handler for each exec.
pub const EXEC_TABLE_VAR: &[u8] = b"PODVM_EXEC_HOST=";

/// Keep the exec table of the confine layer from the environment of the
/// entry (OT-01 of the fourth outside pass: on the tier each program runs
/// through podvm, so Landlock never checks EXECUTE on it). A table that
/// does not fit is a refusal: a run never goes on without its table.
pub(crate) fn table(envp: &[std::ffi::OsString]) -> Result<(), String> {
    use std::os::unix::ffi::OsStrExt;
    let Some(t) = envp
        .iter()
        .find_map(|e| e.as_bytes().strip_prefix(EXEC_TABLE_VAR))
    else {
        return Ok(());
    };
    if !crate::state::EXEC_TABLE.set(t) {
        return Err(format!(
            "the exec table of the confine layer has {} bytes, over the {} that the handler holds",
            t.len(),
            PATH_MAX - 1
        ));
    }
    crate::state::EXEC_ON.store(true, core::sync::atomic::Ordering::Release);
    Ok(())
}

/// Whether the exec table lets the program of the descriptor link `pp`
/// run: no table, or a path of the table covers its host path.
fn exec_allowed(pp: &[u8]) -> bool {
    if !crate::state::EXEC_ON.load(core::sync::atomic::Ordering::Acquire) {
        return true;
    }
    let Some(mut g) = crate::pool::Buf::get() else {
        return false;
    };
    let h = g.buf();
    let n = sys::readlinkat(AT_FDCWD, pp, &mut h[..]);
    n > 0 && pod_confine::exec::allows_in(crate::state::EXEC_TABLE.get(), &h[..n as usize])
}
const X_OK: i64 = 1;
const MAP_PRIVATE_ANON: i64 = 0x22;

/// The guest path of the program into `buf`, and its length.
fn program(dirfd: i64, p: i64, flags: i64, buf: &mut [u8; PATH_MAX]) -> Result<usize, i64> {
    if p == 0 {
        return Err(-sys::EFAULT);
    }
    // SAFETY: the payload gave p as a path.
    let path = unsafe { sys::cstr(p, PATH_MAX) }.ok_or(-sys::ENAMETOOLONG)?;
    if path.is_empty() && flags & AT_EMPTY_PATH == 0 {
        return Err(-sys::ENOENT);
    }
    match path::target(dirfd, path, buf)? {
        Target::Root(n) => Ok(n),
        Target::HostAbs(n) if crate::procself::is_exe(&buf[..n]) => {
            let e = EXE.get();
            buf[..e.len()].copy_from_slice(e);
            buf[e.len()] = 0;
            Ok(e.len())
        }
        // A program outside the root would run with no dispatch.
        _ => Err(-sys::EACCES),
    }
}

/// Check the program as the kernel would: it exists, it is executable, and
/// it is an ELF of x86_64 (static, or dynamic with its interpreter in the
/// root, T-1731) or a script.
fn check(buf: &[u8; PATH_MAX], n: usize) -> i64 {
    let t = Target::Root(n);
    let fd = path::open_path(AT_FDCWD, &[], t, &buf[..], true, false);
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
    // T-1983: the confine layer's `exec=`, as the interposer checks it.
    if !exec_allowed(pp) {
        sys::close(fd);
        return -sys::EACCES;
    }
    let rd = sys::openat2(AT_FDCWD, pp, sys::O_RDONLY | sys::O_CLOEXEC, 0, 0);
    sys::close(fd);
    if rd < 0 {
        return rd;
    }
    let mut head = [0u8; 64];
    // SAFETY: head lives for the call.
    let got = unsafe { sc6(nr::PREAD64, rd, head.as_mut_ptr() as i64, 64, 0, 0, 0) };
    let mut verdict = -sys::ENOEXEC;
    if got >= 2 && head[0] == b'#' && head[1] == b'!' {
        verdict = 0;
    } else if got >= 64
        && crate::state::emulator_for(u16::from_le_bytes([head[18], head[19]])).is_some()
        && head[..4] == *b"\x7fELF"
    {
        // T-1651: the emulator runs an ELF that it serves.
        verdict = 0;
    } else if got >= 64 {
        verdict = match crate::elf::x86_64(&head, |off, out| {
            // SAFETY: out lives for the call.
            unsafe {
                sc6(
                    nr::PREAD64,
                    rd,
                    out.as_mut_ptr() as i64,
                    out.len() as i64,
                    off as i64,
                    0,
                    0,
                )
            }
        }) {
            Ok(_) => 0,
            Err(_) => -sys::ENOEXEC,
        };
    }
    sys::close(rd);
    verdict
}

/// The bytes of the stack of [`execveat`], above one guard page.
const OWN_STACK: usize = 256 << 10;

/// The payload's execveat(2), on a stack of its own.
///
/// ⛔ The handler runs on the payload's alternate signal stack, and Rust's
/// std gives each thread one of `max(SIGSTKSZ, AT_MINSIGSTKSZ)` bytes: 8 KiB
/// on a CPU without AMX. The exec path needs more than 8 KiB in a debug
/// build and 4 to 5 KiB in a release build, with the frame of the kernel
/// (measured on 2026-11-18: gate 44312445769 died of SIGSEGV in the test
/// `exec_loads_the_next_program_in_the_root`). An exec is rare, so it runs
/// on a map of its own; a successful exec drops the map with the process.
/// A nested signal stays safe: [`crate::signals::sigaltstack`] arms each
/// alternate stack with `SS_AUTODISARM`, so the kernel delivers it on this
/// stack, not over the frame of the SIGSYS. Where the map fails, the call
/// runs on the stack that it has.
pub fn execveat(dirfd: i64, p: i64, argv: i64, envp: i64, flags: i64) -> i64 {
    let args = [dirfd, p, argv, envp, flags];
    // SAFETY: an anonymous map with no reserve; its first page is the guard.
    let m = unsafe { sc6(nr::MMAP, 0, (OWN_STACK + 4096) as i64, 3, 0x4022, -1, 0) };
    if m < 0 {
        return execveat_here(&args);
    }
    // SAFETY: the first page of the map.
    unsafe { sc(nr::MPROTECT, m, 4096, 0) };
    let top = m + 4096 + OWN_STACK as i64;
    let r: i64;
    // SAFETY: `top` is the 16-aligned end of a map that nothing else uses;
    // r12 keeps the old stack across the call (callee-saved), and the call
    // keeps the C ABI.
    unsafe {
        core::arch::asm!(
            "mov r12, rsp",
            "mov rsp, {top}",
            "call {f}",
            "mov rsp, r12",
            top = in(reg) top,
            f = sym on_own_stack,
            in("rdi") args.as_ptr(),
            out("r12") _,
            lateout("rax") r,
            clobber_abi("C"),
        );
        sc(nr::MUNMAP, m, (OWN_STACK + 4096) as i64, 0);
    }
    r
}

extern "C" fn on_own_stack(a: *const i64) -> i64 {
    // SAFETY: `a` points at the five arguments of [`execveat`].
    let a = unsafe { core::slice::from_raw_parts(a, 5) };
    execveat_here(a)
}

fn execveat_here(a: &[i64]) -> i64 {
    let (dirfd, p, argv, envp, flags) = (a[0], a[1], a[2], a[3], a[4]);
    let Some(mut g) = crate::pool::Buf::get() else {
        return -sys::ENOMEM;
    };
    let buf = g.buf();
    let n = match program(dirfd, p, flags, buf) {
        Ok(n) => n,
        Err(e) => return e,
    };
    let c = check(buf, n);
    if c < 0 {
        return c;
    }
    // The new environment: the payload's, less our two names, and ours.
    let mut count = 0usize;
    if envp != 0 {
        // SAFETY: the payload gave envp as a NULL-ended array.
        while unsafe { *(envp as *const i64).add(count) } != 0 {
            count += 1;
        }
    }
    let root = ROOT.get();
    let emu = crate::state::EMU.get();
    let emu_spec_len = if emu.is_empty() {
        0
    } else {
        EMU_VAR.len() + 6 + emu.len() + 1
    };
    // T-1657: the syscall tier and its count, as `CAP:FD`.
    let mut tier = [0u8; 48];
    let tier_len = if crate::tier::on() {
        crate::tier::spec(&mut tier)
    } else {
        0
    };
    let strings = ROOT_VAR.len()
        + root.len()
        + 1
        + EXE_VAR.len()
        + n
        + 1
        + emu_spec_len
        + TIER_VAR.len()
        + tier_len
        + 1
        + EXEC_TABLE_VAR.len()
        + crate::state::EXEC_TABLE.get().len()
        + 1;
    let size = ((count + 6) * 8 + strings + 4095) & !4095;
    // SAFETY: an anonymous map for the new array; it ends with the exec.
    let mem = unsafe { sc6(nr::MMAP, 0, size as i64, 3, MAP_PRIVATE_ANON, -1, 0) };
    if mem < 0 {
        return -sys::ENOMEM;
    }
    // SAFETY: mem holds size bytes; each pointer written lies in it.
    unsafe {
        let arr = mem as *mut i64;
        let mut k = 0usize;
        for i in 0..count {
            let e = *(envp as *const i64).add(i);
            let s = sys::cstr(e, 1 << 20).unwrap_or(&[]);
            if s.starts_with(ROOT_VAR)
                || s.starts_with(EXE_VAR)
                || s.starts_with(EMU_VAR)
                || s.starts_with(TIER_VAR)
                || s.starts_with(EXEC_TABLE_VAR)
            {
                continue;
            }
            *arr.add(k) = e;
            k += 1;
        }
        let mut at = mem as *mut u8;
        at = at.add((count + 6) * 8);
        for (name, value) in [(ROOT_VAR, root), (EXE_VAR, &buf[..n])] {
            *arr.add(k) = at as i64;
            k += 1;
            core::ptr::copy_nonoverlapping(name.as_ptr(), at, name.len());
            at = at.add(name.len());
            core::ptr::copy_nonoverlapping(value.as_ptr(), at, value.len());
            at = at.add(value.len());
            *at = 0;
            at = at.add(1);
        }
        if !emu.is_empty() {
            // MACHINE:HOSTPATH, with the machine in decimal.
            let mut digits = [0u8; 5];
            let mut m = crate::state::EMU_MACHINE.load(core::sync::atomic::Ordering::Relaxed);
            let mut d = 0usize;
            loop {
                digits[d] = b'0' + (m % 10) as u8;
                d += 1;
                m /= 10;
                if m == 0 || d == digits.len() {
                    break;
                }
            }
            *arr.add(k) = at as i64;
            k += 1;
            core::ptr::copy_nonoverlapping(EMU_VAR.as_ptr(), at, EMU_VAR.len());
            at = at.add(EMU_VAR.len());
            while d > 0 {
                d -= 1;
                *at = digits[d];
                at = at.add(1);
            }
            *at = b':';
            at = at.add(1);
            core::ptr::copy_nonoverlapping(emu.as_ptr(), at, emu.len());
            at = at.add(emu.len());
            *at = 0;
            at = at.add(1);
        }
        if tier_len > 0 {
            *arr.add(k) = at as i64;
            k += 1;
            core::ptr::copy_nonoverlapping(TIER_VAR.as_ptr(), at, TIER_VAR.len());
            at = at.add(TIER_VAR.len());
            core::ptr::copy_nonoverlapping(tier.as_ptr(), at, tier_len);
            at = at.add(tier_len);
            *at = 0;
            at = at.add(1);
        }
        // T-1983: the handler's own exec table, never the payload's.
        if crate::state::EXEC_ON.load(core::sync::atomic::Ordering::Acquire) {
            let t = crate::state::EXEC_TABLE.get();
            *arr.add(k) = at as i64;
            k += 1;
            core::ptr::copy_nonoverlapping(EXEC_TABLE_VAR.as_ptr(), at, EXEC_TABLE_VAR.len());
            at = at.add(EXEC_TABLE_VAR.len());
            core::ptr::copy_nonoverlapping(t.as_ptr(), at, t.len());
            at = at.add(t.len());
            *at = 0;
        }
        *arr.add(k) = 0;
        let r = sc(nr::EXECVE, SELF.get_z().as_ptr() as i64, argv, mem);
        sc(nr::MUNMAP, mem, size as i64, 0);
        r
    }
}
