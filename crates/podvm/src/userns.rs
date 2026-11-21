//! PodUtils/podvm#397 (TODO/probe.md T-1909): the namespace rung for an
//! unprivileged caller. pod-enter calls `unshare(CLONE_NEWNS)` in the user
//! namespace of the caller, which needs `CAP_SYS_ADMIN` there, so a caller
//! with no capability entered the userland rung also where the host opens
//! user namespaces. Here podvm opens a user, a PID and a UTS namespace at the
//! start of a foreground `run`, while it has one thread, and maps the caller
//! to 0 in it (as `unshare -Ur`). The probe then sees the rights of the user
//! namespace, the ladder selects the namespace rung, `/proc` mounts in the PID
//! namespace, and `--hostname` names the UTS namespace.
//!
//! Three processes: this one waits outside and forwards the shutdown
//! signals; pid 1 of the PID namespace reaps and forwards; pid 2 runs the
//! rest of podvm. When pid 2 ends, pid 1 exits with its code, and the kernel
//! kills each process that stayed in the namespace.
//!
//! ⛔ Only a foreground `run`: `run -d`, `create` and `exec` keep pids in the
//! record, and a pid of the namespace would name another process outside.
//!
//! It is the default where the trial of the whole entry works (call C21 of
//! `docs/decisions.md`); `PODVM_USERNS=0` leaves the entry as before.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

/// The variable that turns the entry off (`0`).
pub const VAR: &str = "PODVM_USERNS";

const CLONE_NEWNS: i32 = 0x0002_0000;
const CLONE_NEWUTS: i32 = 0x0400_0000;
const MS_REC: u64 = 0x4000;
const MS_PRIVATE: u64 = 0x4_0000;
const CLONE_NEWUSER: i32 = 0x1000_0000;
const CLONE_NEWPID: i32 = 0x2000_0000;
/// `O_DIRECTORY`: on aarch64 `0o200000` is `O_DIRECT` (podcrates' COORD-149).
#[cfg(target_arch = "aarch64")]
pub(crate) const O_DIRECTORY: i32 = 0o40000;
#[cfg(not(target_arch = "aarch64"))]
pub(crate) const O_DIRECTORY: i32 = 0o200000;

const SIGHUP: i32 = 1;
const SIGINT: i32 = 2;
const SIGQUIT: i32 = 3;
const SIGUSR1: i32 = 10;
const SIGUSR2: i32 = 12;
const SIGTERM: i32 = 15;
const SIG_IGN: usize = 1;
const EINTR: i32 = 4;

unsafe extern "C" {
    fn unshare(flags: i32) -> i32;
    fn fork() -> i32;
    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    fn kill(pid: i32, sig: i32) -> i32;
    fn signal(sig: i32, handler: usize) -> usize;
    fn getuid() -> u32;
    fn getgid() -> u32;
    fn sethostname(name: *const u8, len: usize) -> i32;
    fn mount(
        src: *const std::ffi::c_char,
        target: *const std::ffi::c_char,
        fstype: *const std::ffi::c_char,
        flags: u64,
        data: *const u8,
    ) -> i32;
    fn __errno_location() -> *mut i32;
    fn alarm(seconds: u32) -> u32;
    fn atexit(f: extern "C" fn()) -> i32;
    fn _exit(code: i32) -> !;
    fn syscall(n: i64, ...) -> i64;
    fn close(fd: i32) -> i32;
    fn open(path: *const std::ffi::c_char, flags: i32, ...) -> i32;
    fn setns(fd: i32, nstype: i32) -> i32;
    fn pipe2(fds: *mut i32, flags: i32) -> i32;
    fn read(fd: i32, buf: *mut std::ffi::c_void, n: usize) -> isize;
    fn write(fd: i32, buf: *const std::ffi::c_void, n: usize) -> isize;
}

mod host;
mod join;
pub mod owners;
mod range;
pub mod sigrelay;
mod sweep; // PodUtils/podvm#535
mod title; // TODO/enter.md T-2054
pub use host::{bind_devices, bind_onto, bind_volumes, check_volume_under_base, host_pid};
pub use join::{joined_pidfd, rejoin};
pub use range::{again_in_range, available as range_available, cp_in_range, enter_alone};
pub use sweep::sweep;
use title::HIDDEN;
pub use title::{shown_line, TITLE_INIT, TITLE_RUN};

static CHILD: AtomicI32 = AtomicI32::new(0);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static UTS: AtomicBool = AtomicBool::new(false);
/// True where this process owns a UTS namespace of its own: the entry of
/// [`enter_if`], or a `run --hostname` of uid 0.
pub fn owns_uts() -> bool {
    UTS.load(Ordering::Relaxed)
}

/// True where this process joined the namespaces of a run (T-1910).
pub fn joined() -> bool {
    join::JOINED.load(Ordering::Relaxed)
}

/// True in pid 2, where this process owns the UTS namespace.
pub fn active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// True for uid 0 of the initial user namespace: pod-enter gives such a
/// caller no ids inside a namespace (TODO/enter.md T-1927, podcrates@T-787).
pub fn host_root() -> bool {
    // SAFETY: getuid cannot fail.
    !active() && unsafe { getuid() } == 0
}

/// Name the UTS namespace of the run (`--hostname`, T-1908).
pub fn set_hostname(name: &str) -> std::io::Result<()> {
    // SAFETY: the pointer and the length name the bytes of `name`.
    if unsafe { sethostname(name.as_ptr(), name.len()) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

extern "C" fn forward(sig: i32) {
    let c = CHILD.load(Ordering::Relaxed);
    if c > 0 {
        // SAFETY: kill is async-signal-safe.
        unsafe { kill(c, sig) };
    }
}

/// True for an argument vector of a foreground `run` of a caller that is
/// not uid 0, where `PODVM_USERNS` is not `0`.
pub fn wanted(argv: &[String], euid: u32, var: Option<&str>) -> bool {
    // `probe` and `system info` take the same entry, so that the rung that
    // they report is the rung that `run` enters (T-1713).
    let verb = argv.get(1).map(String::as_str);
    let reports = verb == Some("probe")
        || (verb == Some("system") && argv.get(2).map(String::as_str) == Some("info"));
    if var == Some("0") || euid == 0 || (verb != Some("run") && !reports) {
        return false;
    }
    if reports {
        return true;
    }
    // ⚠ A word of the payload that looks like `-d` also keeps the entry as
    // before: the cost is a weaker rung, never a wrong pid.
    !argv[2..].iter().any(|a| {
        a == "--detach"
            || a.starts_with("--detach=")
            || a.starts_with("--podvm-tier")
            || (a.len() > 1
                && a.starts_with('-')
                && !a.starts_with("--")
                && a[1..].bytes().all(|c| c.is_ascii_alphabetic())
                && a.contains('d'))
    })
}

/// The lines of the three map files for `uid` and `gid`.
pub fn maps(uid: u32, gid: u32) -> [(&'static str, String); 3] {
    [
        ("/proc/self/setgroups", "deny".to_string()),
        ("/proc/self/uid_map", format!("0 {uid} 1")),
        ("/proc/self/gid_map", format!("0 {gid} 1")),
    ]
}

/// Wait for `pid`; its exit code, or 128 and the signal.
fn wait_for(pid: i32) -> i32 {
    let mut st = 0;
    loop {
        // SAFETY: st lives for the call.
        let r = unsafe { waitpid(pid, &mut st, 0) };
        // SAFETY: errno of this thread.
        if r < 0 && unsafe { *__errno_location() } == EINTR {
            continue;
        }
        if r < 0 {
            return 125;
        }
        return if st & 0x7f == 0 {
            (st >> 8) & 0xff
        } else {
            128 + (st & 0x7f)
        };
    }
}

/// In pid 2: a mount namespace with a `/proc` of the PID namespace. The
/// host's `/proc` names host pids, and `kill` takes pids of the namespace:
/// the sweep of the processes that stayed (T-1708) killed pid 2 itself.
fn own_proc() -> bool {
    let none = c"none".as_ptr();
    let slash = c"/".as_ptr();
    let proc_ = c"proc".as_ptr();
    let target = c"/proc".as_ptr();
    // SAFETY: each pointer is a NUL-ended literal.
    unsafe {
        unshare(CLONE_NEWNS) == 0
            && mount(
                none,
                slash,
                core::ptr::null(),
                MS_REC | MS_PRIVATE,
                core::ptr::null(),
            ) == 0
            && mount(proc_, target, proc_, 0, core::ptr::null()) == 0
    }
}

/// The user, PID and UTS namespaces, and the maps of `uid` and `gid`:
/// with the subordinate range where the caller asks and the host has it
/// (T-1911), else the one line of the caller.
fn open_namespaces(uid: u32, gid: u32) -> bool {
    // TODO/enter.md T-2002: an init of the image takes the range always.
    let var = std::env::var(range::VAR).ok();
    let want = if crate::initns::early() {
        Some("1")
    } else {
        var.as_deref()
    };
    if let Some(p) = range::plan(want) {
        return with_range(&p, uid, gid, CLONE_NEWUSER | CLONE_NEWPID | CLONE_NEWUTS);
    }
    // SAFETY: one thread (the contract of the callers).
    let opened = unsafe { unshare(CLONE_NEWUSER | CLONE_NEWPID | CLONE_NEWUTS) } == 0;
    opened && one_line(uid, gid)
}

fn one_line(uid: u32, gid: u32) -> bool {
    maps(uid, gid)
        .iter()
        .all(|(file, line)| std::fs::write(file, line).is_ok())
}

/// The namespaces with the range: a helper forked before the `unshare`
/// waits on a pipe, then runs `newuidmap` and `newgidmap` on this process
/// from outside. Where the tools fail, the one line of the caller.
fn with_range(p: &range::Plan, uid: u32, gid: u32, flags: i32) -> bool {
    let me = std::process::id() as i32;
    let mut ends = [-1i32; 2];
    // SAFETY: ends has room for two descriptors.
    if unsafe { pipe2(ends.as_mut_ptr(), 0o2000000) } != 0 {
        return false;
    }
    let (r, w) = (ends[0], ends[1]);
    // SAFETY: one thread; the child reads, runs the two tools, and exits.
    let helper = unsafe { fork() };
    if helper < 0 {
        return false;
    }
    if helper == 0 {
        let mut b = 0u8;
        // SAFETY: w is the parent's end; b lives for the read.
        let go = unsafe {
            close(w);
            read(r, (&raw mut b).cast(), 1) == 1
        };
        let ok = go && range::write_maps(p, me, uid, gid);
        // SAFETY: the end of the helper.
        unsafe { _exit(if ok { 0 } else { 1 }) };
    }
    // SAFETY: r is the helper's end.
    unsafe { close(r) };
    // SAFETY: one thread (the contract of the callers).
    let opened = unsafe { unshare(flags) } == 0;
    if opened {
        // SAFETY: w is open; one byte from a local.
        unsafe { write(w, [1u8].as_ptr().cast(), 1) };
    }
    // SAFETY: w is open; the helper sees EOF where nothing was written.
    unsafe { close(w) };
    let mapped = wait_for(helper) == 0;
    if opened && mapped {
        owners::TOP.store(p.top(), std::sync::atomic::Ordering::Relaxed);
    }
    opened && (mapped || one_line(uid, gid))
}

/// The whole entry, tried in a child that is thrown away: true where each
/// step works. A host can open a user namespace and still refuse the
/// mounts in it (AppArmor's `apparmor_restrict_unprivileged_userns` of
/// Ubuntu 24.04), and a process cannot leave a user namespace again.
fn trial(uid: u32, gid: u32) -> bool {
    // SAFETY: one thread; the child only makes system calls and `_exit`s.
    let pid = unsafe { fork() };
    if pid < 0 {
        return false;
    }
    if pid == 0 {
        let code = if open_namespaces(uid, gid) {
            // SAFETY: as above.
            let inner = unsafe { fork() };
            if inner == 0 {
                // SAFETY: the end of the thrown-away child.
                unsafe { _exit(if own_proc() { 0 } else { 1 }) };
            }
            if inner < 0 {
                1
            } else {
                wait_for(inner)
            }
        } else {
            1
        };
        // SAFETY: the end of the thrown-away child.
        unsafe { _exit(code) };
    }
    wait_for(pid) == 0
}

/// Enter the namespaces where [`wanted`] says so. Returns in pid 2, or
/// where the kernel refuses the user namespace (the ladder decides as
/// before); the other two processes exit with the code of pid 2.
///
/// ⛔ Call it first in `main`, with one thread.
pub fn enter_if(argv: &[String]) {
    crate::lab::host::dispatch(); // TODO/net.md T-1916: the lab process of pod-lab
    if argv.get(1).map(String::as_str) == Some(sweep::REAPER_ARG) {
        sweep::reap_all();
    }
    range::serve_rm(argv);
    // T-1911: a tree that a run with the range left is removed with it.
    let _ = podvm_extract::live::RANGE_REMOVER.set(range::remover);
    join::join_if();
    // SAFETY: getuid and getgid cannot fail.
    let (uid, gid) = unsafe { (getuid(), getgid()) };
    // T-1908: uid 0 has the right to a UTS namespace of its own; `run
    // --hostname` takes one, and each other namespace stays pod-enter's.
    if uid == 0
        && argv.get(1).map(String::as_str) == Some("run")
        && argv[2..].iter().any(|a| a == "--hostname" || a.starts_with("--hostname="))
        // SAFETY: one thread.
        && unsafe { unshare(CLONE_NEWUTS) } == 0
    {
        UTS.store(true, Ordering::Relaxed);
    }
    // TODO/enter.md T-2002: an init of the image that this rung cannot hold.
    if crate::initns::decide(argv, uid) || !wanted(argv, uid, std::env::var(VAR).ok().as_deref()) {
        return;
    }
    if !trial(uid, gid) {
        return;
    }
    if !open_namespaces(uid, gid) {
        // The trial passed, so this is a change of the host in between.
        eprintln!("podvm run: the user namespace closed after its trial (TODO/probe.md T-1909)");
        std::process::exit(125);
    }
    // The code of pid 2 comes back on a pipe as soon as pid 2 ends, so that
    // `run` returns then; pid 1 stays until each process of the namespace
    // ended. The detached helpers of podvm (the removal of a `--rm` tree
    // and the fill of the spare tree, T-1781) outlive pid 2, and the end of
    // pid 1 would kill them: each run then left a tree behind.
    let mut ends = [-1i32; 2];
    const O_CLOEXEC: i32 = 0o2000000;
    // SAFETY: ends has room for two descriptors.
    if unsafe { pipe2(ends.as_mut_ptr(), O_CLOEXEC) } != 0 {
        eprintln!(
            "podvm run: pipe2: {} (TODO/probe.md T-1909)",
            std::io::Error::last_os_error()
        );
        std::process::exit(125);
    }
    let (code_r, code_w) = (ends[0], ends[1]);
    // SAFETY: one thread; the child returns into main.
    let pid1 = unsafe { fork() };
    if pid1 < 0 {
        eprintln!(
            "podvm run: fork: {} (TODO/probe.md T-1909)",
            std::io::Error::last_os_error()
        );
        std::process::exit(125);
    }
    if pid1 > 0 {
        // SAFETY: the write end of this process.
        unsafe { close(code_w) };
        CHILD.store(pid1, Ordering::Relaxed);
        sigrelay::relay_signals();
        let mut b = [0u8; 1];
        loop {
            // SAFETY: b has room for one byte.
            let n = unsafe { read(code_r, b.as_mut_ptr().cast(), 1) };
            // SAFETY: errno of this thread.
            if n < 0 && unsafe { *__errno_location() } == EINTR {
                continue;
            }
            if n == 1 {
                std::process::exit(i32::from(b[0]));
            }
            // No code: pid 1 ended first.
            std::process::exit(wait_for(pid1));
        }
    }
    // pid 1 of the PID namespace.
    sigrelay::die_with_outer(true); // T-2006
                                    // SAFETY: the read end of the outer process.
    unsafe { close(code_r) };
    // SAFETY: one thread.
    let pid2 = unsafe { fork() };
    if pid2 < 0 {
        std::process::exit(125);
    }
    if pid2 == 0 {
        // SAFETY: the write end of pid 1.
        unsafe { close(code_w) };
        // TODO/enter.md T-2054 (OQ-08): the payload reads no argv and no
        // environment of podvm in `/proc/2`.
        if let Err(e) = podvm_enter::title::hide(TITLE_RUN) {
            HIDDEN.store(e.errno.max(1), Ordering::Relaxed);
        }
        const O_PATH_DIR_CLOEXEC: i32 = 0o10000000 | O_DIRECTORY | 0o2000000;
        // SAFETY: a NUL-ended literal.
        let host = unsafe { open(c"/proc".as_ptr(), O_PATH_DIR_CLOEXEC) };
        host::HOST_PROC.store(host, Ordering::Relaxed);
        if !own_proc() {
            eprintln!(
                "podvm run: /proc of the PID namespace: {} (TODO/probe.md T-1909)",
                std::io::Error::last_os_error()
            );
            std::process::exit(125);
        }
        ACTIVE.store(true, Ordering::Relaxed);
        UTS.store(true, Ordering::Relaxed);
        sweep::OWNER.store(std::process::id(), Ordering::Relaxed);
        // SAFETY: a handler with no arguments, for the life of the process.
        unsafe { atexit(sweep::sweep_at_exit) };
        return;
    }
    CHILD.store(pid2, Ordering::Relaxed);
    // T-2054: pid 1 reads no argument and no variable again, so where the
    // title is refused its bytes become NUL in place.
    if podvm_enter::title::hide(TITLE_INIT).is_err() {
        // SAFETY: the reaper below takes its argv and no variable.
        let _ = unsafe { podvm_enter::title::blank_in_place() };
    }
    sigrelay::relay_signals();
    let mut code = 125;
    let mut told = false;
    loop {
        let mut st = 0;
        // SAFETY: st lives for the call.
        let r = unsafe { waitpid(-1, &mut st, 0) };
        // SAFETY: errno of this thread.
        if r < 0 && unsafe { *__errno_location() } == EINTR {
            continue;
        }
        if r < 0 {
            break; // ECHILD: no process of the namespace is left
        }
        if r == pid2 {
            code = if st & 0x7f == 0 {
                (st >> 8) & 0xff
            } else {
                128 + (st & 0x7f)
            };
            let b = [code as u8];
            sigrelay::die_with_outer(false); // T-2006: the reaper stays
                                             // SAFETY: one byte of b; the outer process exits on it.
            unsafe { write(code_w, b.as_ptr().cast(), 1) };
            unsafe { close(code_w) };
            told = true;
            CHILD.store(0, Ordering::Relaxed);
            // After pid 2: only the prefill of T-1781 is left (`sweep`).
            // This process becomes the reaper, with a command line that
            // names no payload; the exec keeps its pid and its children.
            if let Ok(me) = std::env::current_exe() {
                use std::os::unix::process::CommandExt;
                // T-2054: no path and no variable of the host in `/proc/1`.
                let e = std::process::Command::new(me)
                    .arg0(TITLE_INIT)
                    .arg(sweep::REAPER_ARG)
                    .env_clear()
                    .exec();
                eprintln!("podvm: the reaper of the namespace: {e} (TODO/probe.md T-1909)");
            }
        }
    }
    if !told {
        // SAFETY: as above.
        unsafe { close(code_w) };
    }
    std::process::exit(code);
}

#[cfg(test)]
mod tests;
