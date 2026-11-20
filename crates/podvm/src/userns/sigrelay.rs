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

const SIGALRM: i32 = 14;
const SIGPWR: i32 = 30;

const SA_SIGINFO: i32 = 4;
const SA_RESTART: i32 = 0x1000_0000;

unsafe extern "C" {
    fn sigaction(sig: i32, act: *const SigAction, old: *mut SigAction) -> i32;
    fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
}

/// TODO/supervise.md T-2006: pid 1 of the PID namespace gets SIGKILL when
/// the outer process dies while the run goes on (a SIGKILL of podvm, the
/// bound of a caller). The end of pid 1 ends each process of the namespace;
/// before, pid 1, pid 2 and the payload stayed. `on` is false from the
/// moment pid 2 ended: the outer process then exits first by design, and
/// pid 1 stays as the reaper of the helpers (T-1781).
pub(in crate::userns) fn die_with_outer(on: bool) {
    const PR_SET_PDEATHSIG: i32 = 1;
    const SIGKILL: u64 = 9;
    // SAFETY: prctl with integers. The kernel delivers a SIGKILL of an
    // ancestor PID namespace to the pid 1 of a namespace.
    unsafe { prctl(PR_SET_PDEATHSIG, if on { SIGKILL } else { 0 }, 0, 0, 0) };
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
        // podcrates `v0.79.0` (T-995): the set of `pod_enter::WAIT_FORWARDED`.
        for s in [SIGHUP, SIGTERM, SIGUSR1, SIGUSR2, SIGALRM, SIGPWR] {
            signal(s, forward as *const () as usize);
        }
        for s in [SIGINT, SIGQUIT] {
            if !install(s) {
                signal(s, SIG_IGN); // #535: the old rule as the fallback
            }
        }
    }
}

/// TODO/cli.md T-2001 OP-05: the outer process relays each signal that
/// the waiter of pod-enter forwards to the payload, so that none ends the
/// outer process first; the parity row of `run` names each one.
#[cfg(test)]
#[test]
fn the_outer_process_relays_each_signal_that_the_waiter_forwards() {
    let names = [
        (SIGHUP, "SIGHUP"),
        (SIGINT, "SIGINT"),
        (SIGQUIT, "SIGQUIT"),
        (SIGUSR1, "SIGUSR1"),
        (SIGUSR2, "SIGUSR2"),
        (SIGTERM, "SIGTERM"),
        (SIGALRM, "SIGALRM"),
        (SIGPWR, "SIGPWR"),
    ];
    let mask: u64 = names.iter().map(|(n, _)| 1u64 << (n - 1)).sum();
    assert_eq!(
        mask,
        podvm_enter::WAIT_FORWARDED,
        "the set of pod-enter changed"
    );
    let row = crate::parity::TABLE
        .iter()
        .find(|r| r.verb == "run" && r.flag.is_none())
        .expect("the row of run");
    for (_, name) in names {
        assert!(row.note.contains(name), "{name}: {}", row.note);
    }
}
