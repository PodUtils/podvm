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

/// TODO/cli.md T-2061: the refusal of `exec` into a container that ended,
/// as docker words it, with its exit code; `None` where the entry goes on.
pub(super) fn not_running(name: &str, state: podvm_supervise::table::State) -> Option<i32> {
    use podvm_supervise::table::State;
    matches!(state, State::Exited | State::Dead).then(|| {
        eprintln!(
            "podvm exec: container {name} is not running ({}). Start it with `podvm start {name}`",
            state.word()
        );
        podvm_image::error::EXIT_CLI_ERROR
    })
}

/// `pod_enter::spawn_joined` of the payload of `pidfd`. TODO/enter.md
/// T-2054 (OQ-08): the relay that pod-enter leaves in the PID namespace of
/// the run is a fork of this process, so this process takes a title and no
/// environment for the fork (`podvm_enter::title`), and its own back after
/// it. With the threads of `-t` the title is refused, and the relay shows
/// podvm's command line as before.
pub(crate) fn spawn_joined(
    root: &podvm_enter::RootDir,
    plan: &podvm_enter::Plan,
    pidfd: &std::os::fd::OwnedFd,
    err: &mut dyn std::io::Write,
) -> podvm_enter::Result<podvm_enter::Child> {
    use std::os::fd::AsRawFd;
    let hidden = podvm_enter::title::hide("podvm-exec");
    let spawned = podvm_enter::spawn_joined(root, plan, pidfd.as_raw_fd() as i64, err);
    if let Ok(saved) = &hidden {
        let _ = podvm_enter::title::restore(saved);
    }
    spawned
}

/// OR-B38-03 of the seventh outside pass (TODO/cli.md T-2075): the parts of
/// `/etc/hosts` that the completion of `exec` writes. The completion
/// rewrites the file, so `exec` gives it the parts of the run (its name and
/// each `--add-host` and `--hostname`), and its own `--add-host` after
/// them. Before, each `exec` wrote the name that the caller typed and its
/// own `--add-host` alone, and dropped those of the run. A record of an
/// older podvm keeps that rule.
pub(super) fn hosts_of(
    ask: &mut crate::complete::Ask,
    target: &str,
    hardening: &podvm_supervise::table::Hardening,
) {
    match &hardening.hosts {
        Some(h) => {
            ask.container_name = h.name.clone();
            let mine = std::mem::take(&mut ask.add_hosts);
            ask.add_hosts = h.added.clone();
            ask.add_hosts
                .extend(mine.into_iter().filter(|m| !h.added.contains(m)));
        }
        None => ask.container_name = Some(target.to_string()),
    }
}

#[cfg(test)]
#[test]
fn exec_writes_the_hosts_of_the_run() {
    use podvm_supervise::table::{Hardening, Hosts};
    let run = Hosts {
        name: Some("ah".into()),
        added: vec!["a:192.0.2.1".into(), "hx:127.0.0.1".into()],
    };
    // (the record, the `--add-host` of exec, the name and the hosts written)
    type Case<'a> = (Option<Hosts>, Vec<&'a str>, Option<&'a str>, Vec<&'a str>);
    let cases: [Case; 4] = [
        (
            Some(run.clone()),
            vec![],
            Some("ah"),
            vec!["a:192.0.2.1", "hx:127.0.0.1"],
        ),
        (
            Some(run.clone()),
            vec!["b:192.0.2.2", "a:192.0.2.1"],
            Some("ah"),
            vec!["a:192.0.2.1", "hx:127.0.0.1", "b:192.0.2.2"],
        ),
        (Some(Hosts::default()), vec![], None, vec![]),
        (
            None,
            vec!["b:192.0.2.2"],
            Some("typed"),
            vec!["b:192.0.2.2"],
        ),
    ];
    for (hosts, mine, name, want) in cases {
        let mut ask = crate::complete::Ask {
            add_hosts: mine.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        };
        let h = Hardening {
            hosts: hosts.clone(),
            ..Default::default()
        };
        hosts_of(&mut ask, "typed", &h);
        assert_eq!(ask.container_name.as_deref(), name, "{hosts:?}");
        assert_eq!(ask.add_hosts, want, "{hosts:?} {mine:?}");
    }
}
