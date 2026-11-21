//! The supervisor of an init of the image (TODO/enter.md T-2002): the
//! process `podvm guest-init`.
//!
//! Three kinds of process:
//!
//! * the supervisor (this one) stays in the PID namespace of its parent. It
//!   opens a PID namespace for its children, starts the init, waits for
//!   its boot, runs the command, and ends the init;
//! * the init is PID 1 of the new PID namespace, in a mount, a cgroup, an
//!   IPC and a UTS namespace of its own, with the mounts of
//!   [`super::mounts`];
//! * the state queries and the command join those namespaces
//!   ([`super::join`]).
//!
//! When the supervisor ends, the kernel sends SIGKILL to the init
//! (`PR_SET_PDEATHSIG`), and the end of PID 1 ends each process of its PID
//! namespace: nothing of the run stays.

use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{cgroup, join, mounts};

const CLONE_NEWNS: i32 = 0x0002_0000;
const CLONE_NEWCGROUP: i32 = 0x0200_0000;
const CLONE_NEWUTS: i32 = 0x0400_0000;
const CLONE_NEWIPC: i32 = 0x0800_0000;
const CLONE_NEWPID: i32 = 0x2000_0000;
const PR_SET_PDEATHSIG: i32 = 1;
const PR_CAPBSET_DROP: i32 = 24;
/// The capabilities that the root of a user namespace holds in name only:
/// CAP_NET_ADMIN (12) over the network namespace of the host, CAP_SYS_MODULE
/// (16), CAP_SYS_RAWIO (17) and CAP_SYS_TIME (25). Out of the bounding set
/// of the init, each unit with `ConditionCapability=` of one of them is
/// skipped (the debugfs and tracefs mounts, timesyncd, networkd) and does
/// not fail, as in a container of docker or systemd-nspawn.
const NO_CAPS_IN_USERNS: [u64; 4] = [12, 16, 17, 25];
const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;
/// systemd reads SIGRTMIN+4 as "power off" (glibc's SIGRTMIN is 34).
const SIGRTMIN_4: i32 = 38;
const WNOHANG: i32 = 1;
const O_CLOEXEC: i32 = 0o2000000;
const TIOCGPTN: u64 = 0x8004_5430;
const TIOCSPTLCK: u64 = 0x4004_5431;
/// The exit code of each failure of podvm before the command (docker's).
const REFUSED: i32 = 125;
/// The bound of the stop of the init after the command, as `docker stop`.
const STOP_BOUND: Duration = Duration::from_secs(10);
/// The bytes of the console that an error can name.
const TAIL: usize = 4096;

unsafe extern "C" {
    fn unshare(flags: i32) -> i32;
    fn fork() -> i32;
    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    fn kill(pid: i32, sig: i32) -> i32;
    fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
    fn pipe2(fds: *mut i32, flags: i32) -> i32;
    fn ioctl(fd: i32, req: u64, ...) -> i32;
    fn dup2(old: i32, new: i32) -> i32;
    fn setsid() -> i32;
    fn _exit(code: i32) -> !;
}

/// What `podvm guest-init` takes.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Spec {
    /// The tree of the run; `/` in a guest.
    pub root: PathBuf,
    /// The base of the cgroups of the run, or none.
    pub cgroup: Option<PathBuf>,
    /// The working directory of the command, a path of the image.
    pub cwd: String,
    /// The ids of the command, where they are not 0.
    pub ids: Option<(u32, u32)>,
    /// The supplementary groups of the command, with `ids`.
    pub groups: Vec<u32>,
    /// A tmpfs on `/tmp` with these binds.
    pub tmp: Option<Vec<mounts::Bind>>,
    /// The console of the init goes to stderr as it comes.
    pub verbose: bool,
    /// Nothing of the run stays after it (the tree goes, and each volume is
    /// read-only): the init gets SIGKILL at the end, with no wait for the
    /// stop of its units (TODO/enter.md T-2010).
    pub discard: bool,
    /// Boot the init, write its state to this file, and stay with it: the
    /// command comes later, by `attach` (TODO/machine.md T-2008).
    pub hold: Option<PathBuf>,
    /// Run the command in the init of this state file, which `hold` made.
    pub attach: Option<PathBuf>,
    /// Write the state of the init to this file after its boot, and go on
    /// with the command: a detached run, whose `exec` reads it (T-2002).
    pub state: Option<PathBuf>,
    /// The command gets a pty of the devpts of the init (`-t`).
    pub tty: bool,
    pub argv: Vec<String>,
}

/// The exit status of a wait as a code: the code, or 128 and the signal.
pub(super) fn code_of(st: i32) -> i32 {
    if st & 0x7f == 0 {
        (st >> 8) & 0xff
    } else {
        128 + (st & 0x7f)
    }
}

/// The console pty; where this process has no devpts (a guest), after the
/// mount of one.
fn console() -> Option<(std::fs::File, PathBuf)> {
    pty().or_else(|| mounts::devpts_here().ok().and_then(|()| pty()))
}

/// The master of a pty and the path of its slave.
fn pty() -> Option<(std::fs::File, PathBuf)> {
    let master = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/ptmx")
        .ok()?;
    let (mut n, unlock) = (0i32, 0i32);
    // SAFETY: each ioctl takes a pointer to one int that lives for the call.
    let ok = unsafe {
        ioctl(master.as_raw_fd(), TIOCSPTLCK, &unlock) == 0
            && ioctl(master.as_raw_fd(), TIOCGPTN, &mut n) == 0
    };
    ok.then(|| (master, PathBuf::from(format!("/dev/pts/{n}"))))
}

/// True for the `uid_map` of a user namespace that is not the initial one
/// (the initial one maps each id to itself).
pub fn in_user_namespace(uid_map: &str) -> bool {
    let f: Vec<&str> = uid_map.split_whitespace().collect();
    f != ["0", "0", "4294967295"]
}

/// The init, from the fork to its `execve`. It returns only on an error,
/// with the line for the supervisor.
fn become_init(spec: &Spec, leaves: Option<&cgroup::Leaves>, slave: Option<&Path>) -> String {
    // SAFETY: prctl with two integers; the init ends with the supervisor.
    unsafe { prctl(PR_SET_PDEATHSIG, SIGKILL as u64, 0, 0, 0) };
    if let Some(l) = leaves
        && let Err(e) = cgroup::enter(&l.init)
    {
        return format!("the move into the cgroup {}: {e}", l.init.display());
    }
    let mut flags = CLONE_NEWNS | CLONE_NEWIPC | CLONE_NEWUTS;
    if leaves.is_some() {
        flags |= CLONE_NEWCGROUP;
    }
    // SAFETY: one thread (a new child of a fork).
    if unsafe { unshare(flags) } != 0 {
        return format!(
            "the namespaces of the init: {}",
            std::io::Error::last_os_error()
        );
    }
    if let Err(e) = mounts::build(&mounts::Want {
        root: &spec.root,
        console: slave,
        cgroup: leaves.is_some(),
        tmp: spec.tmp.as_deref(),
    }) {
        return e;
    }
    if in_user_namespace(&std::fs::read_to_string("/proc/self/uid_map").unwrap_or_default()) {
        for cap in NO_CAPS_IN_USERNS {
            // SAFETY: prctl with integers; a refusal leaves the set as it is.
            unsafe { prctl(PR_CAPBSET_DROP, cap, 0, 0, 0) };
        }
    }
    join::drop_groups();
    // A session of its own, with the console on 0, 1 and 2.
    // SAFETY: setsid takes nothing; dup2 takes two open descriptors.
    unsafe { setsid() };
    if let Ok(con) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/console")
    {
        for fd in 0..3 {
            // SAFETY: as above.
            unsafe { dup2(con.as_raw_fd(), fd) };
        }
    }
    use std::os::unix::process::CommandExt;
    let e = std::process::Command::new(super::INIT)
        .env_clear()
        .env("container", "podvm")
        .env("PATH", join::PATH)
        .exec();
    if e.kind() == std::io::ErrorKind::NotFound {
        return format!("the image has no {}: it carries no init", super::INIT);
    }
    format!("exec {}: {e}", super::INIT)
}

/// The console of the init: its last bytes for an error, and each byte on
/// stderr under `--verbose`.
fn drain(mut master: std::fs::File, verbose: bool) -> Arc<Mutex<Vec<u8>>> {
    let tail = Arc::new(Mutex::new(Vec::new()));
    let keep = tail.clone();
    std::thread::spawn(move || {
        // Each signal that the waiter of the command forwards stays with
        // the waiter: this thread would take it with the default action,
        // and the end of the supervisor is a SIGKILL of the init.
        let mut old = 0u64;
        let _ = podvm_enter::block_forwarded(&mut old);
        let mut buf = [0u8; 4096];
        while let Ok(n) = master.read(&mut buf) {
            if n == 0 {
                break;
            }
            if verbose {
                eprint!("{}", String::from_utf8_lossy(&buf[..n]));
            }
            if let Ok(mut t) = keep.lock() {
                t.extend_from_slice(&buf[..n]);
                let over = t.len().saturating_sub(TAIL);
                t.drain(..over);
            }
        }
    });
    tail
}

fn tail_text(tail: &Arc<Mutex<Vec<u8>>>) -> String {
    let bytes = tail.lock().map(|t| t.clone()).unwrap_or_default();
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let from = lines.len().saturating_sub(6);
    lines[from..].join("\n")
}

/// True where the child `pid` ended; its status then.
pub(super) fn ended(pid: i32) -> Option<i32> {
    let mut st = 0;
    // SAFETY: st lives for the call.
    (unsafe { waitpid(pid, &mut st, WNOHANG) } == pid).then_some(st)
}

/// End the init: the signal of a power off, the bound, then SIGKILL; with
/// `discard`, SIGKILL at once.
fn stop(pid: i32, systemd: bool, discard: bool) {
    // SAFETY: kill takes two integers.
    unsafe { kill(pid, stop_signal(systemd)) };
    let end = Instant::now() + if discard { Duration::ZERO } else { STOP_BOUND };
    while Instant::now() < end {
        if ended(pid).is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut st = 0;
    // SAFETY: as above; the wait reaps the init.
    unsafe {
        kill(pid, SIGKILL);
        waitpid(pid, &mut st, 0);
    }
}

/// An init of the image after its boot.
pub(super) struct Booted {
    /// The pid of the init, in the PID namespace of the supervisor.
    pub init: i32,
    pub ns: join::Namespaces,
    /// True where the init is systemd with a state to ask for.
    pub systemd: bool,
    /// True where the init has a cgroup namespace of its own.
    pub cgroup: bool,
    /// One descriptor of the slave of the console, open until the end.
    _held: Option<std::fs::File>,
}

/// One line for a failure before the command, and its exit code.
pub(super) fn refuse(why: &str) -> i32 {
    eprintln!("podvm: --podvm-init=image: {why} (TODO/enter.md T-2002)");
    REFUSED
}

/// Start the init of the image and wait for its boot. An error is the
/// line of the failure; no init stays then.
pub(super) fn start(spec: &Spec) -> Result<Booted, String> {
    let leaves = match spec.cgroup.as_deref().map(cgroup::make) {
        None => None,
        Some(Ok(l)) => Some(l),
        Some(Err(e)) => return Err(format!("the cgroups of the run: {e}")),
    };
    if let Some(l) = &leaves
        && let Err(e) = cgroup::enter(&l.supervisor)
    {
        return Err(format!(
            "the move into the cgroup {}: {e}",
            l.supervisor.display()
        ));
    }
    // The reader of the console starts here: the kernel gives no new
    // thread to a process after its `unshare(CLONE_NEWPID)`.
    let (tail, slave) = match console() {
        Some((master, slave)) => (drain(master, spec.verbose), Some(slave)),
        None => (Arc::new(Mutex::new(Vec::new())), None),
    };
    // One descriptor of the slave stays open here. systemd closes its own,
    // and a pty with no open slave is gone: each open of /dev/console then
    // gave EIO, and PID 1 waited 1 s at each line of its status.
    let held = slave.as_ref().and_then(|s| {
        use std::os::unix::fs::OpenOptionsExt;
        const O_NOCTTY: i32 = 0o400;
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(O_NOCTTY)
            .open(s)
            .ok()
    });
    let mut ends = [-1i32; 2];
    // SAFETY: ends has room for two descriptors.
    if unsafe { pipe2(ends.as_mut_ptr(), O_CLOEXEC) } != 0 {
        return Err(format!("pipe2: {}", std::io::Error::last_os_error()));
    }
    // SAFETY: two descriptors that pipe2 made, each owned once.
    let (ready_r, ready_w) =
        unsafe { (OwnedFd::from_raw_fd(ends[0]), OwnedFd::from_raw_fd(ends[1])) };
    // SAFETY: the PID namespace of each later child of this thread.
    if unsafe { unshare(CLONE_NEWPID) } != 0 {
        return Err(format!(
            "a PID namespace: {}",
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: the child has this thread only; it becomes the init or ends
    // with `_exit`, and it writes to no stream that the reader can hold.
    let init = unsafe { fork() };
    if init < 0 {
        return Err(format!("fork: {}", std::io::Error::last_os_error()));
    }
    if init == 0 {
        drop(ready_r);
        let why = become_init(spec, leaves.as_ref(), slave.as_deref());
        use std::io::Write;
        if let Err(e) = std::fs::File::from(ready_w).write_all(why.as_bytes()) {
            eprintln!("podvm: --podvm-init=image: {why} (the pipe to the supervisor: {e})");
        }
        // SAFETY: the end of the child.
        unsafe { _exit(REFUSED) };
    }
    drop(ready_w);
    let mut why = String::new();
    let _ = std::fs::File::from(ready_r).read_to_string(&mut why);
    if !why.is_empty() {
        let mut st = 0;
        // SAFETY: st lives for the call.
        unsafe { waitpid(init, &mut st, 0) };
        return Err(why);
    }
    let ns = match join::Namespaces::of(init, leaves.is_some()) {
        Ok(ns) => ns,
        Err(e) => {
            stop(init, false, true);
            return Err(format!("the namespaces of the init: {e}"));
        }
    };
    let booted = join::wait_boot(&ns, &mut || ended(init).map(code_of));
    match booted {
        Ok(systemd) => Ok(Booted {
            init,
            ns,
            systemd,
            cgroup: leaves.is_some(),
            _held: held,
        }),
        Err(e) => {
            stop(init, false, true);
            let console = tail_text(&tail);
            Err(if console.is_empty() {
                e
            } else {
                format!("{e}; the last lines of its console:\n{console}")
            })
        }
    }
}

/// Run the whole of `guest-init`: the exit code of the command, or 125
/// with one line that says what happened.
pub fn supervise(spec: &Spec) -> i32 {
    use super::hold::{write, Held};
    let started = start(spec);
    // The state of a detached run, for the process that waits for the boot
    // and for each later `exec`. With no state file, no `exec` can join:
    // the run ends here with the line.
    // The pid of the state file is the one that the reader has for the
    // init: the supervisor of a detached run is in a PID namespace.
    let held = match &started {
        Ok(b) => match super::detach::host_view(b.init) {
            Ok(pid) => Held::Up {
                pid,
                systemd: b.systemd,
                cgroup: b.cgroup,
            },
            Err(why) => Held::Failed(why),
        },
        Err(why) => Held::Failed(why.clone()),
    };
    let written = spec.state.as_deref().map(|f| write(f, &held));
    let b = match started {
        Ok(b) => b,
        Err(why) => return refuse(&why),
    };
    if let (Held::Failed(why), true) = (&held, spec.state.is_some()) {
        stop(b.init, false, true);
        return refuse(why);
    }
    if let (Some(Err(e)), Some(f)) = (written, &spec.state) {
        stop(b.init, false, true);
        return refuse(&format!("the state file {}: {e}", f.display()));
    }
    let code = join::run_command(&b.ns, spec, b.init, None);
    stop(b.init, b.systemd, spec.discard);
    code
}

/// Wait until the init `pid` ends, with no bound: the supervisor of a held
/// init (TODO/machine.md T-2008).
pub(super) fn wait_end(pid: i32) {
    let mut st = 0;
    // SAFETY: st lives for each call.
    while unsafe { waitpid(pid, &mut st, 0) } != pid {
        if std::io::Error::last_os_error().raw_os_error() != Some(4) {
            break;
        }
    }
}

/// A new process of this program, as `fork` gives it: 0 in the child.
pub(super) fn fork_here() -> i32 {
    // SAFETY: the caller has one thread.
    unsafe { fork() }
}

/// End this process at once, with no handler of the runtime.
pub(super) fn exit_now(code: i32) -> ! {
    // SAFETY: the end of the process.
    unsafe { _exit(code) }
}

/// Send `sig` to `pid`; false where no such process is there.
pub(super) fn signal(pid: i32, sig: i32) -> bool {
    // SAFETY: kill takes two integers.
    unsafe { kill(pid, sig) == 0 }
}

/// The signal that asks the init for its stop: the power off of systemd,
/// else SIGTERM.
pub(super) fn stop_signal(systemd: bool) -> i32 {
    if systemd {
        SIGRTMIN_4
    } else {
        SIGTERM
    }
}

pub(super) const KILL: i32 = SIGKILL;
pub(super) const BOUND_OF_STOP: Duration = STOP_BOUND;
