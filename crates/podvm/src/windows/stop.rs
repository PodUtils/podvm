//! TODO/enter.md T-1953 (PodUtils/podvm#566): a signal to a foreground run
//! of a Windows or DOS guest ends the emulator, then podvm, with the signal.
//!
//! Before, the run had no handler. A SIGTERM ended podvm at once, with the
//! per-run directory (the overlay, about a hundred megabytes) left on the
//! disk and the emulator ended only by `PR_SET_PDEATHSIG` of its spawn,
//! after the fact; a Ctrl-C reached podvm alone, the emulator being in its
//! own process group.
//!
//! Here the set of `machine::guest::signals::SET` is blocked in the thread of
//! the run before the emulator starts (each thread and each child after it
//! inherits the mask: the emulator of `pod-guest` resets none, and a signal
//! sent to it is no end that this module needs, as its end is SIGKILL).
//! A thread of its own reads each signal from a signalfd, and ends each
//! child of podvm with SIGKILL, so that the wait for the emulator returns
//! at once. The run then removes its directory and ends podvm with the
//! signal (143 for SIGTERM), as the machine tier does where its guest
//! takes no signal (`signals::raise`).
//!
//! Not forwarded: a DOS or Windows guest has no command that takes a
//! signal, so the guest ends with its emulator.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use podvm_probe::sys as k;

/// The pause of the watcher between two looks.
const STEP: Duration = Duration::from_millis(100);

/// The signal that came to a run, and the thread that ends its emulator.
pub(crate) struct Stop {
    came: Arc<AtomicI32>,
    done: Arc<AtomicBool>,
}

impl Stop {
    /// Block the set in this thread and watch it from a thread of its own.
    /// Where the kernel refuses the signalfd, a line says so and the run
    /// goes on as before: a signal ends podvm by its default action, and
    /// the emulator by its parent-death signal.
    pub(crate) fn arm(verb: &str) -> Stop {
        let (came, done) = (
            Arc::new(AtomicI32::new(0)),
            Arc::new(AtomicBool::new(false)),
        );
        let mut old = 0u64;
        let sfd = podvm_enter::block_forwarded(&mut old).and_then(|_| k::signalfd_proxied());
        match sfd {
            Ok(fd) => {
                let (c, d, verb) = (came.clone(), done.clone(), verb.to_string());
                std::thread::spawn(move || {
                    let source = || next(fd);
                    watch(
                        source,
                        || kill_children_of(k::getpid(), None),
                        &c,
                        &d,
                        &verb,
                    );
                    let _ = k::close(fd);
                });
            }
            Err(e) => {
                let _ = k::sigrestore(old);
                eprintln!(
                    "podvm {verb}: no signal ends the emulator in order ({}): a SIGTERM ends \
                     podvm, and the emulator with it, but the run directory stays \
                     (TODO/enter.md T-1953)",
                    e.name()
                );
            }
        }
        Stop { came, done }
    }

    /// The signal that came, once the watcher read one.
    pub(crate) fn signal(&self) -> Option<i32> {
        Some(self.came.load(Ordering::SeqCst)).filter(|s| *s > 0)
    }

    /// End the watcher, and podvm with the signal. Call it after the
    /// per-run directory is removed.
    pub(crate) fn end(&self, signo: i32) -> ! {
        self.done.store(true, Ordering::SeqCst);
        crate::machine::guest::signals::raise(signo)
    }
}

impl Drop for Stop {
    fn drop(&mut self) {
        self.done.store(true, Ordering::SeqCst);
    }
}

/// The next signal of the signalfd within [`STEP`], or `None`.
fn next(sfd: i64) -> Option<i32> {
    let mut fds = [k::PollFd {
        fd: sfd as i32,
        events: k::POLLIN,
        revents: 0,
    }];
    let ms = i64::try_from(STEP.as_millis()).unwrap_or(100);
    if !matches!(k::ppoll(&mut fds, ms), Ok(n) if n > 0) {
        return None;
    }
    let mut buf = [0u8; 128];
    match k::read(sfd, &mut buf) {
        Ok(n) if n >= 4 => Some(i32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]])),
        _ => None,
    }
}

/// The loop of the watcher: the first signal is the one that ends podvm;
/// from then on each look ends each child again, so an emulator that the
/// run started a moment after the signal ends too. `source` waits for at
/// most one step.
fn watch(
    mut source: impl FnMut() -> Option<i32>,
    end_children: impl Fn() -> usize,
    came: &AtomicI32,
    done: &AtomicBool,
    verb: &str,
) {
    while !done.load(Ordering::SeqCst) {
        if let Some(signo) = source()
            && came
                .compare_exchange(0, signo, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        {
            let name = crate::machine::guest::signals::name(signo).unwrap_or("a signal");
            eprintln!(
                "podvm {verb}: {name} came: the emulator of the guest ends, and podvm ends \
                 with {name}"
            );
        }
        if came.load(Ordering::SeqCst) > 0 {
            end_children();
        }
    }
}

/// End each child of `parent` with SIGKILL (only `only`, where it is set);
/// the count. A child is the emulator of the guest, or a short helper of
/// the run: both end with it.
fn kill_children_of(parent: i64, only: Option<i64>) -> usize {
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return 0;
    };
    let mut ended = 0;
    for entry in dir.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<i64>().ok())
        else {
            continue;
        };
        if only.is_none_or(|o| o == pid) && parent_of(pid) == Some(parent) {
            // The emulator leads a group of its own (`pod-guest`), so the
            // group goes with it; for a child that leads none, this finds
            // no group.
            let _ = k::kill(-pid, 9);
            if k::kill(pid, 9).is_ok() {
                ended += 1;
            }
        }
    }
    ended
}

/// The parent of `pid` by its `stat`: the field after the command, which
/// is in parentheses and may hold a space or a closing parenthesis.
fn parent_of(pid: i64) -> Option<i64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = &stat[stat.rfind(')')? + 1..];
    after.split_whitespace().nth(1)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The first signal is kept, a later one does not replace it, each look
    /// after it ends the children, and no look before it does.
    #[test]
    fn the_first_signal_ends_the_children_at_each_look() {
        let (came, done) = (AtomicI32::new(0), AtomicBool::new(false));
        let script = Mutex::new(vec![None, Some(15), Some(2), None].into_iter());
        let looks = std::cell::Cell::new(0);
        let ended = std::cell::Cell::new(0);
        let source = || {
            looks.set(looks.get() + 1);
            let next = script.lock().unwrap().next().flatten();
            if looks.get() >= 4 {
                done.store(true, Ordering::SeqCst);
            }
            next
        };
        watch(
            source,
            || {
                ended.set(ended.get() + 1);
                1
            },
            &came,
            &done,
            "windows",
        );
        assert_eq!(
            came.load(Ordering::SeqCst),
            15,
            "SIGTERM first, SIGINT after"
        );
        assert_eq!(
            ended.get(),
            3,
            "the looks of the second, third and fourth step"
        );
    }

    /// The child that is named ends; its sibling, which is a child of the
    /// same parent, stays, and so does the parent. (Each other test of this
    /// binary can have children of its own: the test kills none of them.)
    #[test]
    fn a_child_ends_and_the_parent_stays() {
        let spawn = || {
            std::process::Command::new("sleep")
                .arg("30")
                .spawn()
                .expect("sleep starts")
        };
        let (mut child, mut sibling) = (spawn(), spawn());
        let (pid, me) = (i64::from(child.id()), k::getpid());
        assert_eq!(parent_of(pid), Some(me));
        assert_eq!(kill_children_of(me, Some(pid)), 1);
        let status = child.wait().unwrap();
        assert!(!status.success(), "the child ended by the kill: {status:?}");
        assert!(sibling.try_wait().unwrap().is_none(), "the sibling stays");
        assert_eq!(kill_children_of(me, Some(i64::from(sibling.id()))), 1);
        let _ = sibling.wait();
        assert_eq!(parent_of(me), Some(k::getppid()));
    }

    /// A name with a space and a parenthesis does not move the field.
    #[test]
    fn the_parent_is_read_after_the_last_parenthesis() {
        let stat = "123 (a) b (c) S 77 1 1 0";
        let after = &stat[stat.rfind(')').unwrap() + 1..];
        assert_eq!(after.split_whitespace().nth(1), Some("77"));
    }
}
