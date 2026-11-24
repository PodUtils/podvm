//! TODO/machine.md T-2086 (PodUtils/podrunner#350): the cgroup mounts that
//! an incus instance has from its start, the second path of the meter of
//! [`super::Meter::open`] where the init cannot mount a cgroup file system
//! itself (the AppArmor profile of incus).

use std::path::{Path, PathBuf};

use pod_hostprobe::usage;

/// The magic of cgroup v1 (`CGROUP_SUPER_MAGIC` of `statfs(2)`).
pub const CGROUP_SUPER_MAGIC: u64 = 0x0027_e0eb;

/// The `f_type` of `statfs(2)` of `path`, or `None`.
pub(super) fn fs_magic(path: &Path) -> Option<u64> {
    let p = podvm_probe::sys::CBuf::new(path.to_str()?)?;
    podvm_probe::sys::statfs(&p).ok().map(|s| s.f_type)
}

/// The cgroup mounts that an instance has from its start: LXC mounts them
/// under `/sys/fs/cgroup`, in the cgroup namespace of the instance, before
/// its init runs. Only a mount that shows the root of its hierarchy is
/// taken (the field `root` of mountinfo is `/`): there it is the root of
/// the cgroup namespace, the cgroup of the instance. A mount of a sub-tree
/// gives another cgroup.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Mounted {
    /// The mount point of cgroup2.
    pub v2: Option<PathBuf>,
    /// The mount point of the cgroup v1 hierarchy with `memory`.
    pub memory: Option<PathBuf>,
    /// The mount point of the cgroup v1 hierarchy with `cpuacct`.
    pub cpuacct: Option<PathBuf>,
}

impl Mounted {
    /// The mounts of a text of `/proc/self/mountinfo`. A line that is not
    /// of the form, or whose path is not absolute after the unescape, does
    /// not count. The last line of a kind is taken: a later mount at a
    /// point covers an earlier one.
    pub fn of(info: &str) -> Mounted {
        let mut m = Mounted::default();
        for line in info.lines() {
            let Some((left, right)) = line.split_once(" - ") else {
                continue;
            };
            let left: Vec<&str> = left.split(' ').collect();
            let right: Vec<&str> = right.split(' ').collect();
            let (Some(root), Some(point), Some(fstype)) = (left.get(3), left.get(4), right.first())
            else {
                continue;
            };
            if *root != "/" {
                continue;
            }
            let Some(point) = unescape(point).filter(|p| p.starts_with('/')) else {
                continue;
            };
            let point = PathBuf::from(point);
            match *fstype {
                "cgroup2" => m.v2 = Some(point),
                "cgroup" => {
                    let opts = right.get(2).copied().unwrap_or("");
                    let has = |c: &str| opts.split(',').any(|o| o == c);
                    if has("memory") {
                        m.memory = Some(point.clone());
                    }
                    if has("cpuacct") {
                        m.cpuacct = Some(point);
                    }
                }
                _ => {}
            }
        }
        m
    }

    /// The mounts whose file system is the one that mountinfo names now
    /// (`magic` gives the `f_type` of `statfs(2)` of a path): a mount over
    /// a mount point hides it.
    pub fn checked(self, magic: &dyn Fn(&Path) -> Option<u64>) -> Mounted {
        let is = |p: Option<PathBuf>, want: u64| p.filter(|p| magic(p) == Some(want));
        Mounted {
            v2: is(self.v2, pod_hostprobe::cgroup::CGROUP2_SUPER_MAGIC),
            memory: is(self.memory, CGROUP_SUPER_MAGIC),
            cpuacct: is(self.cpuacct, CGROUP_SUPER_MAGIC),
        }
    }

    /// The meter of these mounts, by the rule of `pod_hostprobe::usage`.
    pub fn meter(&self) -> usage::Meter {
        let mut m = usage::Meter::open(
            self.v2.as_deref(),
            self.memory.as_deref(),
            self.cpuacct.as_deref(),
        );
        if self.v2.is_none() && self.memory.is_none() && self.cpuacct.is_none() {
            m.note("the instance has no cgroup mount that shows the root of its hierarchy");
        }
        m
    }
}

/// A path of mountinfo with its octal escapes (`\040` for a space) undone,
/// or `None` where an escape is not three octal digits or the path is not
/// UTF-8.
fn unescape(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' {
            let d = b.get(i + 1..i + 4)?;
            if !d.iter().all(|c| (b'0'..=b'7').contains(c)) {
                return None;
            }
            let v = d.iter().fold(0u32, |a, c| a * 8 + u32::from(c - b'0'));
            out.push(u8::try_from(v).ok()?);
            i += 4;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}
