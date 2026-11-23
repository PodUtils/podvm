//! TODO/machine.md T-2076 (PodUtils/podrunner#345): the CPU time, the peak
//! memory and the OOM kills of an instance of the incus tier after it
//! stopped. incusd gives them only while the instance runs, and the
//! instance of `run -d` stops when its command ends; its cgroup goes at the
//! stop.
//!
//! The init (`guest-hold --run`, PID 1 of the instance) opens the files of
//! the instance's cgroup before the command starts ([`Meter::open`]). When
//! the command ended, it ends each other process of the instance
//! ([`end_tree`]), takes back its own root ([`Anchor`]), reads the files and
//! writes [`USAGE`] ([`seal`]). Only then does PID 1 end, and the instance
//! stop. `inspect` of a stopped instance reads the file ([`Measure::parse`]).
//!
//! The command cannot give other values: no process of it lives when the
//! init reads and writes, the init reads through descriptors that it
//! opened before the command started, and [`seal`] takes away what the
//! command left at the path (a mount, a link, a directory, a file at the
//! temporary name) and replaces the file atomically.

use std::ffi::{c_char, c_ulong};
use std::fs::File;
use std::io::{Read, Seek};
use std::time::{Duration, Instant};

/// The usage of the instance at the end of its command, written by its init.
pub const USAGE: &str = "/.podvm/usage";
/// The directory of podvm's files in the instance.
const DIR: &str = "/.podvm";
/// The mount point of the cgroup file systems while the init opens them.
const MOUNT: &str = "/.podvm/.meter";
/// The most bytes of [`USAGE`] that a reader takes.
pub const MAX: usize = 4096;
/// The most bytes of a file of a cgroup that the init reads.
const MAX_CGROUP_FILE: u64 = 64 * 1024;
/// The time that PID 1 gives the processes it killed to go.
pub const END_WAIT: Duration = Duration::from_secs(5);

/// What the init measured at the end of an instance. A value is `None`
/// where the init could not read it; `why` gives the reasons.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Measure {
    pub cpu_usec: Option<u64>,
    pub peak_bytes: Option<u64>,
    pub oom_kills: Option<u64>,
    pub why: Vec<String>,
}

impl Measure {
    /// The JSON of [`USAGE`], version 1.
    pub fn to_json(&self) -> Vec<u8> {
        let mut s = serde_json::json!({
            "v": 1,
            "cpu_usec": self.cpu_usec,
            "peak_bytes": self.peak_bytes,
            "oom_kills": self.oom_kills,
            "why": self.why,
        })
        .to_string();
        s.push('\n');
        s.into_bytes()
    }

    /// The measure of the bytes of [`USAGE`]. A file that is too large, not
    /// JSON, of another version, or with a value that is not a count is an
    /// error that names the fault: the reader gives no value of it.
    pub fn parse(b: &[u8]) -> Result<Measure, String> {
        if b.len() > MAX {
            return Err(format!("{} bytes, more than {MAX}", b.len()));
        }
        let v: serde_json::Value =
            serde_json::from_slice(b).map_err(|e| format!("not JSON: {e}"))?;
        let o = v.as_object().ok_or("not a JSON object")?;
        match o.get("v").and_then(serde_json::Value::as_u64) {
            Some(1) => {}
            Some(n) => return Err(format!("version {n}, not 1")),
            None => return Err("no version".into()),
        }
        let count = |k: &str| -> Result<Option<u64>, String> {
            match o.get(k) {
                None | Some(serde_json::Value::Null) => Ok(None),
                Some(x) => x
                    .as_u64()
                    .map(Some)
                    .ok_or_else(|| format!("{k} is not a count: {x}")),
            }
        };
        let why = match o.get("why") {
            None | Some(serde_json::Value::Null) => Vec::new(),
            Some(serde_json::Value::Array(a)) => a
                .iter()
                .take(8)
                .map(|x| match x.as_str() {
                    Some(s) => s
                        .chars()
                        .take(200)
                        .map(|c| if c.is_control() { '?' } else { c })
                        .collect(),
                    None => x.to_string(),
                })
                .collect(),
            Some(x) => return Err(format!("why is not a list: {x}")),
        };
        Ok(Measure {
            cpu_usec: count("cpu_usec")?,
            peak_bytes: count("peak_bytes")?,
            oom_kills: count("oom_kills")?,
            why,
        })
    }
}

/// The CPU time in microseconds of the text of `cpu.stat` (cgroup v2).
pub fn cpu_usec_v2(text: &str) -> Option<u64> {
    text.lines()
        .find_map(|l| l.strip_prefix("usage_usec "))
        .and_then(|n| n.trim().parse().ok())
}

/// The CPU time in microseconds of the text of `cpuacct.usage` (cgroup v1,
/// nanoseconds).
pub fn cpu_usec_v1(text: &str) -> Option<u64> {
    text.trim().parse::<u64>().ok().map(|ns| ns / 1000)
}

/// The bytes of the text of `memory.peak` (v2) or
/// `memory.max_usage_in_bytes` (v1).
pub fn bytes(text: &str) -> Option<u64> {
    text.trim().parse().ok()
}

/// Where the CPU time comes from.
enum Cpu {
    /// `cpu.stat` of cgroup v2.
    V2(File),
    /// `cpuacct.usage` of cgroup v1.
    V1(File),
}

/// The files of the cgroup of the instance, opened before the command
/// starts.
#[derive(Default)]
pub struct Meter {
    cpu: Option<Cpu>,
    peak: Option<File>,
    oom: Option<File>,
    why: Vec<String>,
}

unsafe extern "C" {
    fn mount(
        src: *const c_char,
        target: *const c_char,
        fstype: *const c_char,
        flags: c_ulong,
        data: *const u8,
    ) -> i32;
    fn umount2(target: *const c_char, flags: i32) -> i32;
}
const MS_NOSUID: c_ulong = 2;
const MS_NODEV: c_ulong = 4;
const MS_NOEXEC: c_ulong = 8;
const MNT_DETACH: i32 = 2;
const UMOUNT_NOFOLLOW: i32 = 8;

fn cstr(s: &str) -> std::ffi::CString {
    std::ffi::CString::new(s).unwrap_or_default()
}

/// Mount a cgroup file system of `fstype` with `opts` at [`MOUNT`], run
/// `f` on it, and take the mount away again.
fn with_mount<T>(fstype: &str, opts: Option<&str>, f: impl FnOnce() -> T) -> Result<T, String> {
    let (target, ty, src) = (cstr(MOUNT), cstr(fstype), cstr(fstype));
    let data = opts.map(cstr);
    let flags = MS_NOSUID | MS_NODEV | MS_NOEXEC;
    // SAFETY: C strings that live through the call.
    let r = unsafe {
        mount(
            src.as_ptr(),
            target.as_ptr(),
            ty.as_ptr(),
            flags,
            data.as_ref()
                .map_or(std::ptr::null(), |d| d.as_ptr().cast()),
        )
    };
    if r != 0 {
        let e = std::io::Error::last_os_error();
        return Err(format!(
            "mount -t {fstype}{}: {e}",
            opts.map(|o| format!(" -o {o}")).unwrap_or_default()
        ));
    }
    let out = f();
    // SAFETY: a C string that lives through the call. The open files keep
    // their file system after the detach.
    unsafe { umount2(target.as_ptr(), MNT_DETACH) };
    Ok(out)
}

fn open_in(name: &str) -> Result<File, String> {
    let p = format!("{MOUNT}/{name}");
    File::open(&p).map_err(|e| format!("{name}: {e}"))
}

impl Meter {
    /// Open the files of the cgroup of the instance, as its init sees it
    /// (the root of its cgroup namespace). cgroup v2 serves where its
    /// memory controller is there; else cgroup v1 (`memory` and
    /// `cpuacct`). The `cpu.stat` of a cgroup v2 with no controllers is
    /// that of the host on a host with both versions: it is not taken.
    pub fn open() -> Meter {
        let mut m = Meter::default();
        if let Err(e) = std::fs::create_dir_all(MOUNT) {
            m.why.push(format!("{MOUNT}: {e}"));
            return m;
        }
        let v2 = with_mount("cgroup2", None, || {
            let peak = open_in("memory.peak")?;
            Ok::<_, String>((peak, open_in("cpu.stat"), open_in("memory.events")))
        });
        match v2 {
            Ok(Ok((peak, cpu, oom))) => {
                m.peak = Some(peak);
                match cpu {
                    Ok(f) => m.cpu = Some(Cpu::V2(f)),
                    Err(e) => m.why.push(format!("cgroup2: {e}")),
                }
                match oom {
                    Ok(f) => m.oom = Some(f),
                    Err(e) => m.why.push(format!("cgroup2: {e}")),
                }
            }
            v2 => {
                let v2_why = match v2 {
                    Ok(Err(e)) => format!("cgroup2: {e}"),
                    Err(e) => e,
                    Ok(Ok(_)) => String::new(),
                };
                let mem = with_mount("cgroup", Some("memory"), || {
                    (
                        open_in("memory.max_usage_in_bytes"),
                        open_in("memory.oom_control"),
                    )
                });
                let cpu = with_mount("cgroup", Some("cpuacct"), || open_in("cpuacct.usage"))
                    .or_else(|_| {
                        with_mount("cgroup", Some("cpu,cpuacct"), || open_in("cpuacct.usage"))
                    });
                match mem {
                    Ok((peak, oom)) => {
                        match peak {
                            Ok(f) => m.peak = Some(f),
                            Err(e) => m.why.push(format!("cgroup v1: {e}")),
                        }
                        match oom {
                            Ok(f) => m.oom = Some(f),
                            Err(e) => m.why.push(format!("cgroup v1: {e}")),
                        }
                    }
                    Err(e) => m.why.push(format!("{v2_why}; {e}")),
                }
                match cpu {
                    Ok(Ok(f)) => m.cpu = Some(Cpu::V1(f)),
                    Ok(Err(e)) => m.why.push(format!("cgroup v1: {e}")),
                    Err(e) => m.why.push(format!("{v2_why}; {e}")),
                }
            }
        }
        let _ = std::fs::remove_dir(MOUNT);
        m
    }

    /// The values of the files now.
    pub fn read(&mut self) -> Measure {
        let mut why = std::mem::take(&mut self.why);
        let mut get = |f: &mut File, name: &str| -> Option<String> {
            match reread(f) {
                Ok(t) => Some(t),
                Err(e) => {
                    why.push(format!("{name}: {e}"));
                    None
                }
            }
        };
        let cpu_usec = match &mut self.cpu {
            Some(Cpu::V2(f)) => get(f, "cpu.stat").and_then(|t| cpu_usec_v2(&t)),
            Some(Cpu::V1(f)) => get(f, "cpuacct.usage").and_then(|t| cpu_usec_v1(&t)),
            None => None,
        };
        let peak_bytes = self
            .peak
            .as_mut()
            .and_then(|f| get(f, "the peak memory"))
            .and_then(|t| bytes(&t));
        let oom_kills = self
            .oom
            .as_mut()
            .and_then(|f| get(f, "the OOM events"))
            .and_then(|t| podvm_supervise::report::oom_kills_in(&t));
        Measure {
            cpu_usec,
            peak_bytes,
            oom_kills,
            why,
        }
    }
}

/// The text of an open file of a cgroup from its start: the kernel makes
/// the text again at each read from offset 0.
fn reread(f: &mut File) -> std::io::Result<String> {
    f.rewind()?;
    let mut s = String::new();
    f.take(MAX_CGROUP_FILE).read_to_string(&mut s)?;
    Ok(s)
}

/// The root of the init when it started. A command with root in the
/// instance can change the root of each process of the mount namespace
/// (`pivot_root`); the init takes its own back before it writes.
pub struct Anchor(File);

impl Anchor {
    /// The root of this process now.
    pub fn take() -> Option<Anchor> {
        File::open("/").ok().map(Anchor)
    }

    /// Make the root of [`Anchor::take`] the root and the working directory
    /// of this process again.
    pub fn restore(&self) -> Result<(), String> {
        use std::os::fd::AsRawFd;
        unsafe extern "C" {
            fn fchdir(fd: i32) -> i32;
            fn chroot(path: *const c_char) -> i32;
        }
        // SAFETY: plain system calls on a descriptor that this value holds.
        if unsafe { fchdir(self.0.as_raw_fd()) } != 0 {
            return Err(format!("fchdir: {}", std::io::Error::last_os_error()));
        }
        // SAFETY: a C string literal.
        if unsafe { chroot(c".".as_ptr()) } != 0 {
            return Err(format!("chroot: {}", std::io::Error::last_os_error()));
        }
        std::env::set_current_dir("/").map_err(|e| format!("chdir /: {e}"))
    }
}

/// PID 1 of the instance: send SIGKILL to each other process of its PID
/// namespace and reap each child until none stays or `wait` passed. True
/// where none stays. A process that SIGKILL ends runs no more code of its
/// own, also when it goes late.
pub fn end_tree(wait: Duration) -> bool {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
        fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    }
    const WNOHANG: i32 = 1;
    const ECHILD: i32 = 10;
    const EINTR: i32 = 4;
    // SAFETY: a plain system call; PID 1 itself is not a target of -1.
    unsafe { kill(-1, 9) };
    let start = Instant::now();
    loop {
        let mut st = 0;
        // SAFETY: a plain system call with a local status word.
        let r = unsafe { waitpid(-1, &mut st, WNOHANG) };
        if r < 0 {
            match std::io::Error::last_os_error().raw_os_error() {
                Some(ECHILD) => return true,
                Some(EINTR) if start.elapsed() < wait => continue,
                _ => return false,
            }
        }
        if r == 0 {
            if start.elapsed() >= wait {
                return false;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Write `body` to `path` (a file of [`DIR`]) so that a reader of the
/// instance's root finds this text: take away a mount on [`DIR`] or on
/// the path, a link or a file at [`DIR`], a directory at the path and a
/// file at a temporary name of the path, then replace the file
/// atomically. On an error the file at the path goes, where it can.
pub fn seal(path: &str, body: &[u8]) -> Result<(), String> {
    let r = seal_inner(path, body);
    if r.is_err() {
        let _ = std::fs::remove_file(path);
    }
    r
}

fn seal_inner(path: &str, body: &[u8]) -> Result<(), String> {
    let name = path
        .strip_prefix(DIR)
        .and_then(|n| n.strip_prefix('/'))
        .filter(|n| !n.is_empty() && !n.contains('/'))
        .ok_or_else(|| format!("{path} is not a file of {DIR}"))?;
    unmount_all(DIR);
    match std::fs::symlink_metadata(DIR) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => {
            std::fs::remove_file(DIR).map_err(|e| format!("{DIR}: {e}"))?;
            std::fs::create_dir(DIR).map_err(|e| format!("{DIR}: {e}"))?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(DIR).map_err(|e| format!("{DIR}: {e}"))?;
        }
        Err(e) => return Err(format!("{DIR}: {e}")),
    }
    unmount_all(path);
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir()) {
        std::fs::remove_dir_all(path).map_err(|e| format!("{path}: {e}"))?;
    }
    let stray = format!(".{name}.");
    if let Ok(rd) = std::fs::read_dir(DIR) {
        for e in rd.flatten() {
            let n = e.file_name();
            let n = n.to_string_lossy();
            if n.starts_with(&stray) && n.ends_with(".tmp") {
                let p = e.path();
                let _ = std::fs::remove_dir_all(&p).or_else(|_| std::fs::remove_file(&p));
            }
        }
    }
    pod_core::files::replace(
        std::path::Path::new(path),
        body,
        pod_core::files::Mode::Exact(0o644),
    )
    .map(drop)
    .map_err(|e| e.to_string())
}

/// Take away each mount at `path` (a link at the path is not followed).
fn unmount_all(path: &str) {
    let p = cstr(path);
    for _ in 0..64 {
        // SAFETY: a C string that lives through the call.
        if unsafe { umount2(p.as_ptr(), MNT_DETACH | UMOUNT_NOFOLLOW) } != 0 {
            break;
        }
    }
}
