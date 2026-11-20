//! The namespaces that `exec` and `cp` join: the UTS namespace of the
//! payload (T-1908) and its mount namespace (T-1775). Split out of
//! [`super::enter`] (the limit of 500 lines, TODO/gate.md T-1674).

/// Join the UTS namespace of the payload of `pidfd`, so that `exec` sees
/// the name of `--hostname` (T-1908). Where it is the namespace of this
/// process already, or the join is refused, nothing changes.
pub(crate) fn join_uts(pidfd: &std::os::fd::OwnedFd) {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn setns(fd: i32, nstype: i32) -> i32;
    }
    const CLONE_NEWUTS: i32 = 0x0400_0000;
    // SAFETY: a pidfd and a namespace type; a UTS join has no rule of one
    // thread.
    unsafe { setns(pidfd.as_raw_fd(), CLONE_NEWUTS) };
}

/// A pidfd of `pid` (Linux 5.3), or `None`.
pub(crate) fn pidfd_open(pid: i64) -> Option<std::os::fd::OwnedFd> {
    use std::os::fd::FromRawFd;
    unsafe extern "C" {
        fn syscall(n: std::ffi::c_long, ...) -> std::ffi::c_long;
    }
    const SYS_PIDFD_OPEN: std::ffi::c_long = 434;
    // SAFETY: pidfd_open takes a pid and flags and returns a descriptor.
    let fd = unsafe {
        syscall(
            SYS_PIDFD_OPEN,
            pid as std::ffi::c_long,
            0 as std::ffi::c_long,
        )
    };
    (fd >= 0).then(|| unsafe { std::os::fd::OwnedFd::from_raw_fd(fd as i32) })
}

/// Join the mount namespace of the payload `pid`; false where it is
/// refused (T-1775).
pub(crate) fn join_mounts(pid: Option<i64>) -> bool {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn setns(fd: i32, nstype: i32) -> i32;
    }
    const CLONE_NEWNS: i32 = 0x0002_0000;
    crate::userns::rejoin(pid); // TODO/enter.md T-1910: `cp`
    let Some(fd) = crate::userns::joined_pidfd().or_else(|| pid.and_then(pidfd_open)) else {
        return false;
    };
    // SAFETY: a pidfd and a namespace type; podvm cp runs on one thread.
    unsafe { setns(fd.as_raw_fd(), CLONE_NEWNS) == 0 }
}

/// True where `exec` enters the container by a join of its namespaces: a
/// running container of the namespace rung with a pid (PodUtils/podrunner#326).
pub(crate) fn joins_the_run(
    state: podvm_supervise::table::State,
    rung: &str,
    pid: Option<i64>,
) -> bool {
    state == podvm_supervise::table::State::Running && rung == "namespace" && pid.is_some()
}
