//! The cgroup2 subtree of an init of the image (TODO/enter.md T-2002).
//!
//! systemd as PID 1 makes `init.scope` and a cgroup for each unit, so it
//! needs a cgroup2 directory that it can write. A caller with no root has
//! one only where the host delegates a subtree (a user manager of systemd,
//! a unit with `Delegate=`, a container with a delegated cgroup). The leg
//! of `pod_hostprobe::cgroup` finds it with a real write.
//!
//! The layout under the delegated directory, for one run:
//!
//! ```text
//! DELEGATED/podvm-init-ID/            the base, with no process
//!                        /supervisor  podvm and the command of the run
//!                        /init        the root of the cgroup namespace of
//!                                     the init of the image
//! ```

use std::path::{Path, PathBuf};

use podvm_probe::cgroup::{self, Delegation, Reason};

/// The directory that the caller can write, or the reason for none in the
/// words of the leg.
pub fn delegated() -> Result<PathBuf, String> {
    verdict(cgroup::probe().verdict).or_else(|first| hybrid().ok_or(first))
}

/// A writable directory needs no controller here: systemd runs with the
/// controllers that it finds, also with none.
fn verdict(v: Delegation) -> Result<PathBuf, String> {
    match v {
        Delegation::Delegated { path, .. } => Ok(path),
        Delegation::None(Reason::MissingControllers { path, .. }) => Ok(path),
        other => Err(other.words()),
    }
}

/// A host with cgroup v1 at `/sys/fs/cgroup` and cgroup2 at another mount
/// point (the hybrid layout): the same leg on that mount point.
fn hybrid() -> Option<PathBuf> {
    let info = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    let root = mount_point(&info)?;
    let membership = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let leg = cgroup::assess(
        Ok(cgroup::CGROUP2_SUPER_MAGIC),
        &membership,
        Path::new(&root),
        &|dir| write_test(dir).map_err(|e| podvm_probe::sys::Errno(e.raw_os_error().unwrap_or(5))),
    );
    verdict(leg.verdict).ok()
}

/// The first cgroup2 mount point of a mountinfo text.
pub fn mount_point(info: &str) -> Option<String> {
    info.lines().find_map(|l| {
        let (left, right) = l.split_once(" - ")?;
        (right.split(' ').next() == Some("cgroup2"))
            .then(|| left.split(' ').nth(4).map(str::to_string))
            .flatten()
    })
}

/// A child cgroup made and removed: the real write.
fn write_test(dir: &Path) -> std::io::Result<()> {
    let child = dir.join(format!("podvm-init-probe-{}", std::process::id()));
    std::fs::create_dir(&child)?;
    std::fs::remove_dir(&child)
}

/// The base of this process in a guest: `podvm-init` under its own cgroup.
pub fn own_base() -> Option<PathBuf> {
    let info = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    let root = mount_point(&info)?;
    let membership = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let own = membership.lines().find_map(|l| l.strip_prefix("0::"))?;
    let clean = own.starts_with('/') && own.split('/').all(|c| c != "." && c != "..");
    clean.then(|| {
        Path::new(&root)
            .join(own.trim_start_matches('/'))
            .join("podvm-init")
    })
}

/// The two leaves of a base.
pub struct Leaves {
    pub init: PathBuf,
    pub supervisor: PathBuf,
}

/// Make the base and its leaves, and give the leaves each controller that
/// the directories above offer. A controller that the kernel refuses (a
/// process in the directory above) is left out: systemd runs without it.
pub fn make(base: &Path) -> std::io::Result<Leaves> {
    std::fs::create_dir_all(base)?;
    if let Some(up) = base.parent() {
        enable(up);
    }
    enable(base);
    let leaves = Leaves {
        init: base.join("init"),
        supervisor: base.join("supervisor"),
    };
    for d in [&leaves.init, &leaves.supervisor] {
        match std::fs::create_dir(d) {
            Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => return Err(e),
            _ => {}
        }
    }
    Ok(leaves)
}

fn enable(dir: &Path) {
    let offered = std::fs::read_to_string(dir.join("cgroup.controllers")).unwrap_or_default();
    for c in offered.split_whitespace() {
        let _ = std::fs::write(dir.join("cgroup.subtree_control"), format!("+{c}"));
    }
}

/// Move this process into `leaf`.
pub fn enter(leaf: &Path) -> std::io::Result<()> {
    std::fs::write(leaf.join("cgroup.procs"), "0")
}

/// Remove the base with each cgroup under it, deepest first. A cgroup with
/// a process stays (`rmdir` gives EBUSY); the kernel ends each process of
/// the PID namespace before, so some tries are enough.
pub fn remove(base: &Path) {
    for _ in 0..50 {
        prune(base);
        if !base.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

fn prune(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                prune(&e.path());
            }
        }
    }
    let _ = std::fs::remove_dir(dir);
}

/// The prefix of the base of a run in the delegated directory.
pub const BASE_PREFIX: &str = "podvm-init-";
/// The age from which a base with no process is the rest of a run that
/// was killed: longer than the bound of a boot, so that no run that is
/// between its `mkdir` and its move loses its base.
const STALE: std::time::Duration = std::time::Duration::from_secs(600);

/// True for a base that a killed run left: `cgroup.events` counts no
/// process in the whole subtree, and the directory is old.
pub fn stale(events: &str, age: std::time::Duration) -> bool {
    age >= STALE && events.lines().any(|l| l.trim() == "populated 0")
}

/// Remove each base that a killed run left in the delegated directory. A
/// run that ends removes its own base; a SIGKILL of podvm leaves it empty.
pub fn sweep(delegated: &Path) {
    let Ok(entries) = std::fs::read_dir(delegated) else {
        return;
    };
    for e in entries.flatten() {
        if !e.file_name().to_string_lossy().starts_with(BASE_PREFIX) {
            continue;
        }
        let base = e.path();
        let age = std::fs::metadata(&base)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok());
        let events = std::fs::read_to_string(base.join("cgroup.events")).unwrap_or_default();
        if age.is_some_and(|a| stale(&events, a)) {
            prune(&base);
        }
    }
}
