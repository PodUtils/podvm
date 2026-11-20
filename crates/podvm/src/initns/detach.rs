//! TODO/enter.md T-2002: `run -d` and `create` with the init of the image
//! on the namespace rung.
//!
//! A foreground run boots the init from pid 2 of podvm's own user
//! namespace ([`crate::userns`]). A container that stays has a record with
//! a pid, and the launcher of `pod_supervise` as its parent, so the same
//! supervisor ([`super::boot::supervise`]) starts from the launcher:
//!
//! ```text
//! the launcher            the lock, the control socket, the record
//!   the outer process     `podvm guest-init --detached`: a user namespace
//!                         with the subordinate range; the pid of the
//!                         record; it relays each signal and waits
//!     the supervisor      PID 1 of a PID namespace of its own, with a
//!                         mount and a UTS namespace
//!       the init          PID 1 of a second PID namespace, with its own
//!                         mount, cgroup, IPC and UTS namespace
//!       the command       joins the namespaces of the init after the boot
//! ```
//!
//! The supervisor is PID 1 of a PID namespace because of the end by
//! SIGKILL (`rm -f`, `kill`, the bound of `stop`). The init of a PID
//! namespace ends only when each process of its namespace is reaped, also
//! the command, whose parent (the supervisor) is outside that namespace.
//! With the supervisor as a plain child of the launcher, the command went
//! to the launcher after the SIGKILL of the supervisor, and the launcher
//! waits for one process at a time: it waited 5 s for the init, the init
//! waited for the reap of the command, and `rm -f` kept the record
//! ("1 process of its tree stayed", measured 2026-11-20). The kernel reaps
//! each child of a PID 1 that ends, so nothing of the run waits for a
//! process outside it.
//!
//! The supervisor writes the state of the init beside the record
//! ([`super::hold::Held`]): the launcher waits for it, so `run -d` returns
//! when the boot is complete or failed, and `exec` reads it
//! ([`super::enter`]). A `stop` is a SIGTERM of the supervisor, which the
//! command gets; when the command ends, the supervisor asks the init for
//! its power off, and the end of the supervisor is the end of each process
//! of the PID namespace.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use podvm_supervise::launcher::{InitJob, InitStarted};

use super::boot::{self, Spec};
use super::hold::{write, Held};
use super::{cgroup, mounts, scope, VERB};

/// The first argument of the supervisor of a detached run.
pub const FLAG: &str = "--detached";
/// The time that the launcher gives the supervisor over the bound of the
/// boot: the owners of the tree, the binds and the maps come before it.
const OVER: Duration = Duration::from_secs(60);
/// The mark that the tree has the owners of the image already.
const OWNERS_MARK: &str = ".podvm-owners";

const CLONE_NEWNS: i32 = 0x0002_0000;
const CLONE_NEWUTS: i32 = 0x0400_0000;
const CLONE_NEWPID: i32 = 0x2000_0000;
/// The `/proc` of the PID namespace of the caller, opened by the outer
/// process: the supervisor reads the pid that the caller has for the init.
static CALLER_PROC: std::sync::OnceLock<std::fs::File> = std::sync::OnceLock::new();
const PR_SET_PDEATHSIG: i32 = 1;
const SIGKILL: i32 = 9;

unsafe extern "C" {
    fn unshare(flags: i32) -> i32;
    fn fork() -> i32;
    fn dup2(old: i32, new: i32) -> i32;
    fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    fn syscall(n: i64, ...) -> i64;
}

/// In `main`, with one thread: give the launcher its entry for an init of
/// the image, and run the supervisor where the arguments ask for it.
pub fn start(argv: &[String]) {
    let _ = podvm_supervise::launcher::INIT_ENTRY.set(spawn);
    let word = |i: usize| argv.get(i).map(String::as_str);
    if word(1) == Some(VERB) && word(2) == Some(FLAG) {
        std::process::exit(serve(&argv[3..]));
    }
}

/// The delegated cgroup2 directory of this process, also from the user
/// manager of systemd where `ask` allows the question (TODO/enter.md
/// T-2004), or the reason for none.
pub fn delegated(ask: bool) -> Result<PathBuf, String> {
    match cgroup::delegated() {
        Err(first) if ask => match scope::ask() {
            Ok(()) => cgroup::delegated(),
            Err(why) => Err(format!("{first}; {why}")),
        },
        other => other,
    }
}

/// The base of the cgroups of a run: the launcher removes it after the end
/// of the supervisor, which is a process of one of its leaves.
struct Base(PathBuf);

impl Drop for Base {
    fn drop(&mut self) {
        cgroup::remove(&self.0);
    }
}

/// The arguments of the supervisor of the container of `job`, with the
/// cgroups of the run under `base`.
pub fn args(job: &InitJob<'_>, base: &Path) -> Vec<String> {
    let c = job.container;
    let mut a = vec![
        VERB.to_string(),
        FLAG.to_string(),
        "--state".into(),
        job.state.to_string_lossy().into_owned(),
        "--cgroup".into(),
        base.to_string_lossy().into_owned(),
        "--root".into(),
        c.rootfs.clone(),
        "--cwd".into(),
        c.working_dir.clone(),
        "--".into(),
    ];
    a.extend(c.argv.iter().cloned());
    a
}

/// The entry of the launcher: start the supervisor with the environment
/// of the record and the stdio of the container, and wait for the state of
/// the init. An error is the line of the failure, and no process and no
/// cgroup of the run stays then.
pub fn spawn(job: &InitJob<'_>) -> Result<InitStarted, String> {
    use std::os::unix::process::CommandExt;
    // The state of an earlier start names an init that is gone.
    let _ = std::fs::remove_file(&job.state);
    let me = std::env::current_exe().map_err(|e| format!("the path of podvm: {e}"))?;
    // Here, before the user namespace of the supervisor: the user manager
    // of systemd answers the caller, not uid 0 of a namespace.
    let delegated = delegated(true).map_err(|why| {
        format!(
            "--podvm-init=image: the host delegates no cgroup2 subtree to the caller ({why}), \
             and systemd as PID 1 needs one (TODO/enter.md T-2002)"
        )
    })?;
    // A random name: two runs share the delegated directory, and a pid is
    // not unique across PID namespaces.
    let nonce = crate::machine::guest::init::nonce()?;
    cgroup::sweep(&delegated);
    let base = Base(delegated.join(format!("{}{nonce}", cgroup::BASE_PREFIX)));
    let mut c = std::process::Command::new(me);
    c.args(args(job, &base.0)).env_clear();
    for (k, v) in job.container.env.iter().filter_map(|e| e.split_once('=')) {
        c.env(k, v);
    }
    let pass = job.pass.to_vec();
    // SAFETY: the closure makes system calls only.
    unsafe {
        c.pre_exec(move || {
            // The supervisor ends with the launcher, and the init with it.
            prctl(PR_SET_PDEATHSIG, SIGKILL as u64, 0, 0, 0);
            for (child, host) in &pass {
                if (0..3).contains(child) && dup2(*host as i32, *child as i32) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let child = c
        .spawn()
        .map_err(|e| format!("the start of the supervisor of the init: {e}"))?;
    let pid = child.id() as i32;
    // The launcher reaps its payload; the handle of std must not.
    std::mem::forget(child);
    wait_state(&job.state, pid)?;
    Ok(InitStarted {
        pid: i64::from(pid),
        keep: Box::new(base),
    })
}

/// Wait in the launcher until the supervisor `pid` wrote the state of the
/// init. A failed boot, a supervisor that ended with no state, and the
/// bound each give their own line; the supervisor is reaped then.
fn wait_state(file: &Path, pid: i32) -> Result<(), String> {
    let bound = crate::incus::systemd::BOOT_BOUND + OVER;
    let end = Instant::now() + bound;
    let mut gone: Option<i32> = None;
    let reap = || {
        let mut st = 0;
        // SAFETY: st lives for the call.
        unsafe { waitpid(pid, &mut st, 0) };
    };
    loop {
        match Held::read(file) {
            Ok(Held::Up { .. }) => return Ok(()),
            Ok(Held::Failed(why)) => {
                reap();
                return Err(format!("--podvm-init=image: {why} (TODO/enter.md T-2002)"));
            }
            Err(e) => {
                // One more read after the end of the supervisor: its write
                // comes before its end.
                if let Some(status) = gone {
                    return Err(format!(
                        "--podvm-init=image: the supervisor of the init ended with status \
                         {status} and gave no state ({e}) (TODO/enter.md T-2002)"
                    ));
                }
                if Instant::now() >= end {
                    boot::signal(pid, boot::KILL);
                    reap();
                    return Err(format!(
                        "--podvm-init=image: the supervisor of the init gave no state in {} s \
                         (TODO/enter.md T-2002)",
                        bound.as_secs()
                    ));
                }
                gone = boot::ended(pid).map(boot::code_of);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The pid that the caller of podvm has for the process `pid` of the PID
/// namespace of this process: the `Pid:` of a pidfd in the `/proc` of the
/// caller. `pid` itself where this process is in no PID namespace of its
/// own (a foreground run, a guest).
pub fn host_view(pid: i32) -> Result<i32, String> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let Some(caller) = CALLER_PROC.get() else {
        return Ok(pid);
    };
    const PIDFD_OPEN: i64 = 434;
    // SAFETY: pidfd_open takes two integers and gives a descriptor.
    let fd = unsafe { syscall(PIDFD_OPEN, i64::from(pid), 0i64) };
    if fd < 0 {
        return Err(format!(
            "a pidfd of the init {pid}: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: the descriptor of this call, owned once.
    let pidfd = unsafe { std::fs::File::from_raw_fd(fd as i32) };
    let info = format!(
        "/proc/self/fd/{}/self/fdinfo/{}",
        caller.as_raw_fd(),
        pidfd.as_raw_fd()
    );
    let text = std::fs::read_to_string(&info).map_err(|e| format!("{info}: {e}"))?;
    caller_pid(&text).ok_or(format!(
        "the /proc of the caller names no pid for the init {pid}"
    ))
}

/// The pid of the `Pid:` line of the `fdinfo` of a pidfd: more than 0 for
/// a process that the `/proc` of that file can see.
pub fn caller_pid(fdinfo: &str) -> Option<i32> {
    fdinfo
        .lines()
        .find_map(|l| l.strip_prefix("Pid:"))
        .and_then(|v| v.trim().parse().ok())
        .filter(|p: &i32| *p > 0)
}

/// The outer process and the supervisor of a detached run: the exit code
/// of the command, or 125 with one line on stderr (the log of the
/// container) and in the state file (the caller of `run -d`).
fn serve(args: &[String]) -> i32 {
    let mut spec = match super::parse(args) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("podvm {VERB}: {e}");
            return 125;
        }
    };
    let fail = |spec: &Spec, why: String| {
        if let Some(f) = &spec.state {
            let _ = write(f, &Held::Failed(why.clone()));
        }
        boot::refuse(&why)
    };
    // No descriptor of the launcher (its lock, its socket, the lock of the
    // image) reaches the init or the command; this process keeps them.
    keep_descriptors_here();
    match outer() {
        Err(why) => fail(&spec, why),
        // The outer process: the pid of the record.
        Ok(child) if child > 0 => crate::userns::sigrelay::wait_relaying(child),
        Ok(_) => match prepare(&mut spec) {
            Ok(()) => boot::supervise(&spec),
            Err(why) => fail(&spec, why),
        },
    }
}

/// The user namespace, the PID namespace of the supervisor, and the fork:
/// 0 in the supervisor (PID 1 of that namespace), its pid in the outer
/// process. An error is the step that failed.
fn outer() -> Result<i32, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let step = |what: &str| format!("{what}: {}", std::io::Error::last_os_error());
    if !crate::userns::enter_alone() {
        return Err(
            "the user namespace with the subordinate id range of the caller did not open \
             (newuidmap, newgidmap, and a range in /etc/subuid and /etc/subgid)"
                .to_string(),
        );
    }
    let proc = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(crate::userns::O_DIRECTORY)
        .open("/proc")
        .map_err(|e| format!("/proc of the caller: {e}"))?;
    let _ = CALLER_PROC.set(proc);
    // SAFETY: one thread; the next child is PID 1 of the new namespace.
    if unsafe { unshare(CLONE_NEWPID) } != 0 {
        return Err(step("a PID namespace of the supervisor"));
    }
    // SAFETY: one thread; the child goes on as the supervisor.
    let child = unsafe { fork() };
    if child < 0 {
        return Err(step("the fork of the supervisor"));
    }
    if child == 0 {
        // The supervisor ends with the outer process, and the init with it.
        crate::userns::sigrelay::die_with_outer(true);
    }
    Ok(child)
}

/// Each step of the supervisor before the boot: its mount namespace, the
/// owners of the tree, the volumes. An error is the step that failed.
fn prepare(spec: &mut Spec) -> Result<(), String> {
    let env: Vec<String> = std::env::vars_os()
        .map(|(k, v)| format!("{}={}", k.to_string_lossy(), v.to_string_lossy()))
        .collect();
    let var = |name: &str| {
        env.iter()
            .find_map(|e| e.strip_prefix(name)?.strip_prefix('='))
    };
    if spec.cgroup.is_none() {
        return Err("the supervisor got no cgroup2 directory (--cgroup)".to_string());
    }
    // SAFETY: one thread; the binds of the volumes stay in this namespace.
    if unsafe { unshare(CLONE_NEWNS | CLONE_NEWUTS) } != 0 {
        return Err(format!(
            "a mount and a UTS namespace of the supervisor: {}",
            std::io::Error::last_os_error()
        ));
    }
    mounts::private().map_err(|e| format!("the mounts of / as private: {e}"))?;
    // The pids of the init and of the command name them in this /proc.
    mounts::own_proc().map_err(|e| format!("/proc of the PID namespace of the supervisor: {e}"))?;
    if let Some(name) = var("PODVM_HOSTNAME") {
        crate::userns::set_hostname(name).map_err(|e| format!("--hostname {name}: {e}"))?;
    }
    owners_once(&spec.root);
    // A volume at or under /tmp goes onto the tmpfs of the init; each
    // other one is a bind on the tree, which the init takes with it.
    let (mut at_tmp, mut rest, mut ro) = (Vec::new(), Vec::new(), Vec::new());
    for b in podvm_supervise::launcher::binds_of(&env) {
        if b.guest == "/tmp" || b.guest.starts_with("/tmp/") {
            let suffix = if b.read_only { ":ro" } else { "" };
            let text = format!("{}:{}{suffix}", b.guest, b.host);
            at_tmp.push(
                mounts::Bind::parse(&text)
                    .ok_or(format!("-v {}:{}: no clean path", b.host, b.guest))?,
            );
        } else {
            if b.read_only {
                ro.push(b.guest.clone());
            }
            rest.push(podvm_enter::userland::Volume {
                guest: b.guest,
                host: b.host,
            });
        }
    }
    crate::userns::bind_onto(&spec.root, &rest, &ro, var("PODVM_BINDS_BASE"))?;
    spec.tmp = Some(at_tmp);
    spec.ids = var(crate::interpose::IDENTITY_VAR)
        .and_then(|v| v.split_once(':'))
        .and_then(|(u, g)| Some((u.parse().ok()?, g.parse().ok()?)))
        .filter(|ids| *ids != (0, 0));
    if let Some(ids) = spec.ids {
        spec.groups = super::image_groups(&spec.root.to_string_lossy(), ids);
    }
    spec.verbose = var(super::LOG_VAR) == Some("1");
    spec.discard = var(super::DISCARD_VAR) == Some("1");
    spec.tty = false;
    Ok(())
}

/// Give the tree the owners of the image, one time for a container: a
/// later start keeps each owner that a service changed.
fn owners_once(root: &Path) {
    let Some(mark) = root.parent().map(|d| d.join(OWNERS_MARK)) else {
        return;
    };
    if mark.exists() {
        return;
    }
    crate::userns::owners::apply(root);
    let _ = std::fs::write(mark, "");
}

/// Set close-on-exec on each descriptor from 3 (`close_range` with
/// `CLOSE_RANGE_CLOEXEC`, Linux 5.11); an older kernel gets one `fcntl`
/// for each entry of `/proc/self/fd`.
fn keep_descriptors_here() {
    const CLOSE_RANGE: i64 = 436;
    const CLOSE_RANGE_CLOEXEC: u32 = 4;
    // SAFETY: three integers; the call changes a flag of each descriptor.
    if unsafe { syscall(CLOSE_RANGE, 3u32, u32::MAX, CLOSE_RANGE_CLOEXEC) } == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir("/proc/self/fd") else {
        return;
    };
    let fds: Vec<i64> = entries
        .flatten()
        .filter_map(|e| e.file_name().to_string_lossy().parse().ok())
        .filter(|fd| *fd > 2)
        .collect();
    for fd in fds {
        const F_SETFD: u64 = 2;
        const FD_CLOEXEC: u64 = 1;
        let _ = podvm_probe::sys::fcntl(fd, F_SETFD, FD_CLOEXEC);
    }
}
