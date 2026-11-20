//! TODO/enter.md T-2002: the terminal of a command in an init of the image
//! (`-t`).
//!
//! The pty that podvm gives a run (`run/pty.rs`, `guest-tty`) belongs to
//! the devpts of the process that opened it. The init of the image has a
//! devpts instance of its own on `/dev/pts`, so that pty has no name in
//! the image: `tty` gave "No such device", and `ttyname` failed. Here the
//! command gets a pty of the devpts of the init:
//!
//! * the master opens through `/proc/INIT/root/dev/pts/ptmx`, in the
//!   process that waits for the command;
//! * the command opens the slave by its name in the image, as the leader
//!   of a session of its own, so the slave is its controlling terminal
//!   ([`take`]);
//! * a relay copies the terminal of this process (0 and 1) to the master
//!   and back, with the window size. The relay is a process: this one took
//!   the PID namespace of the init for its children, and the kernel gives
//!   such a process no thread.
//!
//! The terminal of this process is raw for the time of the command, so
//! that each key goes through once and the pty of the init makes the echo
//! and the signals of the keys.

use std::ffi::CStr;
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

use podvm_probe::sys as k;

use crate::machine::guest::tty::{end_of_input, Raw};

const TIOCGPTN: u64 = 0x8004_5430;
const TIOCSPTLCK: u64 = 0x4004_5431;
const TIOCGWINSZ: u64 = 0x5413;
const TIOCSWINSZ: u64 = 0x5414;
const O_RDWR: i32 = 2;
const SIGKILL: i64 = 9;
const WNOHANG: i32 = 1;
const POLLERR: i16 = 0x0008;
const POLLNVAL: i16 = 0x0020;
/// The time that the relay has for the last bytes after the end of the
/// command. The master gives its end when no process holds the slave; a
/// process of the command that stays in the background holds it for longer,
/// and the run does not wait for it (as docker).
const DRAIN: Duration = Duration::from_millis(500);
/// The step of the look at the window size of the terminal of this process.
const SIZE_STEP_MS: i64 = 250;

unsafe extern "C" {
    fn ioctl(fd: i32, req: u64, ...) -> i32;
    fn open(path: *const std::ffi::c_char, flags: i32, ...) -> i32;
    fn dup2(old: i32, new: i32) -> i32;
    fn close(fd: i32) -> i32;
    fn setsid() -> i32;
    fn fork() -> i32;
    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
}

/// The master of a pty of the devpts of an init, and the name of its slave
/// in the image.
pub struct Inner {
    master: std::fs::File,
    pub slave: String,
}

/// Open a pty of the devpts instance of the init `init` (a pid of this
/// `/proc`), unlocked.
pub fn for_init(init: i32) -> std::io::Result<Inner> {
    let master = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(format!("/proc/{init}/root/dev/pts/ptmx"))?;
    let (mut n, unlock) = (0i32, 0i32);
    // SAFETY: each ioctl takes a pointer to one int that lives for the call.
    let ok = unsafe {
        ioctl(master.as_raw_fd(), TIOCSPTLCK, &unlock) == 0
            && ioctl(master.as_raw_fd(), TIOCGPTN, &mut n) == 0
    };
    if !ok {
        return Err(std::io::Error::last_os_error());
    }
    Ok(Inner {
        master,
        slave: format!("/dev/pts/{n}"),
    })
}

/// In the child, after the join of the mount namespace and before the
/// `execve`: a session of its own, and the slave `path` on 0, 1 and 2. The
/// open of a terminal by a session leader with none makes it the
/// controlling terminal. System calls only.
pub fn take(path: &CStr) -> std::io::Result<()> {
    // SAFETY: a C string that lives for the call; descriptors of this child.
    unsafe {
        setsid();
        let fd = open(path.as_ptr(), O_RDWR);
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        for target in 0..3 {
            if fd != target && dup2(fd, target) < 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        if fd > 2 {
            close(fd);
        }
    }
    Ok(())
}

/// A relay that runs, and the terminal of this process in raw mode. The
/// drop gives the relay the time of [`DRAIN`], ends it, and gives the
/// terminal its mode back.
pub struct Relay {
    pid: i32,
    _raw: Raw,
}

impl Inner {
    /// Start the relay. This process keeps no descriptor of the pty: the
    /// master then gives its end when the last process of the command
    /// closes the slave.
    pub fn relay(self) -> std::io::Result<Relay> {
        let raw = Raw::new();
        let master = self.master.as_raw_fd();
        // SAFETY: the child makes system calls only, on a buffer of its
        // stack, and ends with `exit_group`.
        let pid = unsafe { fork() };
        if pid < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if pid == 0 {
            let _ = k::set_pdeathsig(SIGKILL as u64);
            pump(i64::from(master))
        }
        Ok(Relay { pid, _raw: raw })
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        let end = Instant::now() + DRAIN;
        let mut st = 0;
        loop {
            // SAFETY: st lives for the call.
            if unsafe { waitpid(self.pid, &mut st, WNOHANG) } != 0 {
                return;
            }
            if Instant::now() >= end {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = k::kill(i64::from(self.pid), SIGKILL);
        // SAFETY: as above; the wait reaps the relay.
        unsafe { waitpid(self.pid, &mut st, 0) };
    }
}

/// Write each byte of `buf` to `fd`; false where the other side is gone.
fn write_all(fd: i64, mut buf: &[u8]) -> bool {
    while !buf.is_empty() {
        match k::write(fd, buf) {
            Ok(n) if n > 0 => buf = &buf[n as usize..],
            Err(e) if e == k::EINTR => {}
            _ => return false,
        }
    }
    true
}

/// The relay: 0 to the master, the master to 1, and the window size of 0
/// onto the master. It ends when the master gives its end (no process
/// holds the slave), or when 1 is gone. An end of 0 stops only that
/// direction, and the pty gets the end of input.
fn pump(master: i64) -> ! {
    let mut buf = [0u8; 8192];
    let mut size = [0u16; 4];
    let mut input = true;
    let mut last: Option<u8> = None;
    loop {
        let mut now = [0u16; 4];
        // SAFETY: `struct winsize` is four unsigned shorts.
        if unsafe { ioctl(0, TIOCGWINSZ, now.as_mut_ptr()) } == 0 && now != size {
            size = now;
            // SAFETY: as above.
            unsafe { ioctl(master as i32, TIOCSWINSZ, size.as_ptr()) };
        }
        let mut fds = [
            k::PollFd {
                fd: master as i32,
                events: k::POLLIN,
                revents: 0,
            },
            k::PollFd {
                fd: if input { 0 } else { -1 },
                events: k::POLLIN,
                revents: 0,
            },
        ];
        match k::ppoll(&mut fds, SIZE_STEP_MS) {
            Ok(_) => {}
            Err(e) if e == k::EINTR => continue,
            Err(_) => k::exit_group(1),
        }
        if fds[0].revents & k::POLLIN != 0 {
            match k::read(master, &mut buf) {
                Ok(n) if n > 0 => {
                    if !write_all(1, &buf[..n as usize]) {
                        k::exit_group(0);
                    }
                }
                Err(e) if e == k::EINTR => {}
                // EIO: each descriptor of the slave is closed.
                _ => k::exit_group(0),
            }
        } else if fds[0].revents & (k::POLLHUP | POLLERR | POLLNVAL) != 0 {
            k::exit_group(0);
        }
        let mut ended = false;
        if fds[1].revents & k::POLLIN != 0 {
            match k::read(0, &mut buf) {
                Ok(n) if n > 0 => {
                    last = Some(buf[n as usize - 1]);
                    if !write_all(master, &buf[..n as usize]) {
                        k::exit_group(0);
                    }
                }
                Err(e) if e == k::EINTR => {}
                _ => ended = true,
            }
        } else if fds[1].revents & (k::POLLHUP | POLLERR | POLLNVAL) != 0 {
            ended = true;
        }
        if ended {
            input = false;
            let _ = write_all(master, end_of_input(last));
        }
    }
}

#[cfg(test)]
mod tests {
    /// The end of the input is one VEOF after a whole line or no byte, and
    /// two after a line with no end: the first one gives the line to the
    /// reader, and the second one is its end (TODO/cli.md T-1817).
    #[test]
    fn the_end_of_input_ends_a_read_of_the_terminal() {
        for (last, want) in [
            (None, &[0x04u8][..]),
            (Some(b'\n'), &[0x04][..]),
            (Some(b'x'), &[0x04, 0x04][..]),
            (Some(0u8), &[0x04, 0x04][..]),
        ] {
            assert_eq!(super::end_of_input(last), want, "{last:?}");
        }
    }
}
