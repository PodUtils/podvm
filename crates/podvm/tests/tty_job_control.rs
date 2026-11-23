//! TODO/cli.md T-2071 (OR-B38-01 of the seventh outside pass): a
//! job-control shell through the pty of `exec -it` and `run -it` gets each
//! line that the caller types, in order.
//!
//! Before, the payload of `exec -it` into a `run -d` of a user namespace
//! was a member of podvm's session, and `/dev/tty` of the container was the
//! host's: `sh` (job control on) made its process group the foreground
//! group of the caller's terminal, and the relay of podvm got no input
//! after the first chunk. The test is the caller: it opens a pty, makes it
//! the controlling terminal of podvm's session, as a terminal emulator
//! does, and types into the master.
//!
//! The image is an OCI layout with the host's `dash` as `/bin/sh`, `cat`,
//! `sleep`, the loader and libc. A host with no glibc or no `dash` at the
//! x86_64 paths skips the test with a line on stderr. The fault showed as
//! a user with no root (the join of the user namespace); as root the test
//! checks the same rule on the rung of root.

use std::ffi::{c_int, c_ulong};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

unsafe extern "C" {
    fn ioctl(fd: c_int, req: c_ulong, ...) -> c_int;
    fn setsid() -> c_int;
}

const TIOCGPTN: c_ulong = 0x8004_5430;
const TIOCSPTLCK: c_ulong = 0x4004_5431;
const TIOCSCTTY: c_ulong = 0x540E;
const TIOCSWINSZ: c_ulong = 0x5414;
const IMAGE: &str = "localhost/t2071:1";

/// The keys to type, the text that must follow, and how many times.
type Steps<'a> = &'a [(&'a [u8], &'a str, usize)];

mod host_image;
use host_image::{podvm, text};

/// A pty: the master, and the slave that becomes the controlling terminal
/// of podvm's session.
fn pty() -> Option<(std::fs::File, OwnedFd)> {
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
fn session(store: &Path, args: &[&str], steps: &[(&[u8], &str, usize)]) -> Result<i32, String> {
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
    result
        .map(|()| code)
        .map_err(|e| format!("{e}; output: {:?}", text(&out.lock().unwrap())))
}

fn libc_dup(fd: c_int) -> c_int {
    unsafe extern "C" {
        fn dup(fd: c_int) -> c_int;
    }
    // SAFETY: dup of an open descriptor.
    unsafe { dup(fd) }
}

/// T-2071: one rule for each entry and each job-control shell: three lines,
/// each answered in order, and the exit code of the end.
#[test]
fn a_job_control_shell_on_the_pty_of_exec_it_and_run_it_gets_each_line() {
    let Some(tar) = host_image::layout(IMAGE, &["cat", "sleep"]) else {
        eprintln!("T-2071: this host has no glibc or dash at the x86_64 paths; skipped");
        return;
    };
    if pty().is_none() {
        eprintln!("T-2071: /dev/ptmx does not open here; skipped");
        return;
    }
    let (dir, store) = host_image::store_with("t2071", tar);
    let d = podvm(
        &store,
        &["run", "-d", "--name", "t2071", IMAGE, "sleep", "300"],
    );
    assert!(d.status.success(), "run -d: {}", text(&d.stderr));

    // `echo A$((40+N))` prints A4N: the echo of the pty shows the command,
    // and only the shell prints the sum. `cat` gives each line twice: the
    // echo of the pty and the copy of cat.
    let interactive: &[(&[u8], &str, usize)] = &[
        (b"echo A$((40+1))\n", "A41", 1),
        (b"echo A$((40+2))\n", "A42", 1),
        (b"echo A$((40+3))\n", "A43", 1),
        (b"exit 7\n", "exit 7", 1),
    ];
    let job_cat: &[(&[u8], &str, usize)] = &[
        (b"L1-x\n", "L1-x", 2),
        (b"L2-x\n", "L2-x", 2),
        (b"L3-x\n", "L3-x", 2),
        (b"\x04", "", 0),
    ];
    let cases: &[(&[&str], Steps, i32)] = &[
        (&["exec", "-it", "t2071", "sh", "-i"], interactive, 7),
        (&["exec", "-it", "t2071", "sh"], interactive, 7),
        (
            &["exec", "-it", "t2071", "sh", "-c", "set -m; cat"],
            job_cat,
            0,
        ),
        (&["run", "--rm", "-it", IMAGE, "sh", "-i"], interactive, 7),
        (
            &["run", "--rm", "-it", IMAGE, "sh", "-c", "set -m; cat"],
            job_cat,
            0,
        ),
    ];
    let mut failures = Vec::new();
    for (args, steps, want) in cases {
        match session(&store, args, steps) {
            Ok(code) if code == *want => {}
            Ok(code) => failures.push(format!("{args:?}: exit {code}, want {want}")),
            Err(e) => failures.push(e),
        }
    }
    let _ = podvm(&store, &["rm", "-f", "t2071"]);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
