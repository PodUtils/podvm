//! A terminal for a test of `-t` (TODO/cli.md T-2071, T-2070): the test
//! is the caller. It opens a pty, makes it the controlling terminal of
//! podvm's session, as a terminal emulator does, and types into the master.

#![allow(dead_code)]

use std::ffi::{c_int, c_ulong};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::host_image::text;

unsafe extern "C" {
    fn ioctl(fd: c_int, req: c_ulong, ...) -> c_int;
    fn setsid() -> c_int;
    fn unshare(flags: c_int) -> c_int;
    fn open(path: *const std::ffi::c_char, flags: c_int, ...) -> c_int;
    fn getuid() -> u32;
    fn getgid() -> u32;
}

/// Where podvm runs: as the test runs, or in a user namespace of its own
/// where no further user namespace can be made (`max_user_namespaces` 0),
/// as on a host that refuses them to a user (the runner of GitHub, whose
/// AppArmor refuses them): there podvm takes the userland rung.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prep {
    Host,
    NoUserns,
}

/// The four files that make the user namespace of [`Prep::NoUserns`]: the
/// caller keeps its uid and gid, and the namespace allows no child.
fn no_userns_files() -> Vec<(std::ffi::CString, Vec<u8>)> {
    // SAFETY: getuid and getgid cannot fail.
    let (uid, gid) = unsafe { (getuid(), getgid()) };
    [
        ("/proc/self/setgroups", "deny".to_string()),
        ("/proc/self/uid_map", format!("{uid} {uid} 1")),
        ("/proc/self/gid_map", format!("{gid} {gid} 1")),
        ("/proc/sys/user/max_user_namespaces", "0".to_string()),
    ]
    .into_iter()
    .map(|(p, v)| (std::ffi::CString::new(p).unwrap(), v.into_bytes()))
    .collect()
}

/// In the child, before the exec: system calls only, on buffers made
/// before the fork. The exec of a program by a user that is not uid 0
/// drops each capability of the new namespace.
fn enter_no_userns(files: &[(std::ffi::CString, Vec<u8>)]) -> std::io::Result<()> {
    const CLONE_NEWUSER: c_int = 0x1000_0000;
    const O_WRONLY: c_int = 1;
    // SAFETY: each path is a C string and each buffer lives for the call.
    unsafe {
        if unshare(CLONE_NEWUSER) != 0 {
            return Err(std::io::Error::last_os_error());
        }
        for (path, value) in files {
            let fd = open(path.as_ptr(), O_WRONLY);
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            // One write call; the drop closes the descriptor.
            std::fs::File::from_raw_fd(fd).write_all(value)?;
        }
    }
    Ok(())
}

/// podvm on `store`, as `prep` says; the caller adds the arguments.
pub fn podvm_on(store: &Path, prep: Prep) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_podvm"));
    cmd.env("PODVM_SPARE_TREE", "0").env("PODVM_STORE", store);
    if prep == Prep::NoUserns {
        let files = no_userns_files();
        // SAFETY: the hook makes system calls only.
        unsafe { cmd.pre_exec(move || enter_no_userns(&files)) };
    }
    cmd
}

/// True where this host lets the test make the namespace of
/// [`Prep::NoUserns`]; false where it refuses each user namespace already.
pub fn can_prep_no_userns() -> bool {
    let mut cmd = Command::new("/bin/sh");
    let files = no_userns_files();
    // SAFETY: the hook makes system calls only.
    unsafe { cmd.pre_exec(move || enter_no_userns(&files)) };
    cmd.args(["-c", "true"]).status().is_ok_and(|s| s.success())
}

const TIOCGPTN: c_ulong = 0x8004_5430;
const TIOCSPTLCK: c_ulong = 0x4004_5431;
const TIOCSCTTY: c_ulong = 0x540E;
const TIOCSWINSZ: c_ulong = 0x5414;

/// The keys to type, the text that must follow, and how many times.
pub type Steps<'a> = &'a [(&'a [u8], &'a str, usize)];

/// A pty: the master, and the slave that becomes the controlling terminal
/// of podvm's session.
pub fn pty() -> Option<(std::fs::File, OwnedFd)> {
    let master = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(0o400) // O_NOCTTY
        .open("/dev/ptmx")
        .ok()?;
    let (unlock, mut n): (c_int, u32) = (0, 0);
    let size: [u16; 4] = [30, 100, 0, 0];
    // SAFETY: each request takes a pointer to a value of the kernel's type.
    unsafe {
        if ioctl(master.as_raw_fd(), TIOCSPTLCK, &unlock) != 0
            || ioctl(master.as_raw_fd(), TIOCGPTN, &mut n) != 0
        {
            return None;
        }
        ioctl(master.as_raw_fd(), TIOCSWINSZ, size.as_ptr());
    }
    let slave = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(0o400)
        .open(format!("/dev/pts/{n}"))
        .ok()?;
    Some((master, slave.into()))
}

/// One session of a terminal: podvm with `args` on a pty; `steps` are the
/// bytes to type and the text that the output must hold, in order, before
/// the next step. Returns the exit code, or the failure with the output.
pub fn session(
    store: &Path,
    args: &[&str],
    steps: &[(&[u8], &str, usize)],
) -> Result<(i32, String), String> {
    session_on(store, Prep::Host, args, steps).map(|(code, out, _)| (code, out))
}

/// [`session`] as `prep` says; also gives the session of the caller's
/// terminal (`TIOCGSID` of its master; the spawn returns after the exec,
/// so after the `setsid` of the hook): the session of podvm, which leads
/// it, as the test sees pids.
pub fn session_on(
    store: &Path,
    prep: Prep,
    args: &[&str],
    steps: &[(&[u8], &str, usize)],
) -> Result<(i32, String, i64), String> {
    let (master, slave) = pty().ok_or("no pty")?;
    let fd = slave.as_raw_fd();
    let files = (prep == Prep::NoUserns).then(no_userns_files);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_podvm"));
    cmd.env("PODVM_SPARE_TREE", "0")
        .env("PODVM_STORE", store)
        .env("TERM", "dumb")
        .args(args);
    // SAFETY: the slave is open in the child: three dups of it as 0, 1, 2.
    unsafe {
        cmd.stdin(Stdio::from(OwnedFd::from_raw_fd(libc_dup(fd))))
            .stdout(Stdio::from(OwnedFd::from_raw_fd(libc_dup(fd))))
            .stderr(Stdio::from(OwnedFd::from_raw_fd(libc_dup(fd))))
            .pre_exec(move || {
                // The terminal of a terminal emulator: a session of its own
                // whose controlling terminal is the pty.
                setsid();
                if ioctl(0, TIOCSCTTY, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                match &files {
                    Some(f) => enter_no_userns(f),
                    None => Ok(()),
                }
            });
    }
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    drop(slave);
    let out = Arc::new(Mutex::new(Vec::new()));
    let mut reader = master.try_clone().map_err(|e| e.to_string())?;
    let sink = out.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            sink.lock().unwrap().extend_from_slice(&buf[..n]);
        }
    });
    let mut master = master;
    const TIOCGSID: c_ulong = 0x5429;
    let mut sid: c_int = 0;
    // SAFETY: TIOCGSID of a master writes the session of its slave.
    unsafe { ioctl(master.as_raw_fd(), TIOCGSID, &mut sid) };
    let caller = i64::from(sid);
    let seen = |want: &str, times: usize| text(&out.lock().unwrap()).matches(want).count() >= times;
    let mut result = Ok(());
    std::thread::sleep(Duration::from_millis(500));
    for (keys, want, times) in steps {
        let _ = master.write_all(keys);
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end && !seen(want, *times) {
            std::thread::sleep(Duration::from_millis(50));
        }
        if !seen(want, *times) {
            result = Err(format!(
                "{args:?}: after {:?}, no {want:?} ({times} times)",
                text(keys)
            ));
            break;
        }
    }
    let end = Instant::now() + Duration::from_secs(20);
    let code = loop {
        if let Ok(Some(s)) = child.try_wait() {
            break s.code().unwrap_or(-1);
        }
        if Instant::now() > end || result.is_err() {
            let _ = child.kill();
            let _ = child.wait();
            if result.is_ok() {
                result = Err(format!("{args:?}: podvm did not exit"));
            }
            break -1;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    std::thread::sleep(Duration::from_millis(200));
    let all = text(&out.lock().unwrap());
    result
        .map(|()| (code, all.clone(), caller))
        .map_err(|e| format!("{e}; output: {all:?}"))
}

fn libc_dup(fd: c_int) -> c_int {
    unsafe extern "C" {
        fn dup(fd: c_int) -> c_int;
    }
    // SAFETY: dup of an open descriptor.
    unsafe { dup(fd) }
}
