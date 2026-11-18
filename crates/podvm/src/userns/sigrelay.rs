//! PodUtils/podvm#535: SIGINT and SIGQUIT of a run in podvm's own user
//! namespace. A terminal sends them to the whole process group, which holds
//! pid 2 and the payload, so a forward would deliver them twice; and a
//! `kill -INT` of another process reaches podvm alone, so ignoring them lost
//! it. The kernel tells the two apart: a signal of the terminal driver has
//! `si_code` SI_KERNEL (a code above 0), one of `kill(2)` a code of 0 or
//! below. This handler forwards the second only.
//!
//! ⚠ Where `sigaction` fails, the old rule stays: ignored, as before.

/// `struct sigaction` of musl on x86_64.
#[repr(C)]
struct SigAction {
    handler: usize,
    mask: [u64; 16],
    flags: i32,
    restorer: usize,
}

use super::{forward, signal, SIGHUP, SIGINT, SIGQUIT, SIGTERM, SIGUSR1, SIGUSR2, SIG_IGN};

const SA_SIGINFO: i32 = 4;
const SA_RESTART: i32 = 0x1000_0000;

unsafe extern "C" {
    fn sigaction(sig: i32, act: *const SigAction, old: *mut SigAction) -> i32;
}

/// The handler: forward a signal that a process sent; leave one of the
/// terminal, which reached the group already.
extern "C" fn from_a_process(sig: i32, info: *const i32, _ctx: *const u8) {
    // SAFETY: the kernel gives a `siginfo_t`; `si_code` is its third int.
    let code = if info.is_null() {
        1
    } else {
        unsafe { *info.add(2) }
    };
    if code <= 0 {
        forward(sig);
    }
}

/// Install the handler for `sig`; false where `sigaction` refused it.
pub(super) fn install(sig: i32) -> bool {
    let act = SigAction {
        handler: from_a_process as *const () as usize,
        mask: [0; 16],
        flags: SA_SIGINFO | SA_RESTART,
        restorer: 0,
    };
    // SAFETY: a valid action that lives for the call; no old action read.
    unsafe { sigaction(sig, &act, core::ptr::null_mut()) == 0 }
}

/// Install `forward` for the shutdown signals, and for the two that the
/// terminal sends to the whole process group, the handler of this module.
pub(in crate::userns) fn relay_signals() {
    // SAFETY: forward is async-signal-safe; the handlers stay for the life
    // of the process.
    unsafe {
        for s in [SIGHUP, SIGTERM, SIGUSR1, SIGUSR2] {
            signal(s, forward as *const () as usize);
        }
        for s in [SIGINT, SIGQUIT] {
            if !install(s) {
                signal(s, SIG_IGN); // #535: the old rule as the fallback
            }
        }
    }
}
