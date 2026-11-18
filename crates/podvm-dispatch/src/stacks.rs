//! Stacks of the handler, out of the payload's alternate signal stack
//! (TODO/enter.md T-1976).
//!
//! ⛔ The SIGSYS handler has `SA_ONSTACK`, so it starts on the alternate
//! signal stack of the payload's thread: 8 KiB from Rust's std on a CPU
//! without AMX, and the frame of the kernel takes about 3 KiB of it. A deep
//! call of the handler ran past its end in a debug build (gate 44312445769:
//! an exec; gate 44345876471: a rename). So [`run`] takes a stack of this
//! pool for the call: 64 stacks of 128 KiB, each above a guard page, taken
//! with a compare-and-swap. Where none is free, the call runs on the stack
//! that it has.
//!
//! A nested signal stays off the frame of the SIGSYS: `signals::sigaltstack`
//! arms each alternate stack of the payload with `SS_AUTODISARM`, so while a
//! handler runs, the kernel delivers the next signal on the current stack.

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::sys::{nr, sc, sc6};

const SLOTS: usize = 64;
const SIZE: usize = 128 << 10;
const GUARD: usize = 4096;
const STRIDE: usize = SIZE + GUARD;
static BASE: AtomicUsize = AtomicUsize::new(0);
static USED: AtomicU64 = AtomicU64::new(0);

/// Reserve the pool. Call it before the payload runs; a failure leaves the
/// handler on the payload's stacks.
pub fn init() {
    // SAFETY: an anonymous map with no reserve.
    let m = unsafe { sc6(nr::MMAP, 0, (SLOTS * STRIDE) as i64, 3, 0x4022, -1, 0) };
    if m < 0 {
        return;
    }
    for i in 0..SLOTS {
        // SAFETY: the lowest page of each slot, in the map above.
        unsafe { sc(nr::MPROTECT, m + (i * STRIDE) as i64, GUARD as i64, 0) };
    }
    BASE.store(m as usize, Ordering::Release);
}

fn take() -> Option<usize> {
    let mut cur = USED.load(Ordering::Relaxed);
    while cur != u64::MAX {
        let bit = (!cur).trailing_zeros() as usize;
        match USED.compare_exchange_weak(cur, cur | 1 << bit, Ordering::Acquire, Ordering::Relaxed)
        {
            Ok(_) => return Some(bit),
            Err(now) => cur = now,
        }
    }
    None
}

struct Call {
    f: fn(i64, [i64; 6], *mut i64) -> i64,
    n: i64,
    a: [i64; 6],
    g: *mut i64,
}

extern "C" fn on_stack(c: *const Call) -> i64 {
    // SAFETY: `c` points at the call that [`run`] holds for this call.
    let c = unsafe { &*c };
    (c.f)(c.n, c.a, c.g)
}

/// `f(n, a, g)` on a stack of the pool, or `None` where the pool has none.
pub fn run(f: fn(i64, [i64; 6], *mut i64) -> i64, n: i64, a: [i64; 6], g: *mut i64) -> Option<i64> {
    let base = BASE.load(Ordering::Acquire);
    if base == 0 {
        return None;
    }
    let slot = take()?;
    let top = (base + slot * STRIDE + STRIDE) as i64;
    let c = Call { f, n, a, g };
    let r: i64;
    // SAFETY: `top` is the 16-aligned end of a slot that this call holds;
    // r12 keeps the old stack across the call (callee-saved), and the call
    // keeps the C ABI.
    unsafe {
        core::arch::asm!(
            "mov r12, rsp",
            "mov rsp, {top}",
            "call {f}",
            "mov rsp, r12",
            top = in(reg) top,
            f = sym on_stack,
            in("rdi") &c as *const Call,
            out("r12") _,
            lateout("rax") r,
            clobber_abi("C"),
        );
    }
    USED.fetch_and(!(1u64 << slot), Ordering::Release);
    Some(r)
}
