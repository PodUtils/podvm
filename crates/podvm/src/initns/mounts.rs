//! The mounts of an init of the image, made by the process that becomes
//! PID 1, in its own mount namespace (TODO/enter.md T-2002).
//!
//! systemd in a container wants each of these as a mount point before it
//! starts (its container interface): `/proc` of its PID namespace, `/sys`,
//! `/dev` with the nodes and a devpts instance, a tmpfs on `/run`, cgroup2
//! on `/sys/fs/cgroup`, and `/` as a mount point (its generators set the
//! propagation of `/`). Where a mount is missing, systemd makes it itself,
//! and a user namespace refuses devtmpfs.
//!
//! Each step that the kernel refuses gives one line with the step and the
//! errno; only the steps without which no init starts end the boot.

use std::ffi::CString;
use std::path::{Path, PathBuf};

const MS_RDONLY: u64 = 0x1;
const MS_NOSUID: u64 = 0x2;
const MS_NODEV: u64 = 0x4;
const MS_NOEXEC: u64 = 0x8;
const MS_REMOUNT: u64 = 0x20;
const MS_BIND: u64 = 0x1000;
const MS_MOVE: u64 = 0x2000;
const MS_REC: u64 = 0x4000;
const MS_PRIVATE: u64 = 0x4_0000;
const MNT_DETACH: i32 = 2;
/// The devices of a `/dev` on a tmpfs: binds of the nodes of the host.
const DEVICES: &[&str] = &["null", "zero", "full", "random", "urandom", "tty"];
/// The name of the staging directory where the root of the run is `/` (a
/// guest of the machine tier, whose root is the initramfs).
const STAGE: &str = "/.podvm-initroot";

unsafe extern "C" {
    fn mount(
        src: *const std::ffi::c_char,
        target: *const std::ffi::c_char,
        fstype: *const std::ffi::c_char,
        flags: u64,
        data: *const u8,
    ) -> i32;
    fn umount2(target: *const std::ffi::c_char, flags: i32) -> i32;
    fn chroot(path: *const std::ffi::c_char) -> i32;
    fn syscall(n: i64, ...) -> i64;
}

fn c(p: &Path) -> std::io::Result<CString> {
    CString::new(p.as_os_str().as_encoded_bytes())
        .map_err(|_| std::io::Error::from_raw_os_error(22))
}

/// One mount; an empty `src`, `fstype` or `data` is NULL.
fn mnt(src: &str, dst: &Path, fstype: &str, flags: u64, data: &str) -> std::io::Result<()> {
    let text = |s: &str| CString::new(s).map_err(|_| std::io::Error::from_raw_os_error(22));
    let (s, d, f, o) = (text(src)?, c(dst)?, text(fstype)?, text(data)?);
    let ptr = |x: &CString, empty: bool| if empty { std::ptr::null() } else { x.as_ptr() };
    let data = if data.is_empty() {
        std::ptr::null()
    } else {
        o.as_ptr().cast::<u8>()
    };
    // SAFETY: each pointer is NULL or a C string that lives for the call.
    let r = unsafe {
        mount(
            ptr(&s, src.is_empty()),
            d.as_ptr(),
            ptr(&f, fstype.is_empty()),
            flags,
            data,
        )
    };
    if r == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// A bind of a volume at or under `/tmp`: `GUEST:HOST` or `GUEST:HOST:ro`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bind {
    pub guest: String,
    pub host: String,
    pub read_only: bool,
}

impl Bind {
    pub fn parse(s: &str) -> Option<Bind> {
        let (guest, rest) = s.split_once(':')?;
        let (host, read_only) = match rest.strip_suffix(":ro") {
            Some(h) => (h, true),
            None => (rest, false),
        };
        let clean = guest.starts_with("/tmp")
            && guest.split('/').all(|p| p != "..")
            && host.starts_with('/');
        clean.then(|| Bind {
            guest: guest.to_string(),
            host: host.to_string(),
            read_only,
        })
    }
}

/// What the mounts need from the plan of the run.
pub struct Want<'a> {
    /// The tree of the run; `/` in a guest.
    pub root: &'a Path,
    /// The slave of the console pty, a path of the old root.
    pub console: Option<&'a Path>,
    /// True where the init gets a cgroup namespace of its own.
    pub cgroup: bool,
    /// A tmpfs on `/tmp` with these binds (the namespace rung of the host).
    pub tmp: Option<&'a [Bind]>,
}

/// Make each mount under the root, then make the root the `/` of this
/// process. An error names the step that no init can start without.
pub fn build(w: &Want) -> Result<(), String> {
    let step = |what: &str, e: std::io::Error| format!("{what}: {e}");
    private().map_err(|e| step("the mounts of / as private", e))?;
    let stage = stage(w.root).map_err(|e| step("the bind of the root on itself", e))?;
    let at = |p: &str| stage.join(p);
    for d in ["proc", "sys", "dev", "run", "tmp"] {
        let _ = std::fs::create_dir_all(at(d));
    }
    mnt(
        "proc",
        &at("proc"),
        "proc",
        MS_NOSUID | MS_NODEV | MS_NOEXEC,
        "",
    )
    .map_err(|e| step("the mount of /proc of the PID namespace", e))?;
    sys(&at("sys")).map_err(|e| step("the mount of /sys", e))?;
    dev(&at("dev"), w.console).map_err(|e| step("the mount of /dev", e))?;
    mnt(
        "tmpfs",
        &at("run"),
        "tmpfs",
        MS_NOSUID | MS_NODEV,
        "mode=755",
    )
    .map_err(|e| step("the tmpfs on /run", e))?;
    let _ = std::fs::create_dir_all(at("run/systemd"));
    let _ = std::fs::write(at("run/systemd/container"), "podvm\n");
    if w.cgroup {
        cgroup2(&at("run/.podvm-cgroup"), &at("sys/fs/cgroup"))
            .map_err(|e| step("the mount of cgroup2 on /sys/fs/cgroup", e))?;
    }
    if let Some(binds) = w.tmp {
        mnt(
            "tmpfs",
            &at("tmp"),
            "tmpfs",
            MS_NOSUID | MS_NODEV,
            "mode=1777",
        )
        .map_err(|e| step("the tmpfs on /tmp", e))?;
        for b in binds {
            tmp_bind(&stage, b)
                .map_err(|e| step(&format!("the bind of -v {}:{}", b.host, b.guest), e))?;
        }
    }
    machine_id(&at("etc/machine-id"));
    enter(&stage).map_err(|e| step("the change of the root", e))
}

/// Each mount of this mount namespace as private: no later mount reaches
/// the namespace that this one came from.
pub fn private() -> std::io::Result<()> {
    mnt("", Path::new("/"), "", MS_REC | MS_PRIVATE, "")
}

/// A `/proc` of the PID namespace of this process on `/proc`, in a mount
/// namespace of its own: the pids of its children then name them there.
pub fn own_proc() -> std::io::Result<()> {
    let flags = MS_NOSUID | MS_NODEV | MS_NOEXEC;
    mnt("proc", Path::new("/proc"), "proc", flags, "")
}

/// The directory that becomes `/`: a mount point in each case.
fn stage(root: &Path) -> std::io::Result<PathBuf> {
    let stage = if root == Path::new("/") {
        std::fs::create_dir_all(STAGE)?;
        PathBuf::from(STAGE)
    } else {
        root.to_path_buf()
    };
    let src = root.to_string_lossy();
    mnt(&src, &stage, "", MS_BIND | MS_REC, "")?;
    Ok(stage)
}

/// The cgroup2 of the cgroup namespace of this process on `target`. The
/// kernel refuses a new mount of a file system on a mount point of the
/// same file system (EBUSY), and the `/sys/fs/cgroup` of a host or of a
/// guest is one: the mount goes to `scratch` first and moves from there.
fn cgroup2(scratch: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(scratch)?;
    mnt(
        "cgroup2",
        scratch,
        "cgroup2",
        MS_NOSUID | MS_NODEV | MS_NOEXEC,
        "",
    )?;
    let from = scratch.to_string_lossy();
    if mnt(&from, target, "", MS_MOVE, "").is_err() {
        mnt(&from, target, "", MS_BIND, "")?;
        // SAFETY: a C string; the detach takes the first mount off.
        unsafe { umount2(c(scratch)?.as_ptr(), MNT_DETACH) };
    }
    let _ = std::fs::remove_dir(scratch);
    Ok(())
}

/// True for the errno of a new mount on a mount point of the same file
/// system: the mount that the root brought is the one that is wanted.
fn there(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(16)
}

/// `/sys`, read-only, as each container manager gives it: systemd then
/// starts no udev, and it does not look for the real terminal behind
/// `/dev/console` (the serial line of a guest, where the output of the
/// command goes). A sysfs of this process where the kernel gives one (root
/// of the initial user namespace, or a network namespace of this user
/// namespace); the one that the root brought; else the `/sys` of the host,
/// bound.
fn sys(target: &Path) -> std::io::Result<()> {
    let own = MS_NOSUID | MS_NODEV | MS_NOEXEC;
    match mnt("sysfs", target, "sysfs", own | MS_RDONLY, "") {
        Ok(()) => return Ok(()),
        Err(e) if there(&e) => {}
        Err(_) => mnt("/sys", target, "", MS_BIND | MS_REC, "")?,
    }
    // A user namespace must keep the flags that the kernel locks; a
    // refusal leaves `/sys` as it is, and the boot goes on.
    if mnt("", target, "", MS_REMOUNT | MS_BIND | MS_RDONLY, "").is_err() {
        let _ = mnt("", target, "", MS_REMOUNT | MS_BIND | MS_RDONLY | own, "");
    }
    Ok(())
}

/// A devpts on the `/dev/pts` of this process, where a guest has none: the
/// console of the init needs a pty.
pub fn devpts_here() -> std::io::Result<()> {
    std::fs::create_dir_all("/dev/pts")?;
    let flags = MS_NOSUID | MS_NOEXEC;
    mnt(
        "devpts",
        Path::new("/dev/pts"),
        "devpts",
        flags,
        "ptmxmode=0666,mode=0620",
    )
}

/// `/dev`: devtmpfs where the kernel gives it; in a user namespace a tmpfs
/// with a bind of each node of the host. Then a devpts instance, `/dev/shm`,
/// `/dev/mqueue`, the links, and the console.
fn dev(target: &Path, console: Option<&Path>) -> std::io::Result<()> {
    let own = match mnt("devtmpfs", target, "devtmpfs", MS_NOSUID, "mode=755") {
        Ok(()) => true,
        Err(e) => there(&e),
    };
    if !own {
        mnt("tmpfs", target, "tmpfs", MS_NOSUID, "mode=755")?;
        for d in DEVICES {
            let node = target.join(d);
            let _ = std::fs::File::create(&node);
            let _ = mnt(&format!("/dev/{d}"), &node, "", MS_BIND, "");
        }
    }
    for d in ["pts", "shm", "mqueue"] {
        let _ = std::fs::create_dir_all(target.join(d));
    }
    let opts = "newinstance,ptmxmode=0666,mode=0620";
    let flags = MS_NOSUID | MS_NOEXEC;
    let pts = target.join("pts");
    // gid 5 is `tty`; a map with one id does not hold it.
    if mnt("devpts", &pts, "devpts", flags, &format!("{opts},gid=5")).is_err() {
        let _ = mnt("devpts", &pts, "devpts", flags, opts);
    }
    if !own {
        let _ = std::os::unix::fs::symlink("pts/ptmx", target.join("ptmx"));
    }
    let _ = mnt(
        "tmpfs",
        &target.join("shm"),
        "tmpfs",
        MS_NOSUID | MS_NODEV,
        "mode=1777",
    );
    let _ = mnt(
        "mqueue",
        &target.join("mqueue"),
        "mqueue",
        MS_NOSUID | MS_NODEV | MS_NOEXEC,
        "",
    );
    for (name, to) in [
        ("fd", "/proc/self/fd"),
        ("stdin", "/proc/self/fd/0"),
        ("stdout", "/proc/self/fd/1"),
        ("stderr", "/proc/self/fd/2"),
    ] {
        let _ = std::os::unix::fs::symlink(to, target.join(name));
    }
    // The console of the init is a pty of podvm, never the terminal or the
    // serial line of the run: the output of the command goes there.
    let node = target.join("console");
    if std::fs::symlink_metadata(&node).is_err() {
        let _ = std::fs::File::create(&node);
    }
    let src = console.unwrap_or(Path::new("/dev/null"));
    let _ = mnt(&src.to_string_lossy(), &node, "", MS_BIND, "");
    Ok(())
}

/// One volume at or under `/tmp`, on the new tmpfs: no link of the image
/// lies on the path.
fn tmp_bind(stage: &Path, b: &Bind) -> std::io::Result<()> {
    let target = stage.join(b.guest.trim_start_matches('/'));
    let host = Path::new(&b.host);
    if std::fs::metadata(host)?.is_dir() {
        std::fs::create_dir_all(&target)?;
    } else {
        if let Some(up) = target.parent() {
            std::fs::create_dir_all(up)?;
        }
        std::fs::File::create(&target)?;
    }
    mnt(&b.host, &target, "", MS_BIND | MS_REC, "")?;
    if b.read_only {
        let keep = MS_NOSUID | MS_NODEV;
        if mnt("", &target, "", MS_REMOUNT | MS_BIND | MS_RDONLY, "").is_err() {
            mnt("", &target, "", MS_REMOUNT | MS_BIND | MS_RDONLY | keep, "")?;
        }
    }
    Ok(())
}

/// An empty machine-id, so that each run gets an id of its own from
/// systemd, with no first-boot pass (as T-1996 does for an incus instance).
/// A link or another kind of file is left alone.
pub fn machine_id(path: &Path) {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => {
            let _ = std::fs::write(path, "");
        }
        Err(_) if path.parent().is_some_and(|p| p.is_dir()) => {
            let _ = std::fs::write(path, "");
        }
        _ => {}
    }
}

/// Make `stage` the root: `pivot_root`, and where the kernel refuses it
/// (the old root is the initramfs of a guest) the move of `switch_root`.
fn enter(stage: &Path) -> std::io::Result<()> {
    const PIVOT_ROOT: i64 = podvm_probe::sys::SYS_PIVOT_ROOT;
    std::env::set_current_dir(stage)?;
    // SAFETY: two C strings; `.` on `.` stacks the old root on the new one.
    if unsafe { syscall(PIVOT_ROOT, c".".as_ptr(), c".".as_ptr()) } == 0 {
        // SAFETY: a C string; the detach takes the old root off.
        if unsafe { umount2(c".".as_ptr(), MNT_DETACH) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        return std::env::set_current_dir("/");
    }
    mnt(".", Path::new("/"), "", MS_MOVE, "")?;
    // SAFETY: a C string.
    if unsafe { chroot(c".".as_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    std::env::set_current_dir("/")
}

#[cfg(test)]
mod tests {
    use super::Bind;

    #[test]
    fn a_bind_names_a_guest_path_under_tmp_and_a_host_path() {
        let b = |guest: &str, host: &str, read_only| Bind {
            guest: guest.to_string(),
            host: host.to_string(),
            read_only,
        };
        for (text, want) in [
            ("/tmp/a:/srv/a", Some(b("/tmp/a", "/srv/a", false))),
            ("/tmp:/srv/t:ro", Some(b("/tmp", "/srv/t", true))),
            ("/etc:/srv/a", None),
            ("/tmp/../etc:/srv/a", None),
            ("/tmp/a:relative", None),
            ("/tmp/a", None),
        ] {
            assert_eq!(Bind::parse(text), want, "{text}");
        }
    }
}
