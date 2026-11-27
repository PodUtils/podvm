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
    let (master, slave) = pty().ok_or("no pty")?;
    let fd = slave.as_raw_fd();
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
            .pre_exec(|| {
                // The terminal of a terminal emulator: a session of its own
                // whose controlling terminal is the pty.
                setsid();
                if ioctl(0, TIOCSCTTY, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
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
        .map(|()| (code, all.clone()))
        .map_err(|e| format!("{e}; output: {all:?}"))
}

fn libc_dup(fd: c_int) -> c_int {
    unsafe extern "C" {
        fn dup(fd: c_int) -> c_int;
    }
    // SAFETY: dup of an open descriptor.
    unsafe { dup(fd) }
}
