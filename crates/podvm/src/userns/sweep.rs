//! The end of the PID namespace of a run: the sweep at the exit of pid 2
//! and the reaper that pid 1 becomes (TODO/probe.md T-1909).

use super::*;

/// In pid 2: name the detached prefill of T-1781 (`keep`), which the
/// sweep at the exit of pid 2 spares. The sweep sends SIGKILL to each
/// other process of the PID namespace but pid 1 and pid 2: a process of
/// the payload that the sweep of the launcher (T-1708) did not end would
/// keep pid 1 waiting. Outside a namespace of podvm's own it does nothing.
pub fn sweep(keep: Option<u32>) {
    if !active() {
        return;
    }
    if let Some(k) = keep {
        KEEP.store(k, Ordering::Relaxed);
    }
}

/// The prefill that [`sweep`] keeps, 0 for none.
pub(super) static KEEP: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// The sweep itself, at each exit of pid 2 (`atexit`): `main`'s own end,
/// and each `std::process::exit` of a verb (the bound of `--timeout`).
/// The pid that registered [`sweep_at_exit`]: a child of fork inherits the
/// handler, and must not sweep.
pub(super) static OWNER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub(super) extern "C" fn sweep_at_exit() {
    let me = std::process::id();
    if OWNER.load(Ordering::Relaxed) != me {
        return;
    }
    let keep = Some(KEEP.load(Ordering::Relaxed)).filter(|k| *k != 0);
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return;
    };
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        if pid == 1 || pid == me || Some(pid) == keep {
            continue;
        }
        // SAFETY: kill takes two integers; the pids are of this namespace.
        unsafe { kill(pid as i32, 9) };
    }
    // TODO/cli.md T-2087: wait until each process that got SIGKILL has
    // ended and pid 1 reaped it, so that pid 1 sees the prefill alone, or
    // nothing and ends with the run. A process that stays past the bound
    // keeps pid 1 as its reaper, as before.
    let bound = std::time::Duration::from_millis(podvm_supervise::tree::TREE_KILL_MS);
    let deadline = std::time::Instant::now() + bound;
    while std::time::Instant::now() < deadline {
        let mut st = 0;
        // SAFETY: st lives for the call; WNOHANG (1) does not wait.
        while unsafe { waitpid(-1, &mut st, 1) } > 0 {}
        if others_but(keep) == Some(0) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// In pid 1 after the end of pid 2: reap each ended child, and true where
/// no child is left (ECHILD): pid 1 then has no process to wait for.
pub(super) fn none_left() -> bool {
    const WNOHANG: i32 = 1;
    const ECHILD: i32 = 10;
    loop {
        let mut st = 0;
        // SAFETY: st lives for the call; WNOHANG does not wait.
        let r = unsafe { waitpid(-1, &mut st, WNOHANG) };
        if r > 0 {
            continue;
        }
        // SAFETY: errno of this thread.
        let e = unsafe { *__errno_location() };
        if r < 0 && e == EINTR {
            continue;
        }
        return r < 0 && e == ECHILD;
    }
}

/// TODO/cli.md T-2087 (OR-B40-05): in pid 2, SIGKILL to each process of
/// the PID namespace but pid 1 and pid 2. In pid 2, `kill(-1)` names
/// exactly these. pid 2 is no child subreaper in its own namespace, so an
/// orphan of the payload is a child of pid 1, which reaps it at once.
/// False outside a namespace of podvm's own.
pub fn kill_others() -> bool {
    if !active() {
        return false;
    }
    // SAFETY: kill takes two integers.
    unsafe { kill(-1, 9) };
    true
}

/// TODO/supervise.md T-1708 in pid 2: end each other process of the PID
/// namespace but pid 1, reap the children of pid 2, and wait until no such
/// process is left, for at most `bound`. The count of the processes that
/// stayed; `Unknown` where `/proc` cannot count them. None outside a
/// namespace of podvm's own: the caller ends its tree as before.
pub fn end_tree(bound: std::time::Duration) -> Option<podvm_supervise::Residual> {
    if !active() {
        return None;
    }
    const WNOHANG: i32 = 1;
    const ESRCH: i32 = 3;
    let deadline = std::time::Instant::now() + bound;
    loop {
        let mut st = 0;
        // SAFETY: st lives for the call; WNOHANG does not wait.
        while unsafe { waitpid(-1, &mut st, WNOHANG) } > 0 {}
        // SAFETY: signal 0 checks for a target only. ESRCH: no process of
        // the namespace is left but pid 1 and pid 2 (a zombie of pid 1
        // counts until pid 1 reaps it).
        let none = unsafe { kill(-1, 0) } < 0 && unsafe { *__errno_location() } == ESRCH;
        if none {
            return Some(podvm_supervise::Residual::Count(0));
        }
        if std::time::Instant::now() >= deadline {
            return Some(match others() {
                Some(n) => podvm_supervise::Residual::Count(n),
                None => podvm_supervise::Residual::Unknown,
            });
        }
        kill_others();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// The count of the processes of the PID namespace but pid 1 and this one,
/// from the `/proc` of the namespace (`own_proc`); None where it cannot be
/// read.
fn others() -> Option<u64> {
    others_but(None)
}

/// [`others`], also without `keep`.
fn others_but(keep: Option<u32>) -> Option<u64> {
    let me = std::process::id();
    let rd = std::fs::read_dir("/proc").ok()?;
    let n = rd
        .flatten()
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()))
        .filter(|p| *p != 1 && *p != me && Some(*p) != keep)
        .count();
    Some(n as u64)
}

pub(super) const SIGALRM: i32 = 14;
/// How long pid 1 waits for the detached prefill after pid 2 ended.
pub(super) const LINGER_SECS: u32 = 600;

/// In pid 1: SIGKILL to each process of the PID namespace.
pub(super) extern "C" fn end_namespace(_: i32) {
    // SAFETY: kill is async-signal-safe; -1 in pid 1 names each process of
    // the namespace but pid 1.
    unsafe { kill(-1, 9) };
}

/// pid 1 after pid 2 ended: the same process with no word of the run in
/// its command line, which reaps until no process of the namespace is
/// left, for at most [`LINGER_SECS`].
pub const REAPER_ARG: &str = "--podvm-ns-reaper";

pub(super) fn reap_all() -> ! {
    // SAFETY: the handler only calls kill.
    unsafe {
        signal(SIGALRM, end_namespace as *const () as usize);
        alarm(LINGER_SECS);
    }
    loop {
        let mut st = 0;
        // SAFETY: st lives for the call.
        let r = unsafe { waitpid(-1, &mut st, 0) };
        // SAFETY: errno of this thread.
        if r < 0 && unsafe { *__errno_location() } != EINTR {
            std::process::exit(0);
        }
    }
}
