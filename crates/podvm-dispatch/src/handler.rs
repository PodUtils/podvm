//! The SIGSYS handler: each system call of the payload comes here.
//!
//! The kernel gives the call in the registers of the trap
//! (`PR_SET_SYSCALL_USER_DISPATCH`); the handler serves it from podvm's own
//! code, whose calls do not dispatch, and writes the result to RAX.
//!
//! ⛔ A forced SIGSYS that arrives while SIGSYS is blocked resets the
//! handler to SIG_DFL and the kernel kills the process. So the handler has
//! `SA_NODEFER`, and each mask of the payload loses SIGSYS.

use core::sync::atomic::Ordering;

use crate::signals::{keep_thread_state, sigaction, sigaltstack, sigprocmask, KAction};

use crate::state::{self, BRK_BASE, BRK_CUR, BRK_END};
use crate::sys::{self, nr, sc, sc6};

pub const SIGSYS: i64 = 31;
pub const SA_SIGINFO: u64 = 4;
pub const SA_ONSTACK: u64 = 0x0800_0000;
pub const SA_RESTORER: u64 = 0x0400_0000;
pub const SA_NODEFER: u64 = 0x4000_0000;
const CLONE_VM: i64 = 0x100;
const CLONE_VFORK: i64 = 0x4000;
const CLONE_SIGHAND: i64 = 0x800;
const CLONE_THREAD: i64 = 0x1_0000;
const SIGCHLD: i64 = 17;
const PR_SET_SYSCALL_USER_DISPATCH: i64 = 59;
/// The stack of a vfork child, and the part of the given stack it keeps.
const VFORK_STACK: i64 = 256 << 10;
const WINDOW: i64 = 256;
const GETPID: i64 = 39;
const PROCESS_VM_READV: i64 = 310;

/// The registers of `ucontext_t.uc_mcontext.gregs` (x86_64).
pub mod reg {
    pub const R8: usize = 0;
    pub const R9: usize = 1;
    pub const R10: usize = 2;
    pub const RDI: usize = 8;
    pub const RSI: usize = 9;
    pub const RDX: usize = 12;
    pub const RAX: usize = 13;
    pub const RIP: usize = 16;
    pub const EFL: usize = 17;
    /// The offset of `gregs` in `ucontext_t`.
    pub const OFFSET: usize = 40;
}

unsafe extern "C" {
    pub(crate) fn podvm_dispatch_restorer();
    fn podvm_dispatch_clone_thread(flags: i64, sp: i64, ptid: i64, ctid: i64, tls: i64) -> i64;
    fn podvm_dispatch_unmap_exit(addr: i64, len: i64) -> !;
}

core::arch::global_asm!(
    // munmap(addr, len), then exit(0) of the thread, with no access to the
    // stack between them: the stack is in the range (musl's `__unmapself`
    // of a detached thread). TODO/enter.md T-1652, found by T-1666.
    ".globl podvm_dispatch_unmap_exit",
    "podvm_dispatch_unmap_exit:",
    "mov eax, 11",
    "syscall",
    "xor edi, edi",
    "mov eax, 60",
    "syscall",
    "ud2",
    ".globl podvm_dispatch_restorer",
    "podvm_dispatch_restorer:",
    "mov eax, 15",
    "syscall",
    "ud2",
    // clone(flags, sp, ptid, ctid, tls). The child starts on `sp` with the
    // frame that `clone` wrote below it: the region, then the registers of
    // the payload, its flags and its RIP. It arms dispatch for itself and
    // goes back to the payload with RAX = 0 and RSP = sp.
    ".globl podvm_dispatch_clone_thread",
    "podvm_dispatch_clone_thread:",
    "mov r10, rcx",
    "mov eax, 56",
    "syscall",
    "test rax, rax",
    "jz 2f",
    "ret",
    "2:",
    "mov rdx, [rsp - 152]",
    "mov r10, [rsp - 144]",
    "mov edi, 59",
    "mov esi, 1",
    "xor r8d, r8d",
    "mov eax, 157",
    "syscall",
    "lea rsp, [rsp - 136]",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop r11",
    "pop r10",
    "pop r9",
    "pop r8",
    "pop rbp",
    "pop rdi",
    "pop rsi",
    "pop rdx",
    "pop rcx",
    "pop rbx",
    "pop rax",
    "popfq",
    "ret",
);

/// Arm dispatch for the calling thread: each call outside podvm's code
/// traps.
pub fn arm() -> i64 {
    let start = state::REGION_START.load(Ordering::Relaxed) as i64;
    let len = state::REGION_LEN.load(Ordering::Relaxed) as i64;
    // SAFETY: a NULL selector: the kernel reads no memory.
    unsafe { sc6(nr::PRCTL, PR_SET_SYSCALL_USER_DISPATCH, 1, start, len, 0, 0) }
}

/// The action of SIGSYS: this handler, on the payload's signal stack.
pub fn sigsys_action() -> KAction {
    KAction {
        handler: on_sigsys as *const () as usize as u64,
        flags: SA_SIGINFO | SA_RESTORER | SA_ONSTACK | SA_NODEFER,
        restorer: podvm_dispatch_restorer as *const () as usize as u64,
        mask: 0,
    }
}

fn brk(want: i64) -> i64 {
    let base = BRK_BASE.load(Ordering::Relaxed) as i64;
    let end = BRK_END.load(Ordering::Relaxed) as i64;
    let cur = BRK_CUR.load(Ordering::Relaxed) as i64;
    if base == 0 || want < base || want > end {
        return cur;
    }
    let page = |v: i64| (v + 4095) & !4095;
    if page(want) > page(cur) {
        // SAFETY: the range lies in the reserved range of the break.
        let r = unsafe { sc(nr::MPROTECT, page(cur), page(want) - page(cur), 3) };
        if r < 0 {
            return cur;
        }
    }
    BRK_CUR.store(want as usize, Ordering::Relaxed);
    want
}

/// clone, fork and vfork. A child of a copied address space arms dispatch
/// in this handler; a thread starts in the trampoline.
fn clone(a: [i64; 6], g: *mut i64) -> i64 {
    let (mut flags, sp, ptid, ctid, tls) = (a[0], a[1], a[2], a[3], a[4]);
    if sp == 0 && flags & CLONE_VM != 0 {
        // A vfork on the same stack would write over this handler's frame
        // in the parent: it becomes a fork, which the child cannot tell.
        if flags & (CLONE_THREAD | CLONE_SIGHAND) != 0 {
            return -sys::EINVAL;
        }
        flags &= !CLONE_VM;
    }
    if sp == 0 {
        // SAFETY: a fork: the child returns here with a copy of the frame.
        let r = unsafe { sc6(nr::CLONE, flags, 0, ptid, ctid, tls, 0) };
        if r == 0 {
            arm();
        }
        return r;
    }
    // ⛔ A vfork child on a stack of its own (musl's posix_spawn gives
    // about 5 KiB) has no room for this handler and the kernel's frame: an
    // overflow wrote over the parent's `args` (measured: a fault at
    // address 59). The parent waits for the child's exec or exit, so the
    // child takes a larger stack for that time: the top of the given one,
    // where the libc keeps the function and its argument, is copied there.
    let mut moved = 0i64;
    let mut sp = sp;
    if flags & CLONE_VM != 0 && flags & CLONE_VFORK != 0 {
        // SAFETY: an anonymous map for the child's stack.
        let m = unsafe { sc6(nr::MMAP, 0, VFORK_STACK, 3, 0x22, -1, 0) };
        if m >= 0 {
            let top = (m + VFORK_STACK - WINDOW) & !15;
            // SAFETY: the window above sp is the libc's, in the parent's
            // stack, up to the end of its map; the copy lands in the new map.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    sp as *const u8,
                    top as *mut u8,
                    window_of(sp) as usize,
                )
            };
            sp = top;
            moved = m;
        }
    }
    // The frame of the trampoline, below the child's stack.
    let f = (sp - 152) as *mut i64;
    // SAFETY: the payload gave sp as the top of a stack it owns; the frame
    // lies in the unused part below it. g holds the trap's registers.
    unsafe {
        let gr = |i: usize| *g.add(i);
        let regs = [
            gr(7),
            gr(6),
            gr(5),
            gr(4),
            gr(3),
            gr(2),
            gr(1),
            gr(0), // r15..r8
            gr(10),
            gr(8),
            gr(9),
            gr(12),
            gr(14),
            gr(11),
            0, // rbp rdi rsi rdx rcx rbx rax
        ];
        *f = state::REGION_START.load(Ordering::Relaxed) as i64;
        *f.add(1) = state::REGION_LEN.load(Ordering::Relaxed) as i64;
        for (i, v) in regs.iter().enumerate() {
            *f.add(2 + i) = *v;
        }
        *f.add(17) = gr(reg::EFL);
        *f.add(18) = gr(reg::RIP);
        let r = podvm_dispatch_clone_thread(flags, sp, ptid, ctid, tls);
        if moved != 0 {
            // The child has exec'd or exited: its stack is free.
            sc(nr::MUNMAP, moved, VFORK_STACK, 0);
        }
        r
    }
}

/// TODO/enter.md T-1935: the bytes of the window above `sp` that can be
/// read. glibc's `clone` puts `sp` 16 bytes below the end of the map of
/// `posix_spawn`, so a copy of the whole window read past the map (into the
/// guard page of Rust's alternate stack, measured with `uv venv`), and the
/// spawn died of SIGSEGV. musl keeps more than that above `sp`, in the same
/// map. `process_vm_readv` of this process answers EFAULT for a page that
/// cannot be read; `mincore` does not tell a guard page from a stack.
fn window_of(sp: i64) -> i64 {
    let next = (sp & !4095) + 4096;
    if sp + WINDOW <= next {
        return WINDOW;
    }
    let mut byte = 0u8;
    let local = [core::ptr::addr_of_mut!(byte) as i64, 1];
    let remote = [next, 1];
    // SAFETY: getpid takes no argument; process_vm_readv writes one byte
    // into `byte` and reads one byte of this process at `next`, or fails.
    let r = unsafe {
        let pid = sc(GETPID, 0, 0, 0);
        sc6(
            PROCESS_VM_READV,
            pid,
            local.as_ptr() as i64,
            1,
            remote.as_ptr() as i64,
            1,
            0,
        )
    };
    if r == 1 {
        WINDOW
    } else {
        next - sp
    }
}

/// The descriptors of the handler that the payload must not close: the
/// root's, the count of `/tmp` (T-1657) and each volume's (T-1724; OS-01
/// of the outside pass of 2026-11-18: Python's `posix_spawn` closed them
/// with `close_range` before its exec, which then gave EBADF). Unset ones
/// are -1.
fn kept() -> [i64; KEEP] {
    let mut k = [-1i64; KEEP];
    k[0] = state::root_fd();
    k[1] = crate::tier::FD.load(core::sync::atomic::Ordering::Relaxed);
    for (i, v) in state::vols().iter().enumerate() {
        k[2 + i] = v.fd.load(core::sync::atomic::Ordering::Relaxed);
    }
    k
}

const KEEP: usize = 2 + state::MAX_VOLS;

/// Keep the descriptors of [`kept`] out of reach of close, dup2, dup3 and
/// close_range.
fn guard_fd(n: i64, a: [i64; 6]) -> Option<i64> {
    if !matches!(n, nr::CLOSE | nr::DUP2 | nr::DUP3 | nr::CLOSE_RANGE) {
        return None;
    }
    let keep = kept();
    let held = |fd: i64| fd >= 0 && keep.contains(&(fd as i32 as i64));
    match n {
        nr::CLOSE if held(a[0]) => Some(-sys::EBADF),
        nr::DUP2 | nr::DUP3 if held(a[1]) => Some(-sys::EBUSY),
        nr::CLOSE_RANGE => {
            let mut out = [(0i64, 0i64); KEEP + 1];
            let n = range_gaps(a[0], a[1], &keep, &mut out)?;
            for &(lo, hi) in &out[..n] {
                // SAFETY: close_range of a range with no kept descriptor.
                unsafe { sc(nr::CLOSE_RANGE, lo, hi, a[2]) };
            }
            Some(0)
        }
        _ => None,
    }
}

/// TODO/enter.md T-1935: the parts of `lo..=hi` around the kept
/// descriptors (a negative one is none) into `out`, and their count; or
/// `None` where the range holds none of them and the call goes to the
/// kernel as it is. Python's `subprocess` closes each descriptor above 2
/// with one `close_range`.
pub(crate) fn range_gaps(lo: i64, hi: i64, keep: &[i64], out: &mut [(i64, i64)]) -> Option<usize> {
    let mut k = [i64::MAX; KEEP];
    let mut m = 0usize;
    for &f in keep {
        if f >= 0 && lo <= f && f <= hi && m < KEEP && !k[..m].contains(&f) {
            k[m] = f;
            m += 1;
        }
    }
    if m == 0 {
        return None;
    }
    k[..m].sort_unstable();
    let mut n = 0usize;
    let mut from = lo;
    for &f in &k[..m] {
        if from < f && n < out.len() {
            out[n] = (from, f - 1);
            n += 1;
        }
        from = f + 1;
    }
    if from <= hi && n < out.len() {
        out[n] = (from, hi);
        n += 1;
    }
    Some(n)
}

/// True where `munmap(addr, len)` takes the stack that this handler runs
/// on: the return of the handler would fault. musl ends a detached thread
/// so (`__unmapself`: `munmap` of its own stack, then `exit`).
fn unmaps_this_stack(addr: i64, len: i64) -> bool {
    let here = 0u8;
    let sp = core::ptr::addr_of!(here) as i64;
    len > 0 && addr <= sp && sp < addr.saturating_add(len)
}

/// The SIGSYS handler.
///
/// # Safety
/// The kernel calls it with a SIGSYS of dispatch.
pub unsafe extern "C" fn on_sigsys(_sig: i32, info: *mut u8, uc: *mut u8) {
    // SAFETY: the kernel's siginfo and ucontext for this signal.
    unsafe {
        let g = uc.add(reg::OFFSET) as *mut i64;
        let n = *(info.add(24) as *const i32) as i64;
        let a = [
            *g.add(reg::RDI),
            *g.add(reg::RSI),
            *g.add(reg::RDX),
            *g.add(reg::R10),
            *g.add(reg::R8),
            *g.add(reg::R9),
        ];
        // T-1976: a call that returns runs on a stack of the handler's
        // pool. A call that does not return here (clone, the exits, an
        // exec, which has its own) or that reads the stack it runs on
        // (munmap) stays on the payload's.
        let stays = matches!(
            n,
            nr::CLONE
                | nr::FORK
                | nr::VFORK
                | nr::EXECVE
                | nr::EXECVEAT
                | nr::MUNMAP
                | nr::RT_SIGRETURN
                | nr::EXIT
                | nr::EXIT_GROUP
        );
        let r = if stays {
            call(n, a, g)
        } else {
            crate::stacks::run(call, n, a, g).unwrap_or_else(|| call(n, a, g))
        };
        *g.add(reg::RAX) = r;
        if r >= 0 {
            keep_thread_state(n, a, uc);
        }
    }
}

fn call(n: i64, a: [i64; 6], g: *mut i64) -> i64 {
    if let Some(r) = guard_fd(n, a) {
        return r;
    }
    if let Some(r) = crate::ro::gate(n, a) {
        return r; // TODO/enter.md T-1901: a `:ro` volume
    }
    if let Some(r) = crate::meta::gate(n, a) {
        return r; // TODO/enter.md T-1985: `--podvm-confine`, mode, owner, times
    }
    if let Some(r) = crate::uts::call(n, a) {
        return r; // TODO/enter.md T-1908: `--hostname`
    }
    if let Some(r) = crate::tier::call(n, a) {
        return r;
    }
    match n {
        nr::CLONE => clone(a, g),
        nr::FORK => clone([SIGCHLD, 0, 0, 0, 0, 0], g),
        nr::VFORK => clone([CLONE_VFORK | SIGCHLD, 0, 0, 0, 0, 0], g),
        nr::CLONE3 => -sys::ENOSYS,
        // SAFETY: g is the gregs of this frame; its mask lies after them.
        nr::EXECVE | nr::EXECVEAT => unsafe {
            let frame = *((g as *mut u8).sub(reg::OFFSET).add(296) as *const u64);
            let (d, p, v, e, f) = if n == nr::EXECVE {
                (sys::AT_FDCWD, a[0], a[1], a[2], 0)
            } else {
                (a[0], a[1], a[2], a[3], a[4])
            };
            crate::exec::execveat(d, p, v, e, f, frame)
        },
        nr::RT_SIGACTION => sigaction(a[0], a[1], a[2], a[3]),
        nr::RT_SIGPROCMASK => sigprocmask(a[0], a[1], a[2], a[3]),
        nr::RT_SIGRETURN => -sys::ENOSYS,
        // SAFETY: g is the gregs of this frame.
        nr::SIGALTSTACK => unsafe { sigaltstack(a[0], a[1], g) },
        nr::BRK => brk(a[0]),
        nr::MUNMAP if unmaps_this_stack(a[0], a[1]) => {
            // SAFETY: the payload unmaps the stack that this handler runs
            // on, and its next call is the exit of the thread.
            unsafe { podvm_dispatch_unmap_exit(a[0], a[1]) }
        }
        nr::PRCTL if a[0] == PR_SET_SYSCALL_USER_DISPATCH => -sys::EINVAL,
        // SAFETY: each other call runs as the payload gave it.
        _ => crate::calls::path_call(n, a)
            .unwrap_or_else(|| unsafe { sc6(n, a[0], a[1], a[2], a[3], a[4], a[5]) }),
    }
}
