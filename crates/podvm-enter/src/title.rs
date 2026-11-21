//! The command line and the environment that `/proc/PID` shows of a process
//! of podvm in the PID namespace of a payload (TODO/enter.md T-2054, OQ-08
//! of the sixth outside pass).
//!
//! ⛔ On the namespace rung a process of podvm stays in the PID namespace
//! of the payload: pid 1 and pid 2 of podvm's user namespace (T-1909), the
//! init of `Plan::pid_namespace` (a fork of the launcher, T-1930) and the
//! relay of a join (T-1910). Each one is a fork of podvm with no exec, so
//! the payload read podvm's whole argv in `ps` (the values of `-e`) and its
//! whole environment in `/proc/1/environ` (the variables of the caller, a
//! token too).
//!
//! ⭐ [`hide`] points the command line of the process at a short title and
//! its environment at nothing, with `prctl(PR_SET_MM, PR_SET_MM_MAP)`, which
//! needs no capability where `exe_fd` is -1 (Linux 3.18 and later, with
//! `CONFIG_CHECKPOINT_RESTORE`). The strings that Rust and the C library
//! read (`std::env::args`, `environ`) do not move: the kernel reads
//! `/proc/PID/cmdline` and `/proc/PID/environ` through `arg_start` to
//! `env_end` only. The name of the process (`/proc/PID/comm`, the base
//! name of podvm's binary) takes the title too. A fork takes the map and the
//! name with it, so a child of a hidden process is hidden until its exec.
//! [`restore`] gives the map back; the name keeps the title, as it names no
//! argument.
//!
//! The title lives in memory of the heap: the kernel reads those two files
//! with `FOLL_ANON`, so a title in `.data` of the binary reads empty
//! (measured on Linux 6.18).

use std::ffi::c_int;

const PR_SET_MM: c_int = 35;
const PR_SET_MM_MAP: u64 = 14;
const PR_SET_NAME: c_int = 15;

/// `struct prctl_mm_map` of `linux/prctl.h`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Map {
    start_code: u64,
    end_code: u64,
    start_data: u64,
    end_data: u64,
    start_brk: u64,
    brk: u64,
    start_stack: u64,
    arg_start: u64,
    arg_end: u64,
    env_start: u64,
    env_end: u64,
    auxv: u64,
    auxv_size: u32,
    exe_fd: u32,
}

/// The map of this process before [`hide`], for [`restore`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Saved(Map);

/// Why the title did not take: the errno of the call that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused {
    pub call: &'static str,
    pub errno: i32,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} failed with errno {}", self.call, self.errno)
    }
}

/// The fields of the map in the text of `/proc/self/stat`: 26 to 28 and 45
/// to 51 of `proc(5)`, counted after the last `)` of the command name.
pub fn map_of(stat: &str, brk: u64) -> Option<Map> {
    map_and_threads(stat, brk).map(|(m, _)| m)
}

/// [`map_of`] and the number of threads, field 20 of `proc(5)`.
fn map_and_threads(stat: &str, brk: u64) -> Option<(Map, u64)> {
    let rest = &stat[stat.rfind(')')? + 1..];
    // Field 3 (the state) is the first after the name; a field that is no
    // number (the state) reads as 0, and a missing one as `None`.
    let mut f = [None::<u64>; 52];
    for (i, w) in rest.split_whitespace().take(49).enumerate() {
        f[i + 3] = Some(w.parse().unwrap_or(0));
    }
    let at = |n: usize| f[n];
    let threads = at(20)?;
    let map = Map {
        start_code: at(26)?,
        end_code: at(27)?,
        start_stack: at(28)?,
        start_data: at(45)?,
        end_data: at(46)?,
        start_brk: at(47)?,
        brk,
        arg_start: at(48)?,
        arg_end: at(49)?,
        env_start: at(50)?,
        env_end: at(51)?,
        auxv: 0,
        auxv_size: 0,
        exe_fd: u32::MAX,
    };
    Some((map, threads))
}

/// `map` with the command line at `title` (its bytes and a NUL, at
/// `at`) and an empty environment just after it.
pub fn titled(map: Map, at: u64, len: u64) -> Map {
    Map {
        arg_start: at,
        arg_end: at + len,
        env_start: at + len,
        env_end: at + len,
        ..map
    }
}

fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

/// The map of this process. ⛔ A refusal where it has more than one
/// thread: another thread can move the break of the heap between its read
/// and the call, and the call would set a stale one.
fn current(alone: bool) -> Result<Map, Refused> {
    unsafe extern "C" {
        fn syscall(n: i64, ...) -> i64;
    }
    #[cfg(target_arch = "x86_64")]
    const SYS_BRK: i64 = 12;
    #[cfg(any(
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "loongarch64"
    ))]
    const SYS_BRK: i64 = 214;
    #[cfg(not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "riscv64",
        target_arch = "loongarch64"
    )))]
    const SYS_BRK: i64 = 45;
    let stat = std::fs::read_to_string("/proc/self/stat").map_err(|e| Refused {
        call: "read(/proc/self/stat)",
        errno: e.raw_os_error().unwrap_or(0),
    })?;
    // SAFETY: brk(0) changes nothing and gives the current break.
    let brk = unsafe { syscall(SYS_BRK, 0i64) } as u64;
    match map_and_threads(&stat, brk) {
        Some((m, n)) if n == 1 || !alone => Ok(m),
        Some(_) => Err(Refused {
            call: "a title with more than one thread",
            errno: 0,
        }),
        None => Err(Refused {
            call: "the parse of /proc/self/stat",
            errno: 0,
        }),
    }
}

unsafe extern "C" {
    fn prctl(option: c_int, ...) -> c_int;
}

fn set(map: &Map) -> Result<(), Refused> {
    let size = std::mem::size_of::<Map>() as u64;
    // SAFETY: the map lives for the call, and `size` is its size.
    if unsafe {
        prctl(
            PR_SET_MM,
            PR_SET_MM_MAP,
            map as *const Map as u64,
            size,
            0u64,
        )
    } == 0
    {
        Ok(())
    } else {
        Err(Refused {
            call: "prctl(PR_SET_MM_MAP)",
            errno: errno(),
        })
    }
}

/// Show `title` as the command line of this process and no environment.
/// ⛔ With one thread: the break of the heap must not move between its
/// read and the call.
pub fn hide(title: &str) -> Result<Saved, Refused> {
    hide_as(title, true)
}

/// [`hide`]; `alone` false takes a process of more threads (a test whose
/// other threads wait).
fn hide_as(title: &str, alone: bool) -> Result<Saved, Refused> {
    let old = current(alone)?;
    let mut bytes = title.as_bytes().to_vec();
    bytes.push(0);
    // The kernel reads the title for the life of the process.
    let bytes: &'static mut [u8] = Box::leak(bytes.into_boxed_slice());
    let new = titled(old, bytes.as_ptr() as u64, bytes.len() as u64);
    set(&new)?;
    // SAFETY: PR_SET_NAME reads a NUL-ended name (the kernel takes 15
    // bytes); a refusal leaves the name of the binary, which is no secret.
    unsafe { prctl(PR_SET_NAME, bytes.as_ptr() as u64) };
    Ok(Saved(old))
}

/// Give back the command line and the environment of [`Saved`] (not the
/// name).
pub fn restore(saved: &Saved) -> Result<(), Refused> {
    restore_as(saved, true)
}

fn restore_as(saved: &Saved, alone: bool) -> Result<(), Refused> {
    let now = current(alone)?;
    set(&Map {
        arg_start: saved.0.arg_start,
        arg_end: saved.0.arg_end,
        env_start: saved.0.env_start,
        env_end: saved.0.env_end,
        ..now
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_map_is_read_from_the_fields_of_proc_stat() {
        // 52 fields; the name holds a space and a `)`.
        let mut fields: Vec<String> = (3..=52).map(|n| n.to_string()).collect();
        fields[0] = "S".into();
        let stat = format!("77 (a b) c) {}", fields.join(" "));
        let m = map_of(&stat, 9).expect("parses");
        assert_eq!(
            (m.start_code, m.end_code, m.start_stack),
            (26, 27, 28),
            "{m:?}"
        );
        assert_eq!(
            (m.start_data, m.end_data, m.start_brk, m.brk),
            (45, 46, 47, 9)
        );
        assert_eq!(
            (m.arg_start, m.arg_end, m.env_start, m.env_end),
            (48, 49, 50, 51)
        );
        assert_eq!((m.auxv, m.auxv_size, m.exe_fd), (0, 0, u32::MAX));
        assert_eq!(map_of("77 (x) S 1 2", 0), None, "too few fields");
        assert_eq!(map_of("no name", 0), None);
        let t = titled(m, 1000, 6);
        assert_eq!(
            (t.arg_start, t.arg_end, t.env_start, t.env_end),
            (1000, 1006, 1006, 1006)
        );
        assert_eq!(t.start_code, m.start_code);
        assert_eq!(std::mem::size_of::<Map>(), 104, "struct prctl_mm_map");
        let threads = map_and_threads(&stat, 9).map(|(_, t)| t);
        assert_eq!(threads, Some(20), "the threads");
    }

    /// The child of [`a_hidden_process_shows_its_title_and_no_environment`]:
    /// a process of its own, as `hide` wants one thread.
    #[test]
    #[ignore = "the child of a_hidden_process_shows_its_title_and_no_environment"]
    fn hidden_child() {
        let before = std::fs::read("/proc/self/cmdline").unwrap();
        // The other thread of the harness waits for this one.
        let saved = match hide_as("podvm-test", false) {
            Ok(saved) => saved,
            Err(e) => {
                println!("T-2054: REFUSED: {e}");
                return;
            }
        };
        let cmd = std::fs::read("/proc/self/cmdline").unwrap();
        let env = std::fs::read("/proc/self/environ").unwrap();
        assert_eq!(cmd, b"podvm-test\0", "the title");
        // The name is of a thread: this test runs on one of the harness.
        let comm = std::fs::read_to_string("/proc/thread-self/comm").unwrap();
        assert_eq!(comm, "podvm-test\n", "the name");
        assert!(env.is_empty(), "no environment");
        assert!(std::env::args_os().count() > 1, "the arguments stay");
        restore_as(&saved, false).unwrap();
        assert_eq!(
            std::fs::read("/proc/self/cmdline").unwrap(),
            before,
            "the restore"
        );
    }

    /// In a process of its own: the title shows in `/proc/self/cmdline`,
    /// the environment reads empty, `std::env::args` still gives the
    /// arguments, and the restore gives the command line back. Where the
    /// kernel refuses the call (no `CONFIG_CHECKPOINT_RESTORE`) the test
    /// says so and passes.
    #[test]
    fn a_hidden_process_shows_its_title_and_no_environment() {
        // The harness has threads: the title is refused, and nothing moves.
        let before = std::fs::read("/proc/self/cmdline").unwrap();
        let e = hide("podvm-test").expect_err("more than one thread");
        assert_eq!(e.call, "a title with more than one thread");
        assert_eq!(std::fs::read("/proc/self/cmdline").unwrap(), before);
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", "title::tests::hidden_child"])
            .args(["--nocapture", "--test-threads=1"])
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        if text.contains("T-2054: REFUSED") {
            eprintln!("T-2054: PR_SET_MM_MAP is refused here; skipped");
            return;
        }
        assert!(text.contains("1 passed"), "{text}");
        assert!(out.status.success(), "{text}");
    }
}
