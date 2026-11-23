//! TODO/machine.md T-2077: PID 1 of an instance and the OOM killer of the
//! instance's cgroup.
//!
//! Measured on the cloud host (cgroup v1, incusd of route A): under
//! `--memory 32m`, `head -c 200000000 /dev/zero | tail` makes two OOM
//! events some 5 ms apart. The first kills `tail`; the second comes before
//! the memory of `tail` is free, and with `tail` gone the largest task is
//! PID 1 (`guest-hold`, some 1100 pages, `oom_score_adj` 0). The killer
//! takes it, the instance stops, and no exit file and no usage file stay
//! (12 of 44 runs).
//!
//! So PID 1 sets its own `oom_score_adj` to -1000 before the command
//! starts, and the command gets back the value of PID 1 at the start: the
//! killer never takes PID 1. A kernel that refuses -1000 (an instance of an
//! incusd with no `CAP_SYS_RESOURCE` of the host: measured, EACCES) keeps
//! PID 1 at its value, and the command gets 1000 instead: each process of
//! the command has a score above each value that PID 1 can have, so the
//! killer takes PID 1 only where no process of the command is left.

use std::path::Path;

/// The value of `oom_score_adj` that the OOM killer never takes.
pub const MIN: i32 = -1000;
/// The highest value of `oom_score_adj`.
pub const MAX: i32 = 1000;
/// The file of this process.
pub const SELF: &str = "/proc/self/oom_score_adj";

/// What PID 1 could set for itself, and what the command gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guard {
    /// PID 1 has [`MIN`]; the command gets `base`, the value of PID 1
    /// before.
    Immune { base: i32 },
    /// The kernel refused [`MIN`]: PID 1 keeps `base`, and the command gets
    /// [`MAX`].
    Last { base: i32 },
}

/// The value of the text of an `oom_score_adj` file, where it is one.
pub fn parse(text: &str) -> Option<i32> {
    text.trim()
        .parse::<i32>()
        .ok()
        .filter(|v| (MIN..=MAX).contains(v))
}

impl Guard {
    /// Set [`MIN`] in `file` (the `oom_score_adj` of this process). An
    /// unreadable value before counts as 0, the value of the kernel. The
    /// reason of a refusal comes with [`Guard::Last`].
    pub fn take(file: &Path) -> (Guard, Option<String>) {
        let base = std::fs::read_to_string(file)
            .ok()
            .and_then(|t| parse(&t))
            .unwrap_or(0);
        // A PID 1 that has MIN already: the command gets the value of the
        // kernel, not MIN.
        if base == MIN {
            return (Guard::Immune { base: 0 }, None);
        }
        match std::fs::write(file, format!("{MIN}\n")) {
            Ok(()) => match std::fs::read_to_string(file).ok().and_then(|t| parse(&t)) {
                Some(MIN) => (Guard::Immune { base }, None),
                other => (
                    Guard::Last { base },
                    Some(format!(
                        "{}: the write of {MIN} gave {}",
                        file.display(),
                        other.map_or("no value".to_string(), |v| v.to_string())
                    )),
                ),
            },
            Err(e) => (
                Guard::Last { base },
                Some(format!("{}: {MIN}: {e}", file.display())),
            ),
        }
    }

    /// The value of the command.
    pub fn child(self) -> i32 {
        match self {
            Guard::Immune { base } => base,
            Guard::Last { .. } => MAX,
        }
    }

    /// True where the command must not start with the value of PID 1: with
    /// [`MIN`], no process of the command would be killable, and a command
    /// over its limit would wait for memory for ever.
    pub fn must_set(self) -> bool {
        matches!(self, Guard::Immune { .. })
    }

    /// The text that the command writes to its file before its `exec`.
    pub fn child_text(self) -> Vec<u8> {
        format!("{}\n", self.child()).into_bytes()
    }
}

/// In the child after the fork, before the `exec`: write `text` to the
/// `oom_score_adj` of the child. Async-signal-safe: no allocation. An
/// error is one only where `must` is true.
///
/// # Safety
///
/// Call it only in the child of a fork (the `pre_exec` of a command).
pub unsafe fn set_in_child(text: &[u8], must: bool) -> std::io::Result<()> {
    unsafe extern "C" {
        fn open(path: *const std::ffi::c_char, flags: i32, ...) -> i32;
        fn write(fd: i32, buf: *const std::ffi::c_void, n: usize) -> isize;
        fn close(fd: i32) -> i32;
    }
    const O_WRONLY: i32 = 1;
    const O_CLOEXEC: i32 = 0o2000000;
    // SAFETY: a C string literal; plain system calls.
    let fd = unsafe { open(c"/proc/self/oom_score_adj".as_ptr(), O_WRONLY | O_CLOEXEC) };
    let ok = fd >= 0
        // SAFETY: a buffer of `text.len()` bytes that lives through the call.
        && unsafe { write(fd, text.as_ptr().cast(), text.len()) } == text.len() as isize;
    let err = std::io::Error::last_os_error();
    if fd >= 0 {
        // SAFETY: the descriptor of the open above.
        unsafe { close(fd) };
    }
    if ok || !must {
        Ok(())
    } else {
        Err(err)
    }
}
