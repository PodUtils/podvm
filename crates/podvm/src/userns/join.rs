//! `exec` and `cp` into a run in a user namespace (TODO/enter.md T-1910).

use super::*;

/// TODO/enter.md T-1910: the variable that asks a new podvm to join the
/// user, the UTS and the PID namespace of the process that it names (a
/// host pid), first in `main`.
pub const JOIN_VAR: &str = "PODVM_USERNS_JOIN";
pub(super) static JOINED: AtomicBool = AtomicBool::new(false);
pub(super) static JOIN_FD: AtomicI32 = AtomicI32::new(-1);

/// A new pidfd of the payload that this process joined, or `None`. The
/// record names a host pid, which names nothing in the PID namespace.
pub fn joined_pidfd() -> Option<std::os::fd::OwnedFd> {
    use std::os::fd::BorrowedFd;
    let fd = JOIN_FD.load(Ordering::Relaxed);
    if fd < 0 {
        return None;
    }
    // SAFETY: the descriptor stays open for the life of the process.
    let b = unsafe { BorrowedFd::borrow_raw(fd) };
    b.try_clone_to_owned().ok()
}

/// Join the namespaces of [`JOIN_VAR`]. As the owner of the user
/// namespace, this process holds each capability in it after the join, so
/// the join of the mount namespace (T-1765) works after it. A refused
/// join is named, and the verb goes on as before.
pub(super) fn join_if() {
    let Ok(v) = std::env::var(JOIN_VAR) else {
        return;
    };
    JOINED.store(true, Ordering::Relaxed);
    // SAFETY: one thread (the contract of `enter_if`).
    unsafe { std::env::remove_var(JOIN_VAR) };
    let Ok(pid) = v.parse::<i64>() else {
        return;
    };
    const PIDFD_OPEN: i64 = 434;
    // SAFETY: pidfd_open takes two integers.
    let fd = unsafe { syscall(PIDFD_OPEN, pid, 0) };
    // TODO/probe.md T-1912: only the namespaces that differ. The entry of
    // pod-enter through a user namespace (`run -d`) opens no PID and no UTS
    // namespace, and a join of the host's own PID namespace needs a right
    // that the caller lacks there (EPERM).
    let differs = |ns: &str| {
        let ours = std::fs::read_link(format!("/proc/self/ns/{ns}"));
        let theirs = std::fs::read_link(format!("/proc/{pid}/ns/{ns}"));
        matches!((&ours, &theirs), (Ok(a), Ok(b)) if a != b)
    };
    let mut flags = CLONE_NEWUSER;
    if differs("uts") {
        flags |= CLONE_NEWUTS;
    }
    let pid_ns = differs("pid");
    if pid_ns {
        flags |= CLONE_NEWPID;
    }
    // SAFETY: a pidfd and namespace types, on one thread.
    let joined = fd >= 0 && unsafe { setns(fd as i32, flags) } == 0;
    if !joined {
        // ⛔ Not on with the verb: outside the namespaces of the run, the
        // command would act on the host's processes and files.
        eprintln!(
            "podvm: the namespaces of the run (pid {pid}) refused the join: {} (TODO/enter.md T-1910)",
            std::io::Error::last_os_error()
        );
        std::process::exit(125);
    }
    JOIN_FD.store(fd as i32, Ordering::Relaxed);
    if !pid_ns {
        return;
    }
    // After `setns(CLONE_NEWPID)` this process makes no thread, and its
    // children go into the PID namespace: a child goes on with the verb as
    // a member of it, and this process waits and relays, as in `enter_if`.
    // SAFETY: one thread.
    let child = unsafe { fork() };
    if child < 0 {
        eprintln!(
            "podvm: fork in the PID namespace of the run: {} (TODO/enter.md T-1910)",
            std::io::Error::last_os_error()
        );
        std::process::exit(125);
    }
    if child > 0 {
        CHILD.store(child, Ordering::Relaxed);
        super::sigrelay::relay_signals();
        std::process::exit(wait_for(child));
    }
}

/// TODO/enter.md T-1910: where the running payload `pid` (a host pid) is
/// in another user namespace, start this podvm again with the same
/// arguments and [`JOIN_VAR`]. It returns where nothing is to join, where
/// this process joined already, or where the new start fails.
pub fn rejoin(pid: Option<i64>) {
    use std::os::unix::process::CommandExt;
    let Some(pid) = pid.filter(|_| !JOINED.load(Ordering::Relaxed)) else {
        return;
    };
    let ours = std::fs::read_link("/proc/self/ns/user");
    let theirs = std::fs::read_link(format!("/proc/{pid}/ns/user"));
    if !matches!((&ours, &theirs), (Ok(a), Ok(b)) if a != b) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut args = std::env::args_os();
    let arg0 = args.next().unwrap_or_default();
    let e = std::process::Command::new(exe)
        .arg0(arg0)
        .args(args)
        .env(JOIN_VAR, pid.to_string())
        .exec();
    eprintln!("podvm: starting again to join the run: {e} (TODO/enter.md T-1910)");
}
