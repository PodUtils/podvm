//! The room of incusd for an image, TODO/machine.md T-2078
//! (PodUtils/podrunner#346).
//!
//! On a storage pool of the driver `dir`, incusd unpacks the image into the
//! root of each new instance with `tar`. Where the pool fills, `tar` writes
//! one line for each file that it cannot write, and incusd puts all of its
//! stderr into the error of the operation: 108 MB for 400 000 files on a
//! pool of 359 MiB (measured on 2026-11-23). The answer is then over the
//! bound of the client (64 MiB), and the client gives "the response body is
//! larger than allowed" with no word of the cause.
//!
//! So, before an import, podvm compares what the image needs with the free
//! space of the pool and of incusd's images, and refuses by name where the
//! image cannot fit. After a call that failed in its transport, it gives
//! one more line with the free space and the need.
//!
//! What the image needs is a lower bound: the bytes of each file of the
//! tree and one inode for each entry that needs its own inode. A stream
//! counts a hard link once per inode. The tree-based tarball writes each
//! name with its bytes. incusd writes the upload into its images, then the root
//! of the instance. TODO/machine.md T-2085 (PodUtils/podrunner#347 ask 1):
//! where incusd's images have less room than [`super::gzip::ROOM_TIMES`]
//! times the tree, the upload is gzip ([`super::gzip`]), whose size is not
//! known before the upload: its lower bound is 0, and its estimate is the
//! bytes of the layers of the image. A plain upload needs the bytes of the
//! tree. A fallback import removes its own extracted tree before the create: on the file system of that
//! tree, the root of the instance takes its place, and its lower bound is
//! then 0. The free space is that of a user (`f_bavail`): the blocks that
//! ext4 keeps for root stay for the host. The estimate of
//! [`ESTIMATE_PERCENT`] comes from experiment 434 clause 21 (incusd's
//! image, its volume and its own files: 2.38 times the tree, bound 2.6),
//! and gives a warning only.
//!
//! [`super::target`] finds the pool and reads its free space: `statfs` of a
//! pool of the driver `dir` that podvm sees, else the resources that incusd
//! gives for each driver. A driver other than `dir` and thick `lvm` makes
//! the root of an instance as a clone of one volume of the image
//! ([`clones`]): an import needs the bytes of the tree there for that
//! volume, and a create of an image that incus has needs no bytes that
//! podvm can bound.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use super::target::{after_failure, incus_dir, room_at, Target};

use pod_hostprobe::space::mib;

/// What the root of an image needs: the bytes of its files and its count
/// of entries (files, directories, links).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Need {
    pub bytes: u64,
    pub entries: u64,
}

/// The free space of a file system for a user; `inodes` is `None` where
/// the file system does not count them (btrfs, tmpfs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Room {
    pub bytes: u64,
    pub inodes: Option<u64>,
}

/// The estimate of the need, in percent of the lower bound.
pub const ESTIMATE_PERCENT: u64 = 130;

/// The verdict of one file system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Fits,
    /// More free than the lower bound, less than the estimate.
    Tight {
        estimate: u64,
    },
    /// Fewer free bytes than the lower bound.
    NoBytes,
    /// Fewer free inodes than the entries.
    NoInodes,
}

/// The verdict of `need` on `room`; `extra` is bytes of the estimate only
/// (the upload of T-2085).
pub fn judge(need: Need, extra: u64, room: Room) -> Verdict {
    if room.bytes < need.bytes {
        return Verdict::NoBytes;
    }
    if room.inodes.is_some_and(|i| i < need.entries) {
        return Verdict::NoInodes;
    }
    let estimate = (need.bytes.saturating_mul(ESTIMATE_PERCENT) / 100).saturating_add(extra);
    if room.bytes < estimate {
        return Verdict::Tight { estimate };
    }
    Verdict::Fits
}

/// The need of the root at `rootfs` with podvm of `podvm_len` bytes at
/// `/.podvm/podvm`, as the unified tarball writes it ([`super::image`]).
/// An entry that cannot be read counts nothing: the need stays a lower
/// bound.
pub fn need_of(rootfs: &Path, podvm_len: u64) -> Need {
    let mut need = Need {
        bytes: podvm_len,
        entries: 2,
    };
    let init_dir = rootfs.join(super::image::INIT_DIR);
    let init_file = rootfs.join(super::image::INIT_FILE);
    use std::os::unix::fs::MetadataExt;
    let mut inodes = std::collections::HashSet::new();
    let mut stack = vec![rootfs.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let path = e.path();
            if path == init_file {
                continue;
            }
            let Ok(md) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            // T-2079 (OR-B39-08): the archive gives each other name of a
            // file as a hard link, so its inode and its bytes count once.
            if md.is_file() && md.nlink() > 1 && !inodes.insert((md.dev(), md.ino())) {
                continue;
            }
            if path != init_dir {
                need.entries += 1;
            }
            if md.is_dir() {
                stack.push(path);
            } else if md.is_file() && path != init_dir {
                need.bytes = need.bytes.saturating_add(md.len());
            }
        }
    }
    need
}

/// What an import and a create need (TODO/machine.md T-2085).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Plan {
    /// The tree: the root of the instance.
    pub tree: Need,
    /// The estimate of the upload where it is gzip: the bytes of the
    /// layers.
    pub upload: u64,
    /// The upload is gzip; else it is the plain tarball.
    pub gzip: bool,
    /// The device of the store's tree where the import removes it before
    /// the create.
    pub freed: Option<u64>,
    /// This run imported the image: a pool of clones makes the volume of
    /// the image at the create, also after the import.
    pub imported: bool,
}

/// A file system that incusd writes: its words, a path on it (empty where
/// incusd names none), and its device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spot {
    pub label: String,
    pub path: PathBuf,
    pub dev: Dev,
    /// The pool makes the root of an instance as a clone of the volume of
    /// its image ([`clones`]).
    pub clones: bool,
    /// The free space as incusd measured it; `None`: `statfs` of `path`.
    pub room: Option<Room>,
}

impl Spot {
    /// The words and the path of the spot, for a line.
    pub fn place(&self) -> String {
        if self.path.as_os_str().is_empty() {
            self.label.clone()
        } else {
            format!("{} at {}", self.label, self.path.display())
        }
    }
}

/// What podvm knows of the file system under a spot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dev {
    /// The device of the file system (`statfs`).
    Is(u64),
    /// The pool has storage of its own: a loop file of incusd, a block
    /// device, a volume group. A file that podvm removes on another file
    /// system gives it no room.
    Own,
    /// Not known: the pool can share a file system with the store of podvm
    /// or with incusd's images (a `dir`, a dataset of ZFS, a directory on
    /// btrfs).
    Unknown,
}

impl Dev {
    /// The device of a pool of `driver` on `source`, as incusd names them.
    pub fn of(driver: &str, source: &str) -> Dev {
        let own_driver = matches!(driver, "lvm" | "lvmcluster" | "ceph" | "powerflex" | "pure");
        if own_driver || source.starts_with("/dev/") || source.ends_with(".img") {
            Dev::Own
        } else {
            Dev::Unknown
        }
    }

    /// True where both are one known file system. Two loads on spots that
    /// are not known to be one each keep their own lower bound.
    pub fn same(self, other: Dev) -> bool {
        matches!((self, other), (Dev::Is(a), Dev::Is(b)) if a == b)
    }

    /// True where the removal of the store's tree on the device `freed` can
    /// give room here: so the lower bound of a load here is 0.
    pub fn can_gain(self, freed: Option<u64>) -> bool {
        match self {
            Dev::Is(d) => freed == Some(d),
            Dev::Own => false,
            Dev::Unknown => freed.is_some(),
        }
    }
}

/// True where a pool of `driver` with `config` makes the root of an
/// instance as a clone of the volume of its image. Only `dir` and thick
/// `lvm` copy each file; an unknown driver counts as a clone, so that the
/// need stays a lower bound.
pub fn clones(driver: &str, config: &BTreeMap<String, String>) -> bool {
    match driver {
        "dir" | "lvmcluster" => false,
        "lvm" => config.get("lvm.use_thinpool").is_none_or(|v| v != "false"),
        _ => true,
    }
}

/// What one step writes on one file system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Load {
    pub spot: Spot,
    /// What goes there: "the root of the instance", ...
    pub what: &'static str,
    /// The lower bound.
    pub need: Need,
    /// Bytes of the estimate only: the upload of gzip.
    pub extra: u64,
}

/// The loads of a create (and, with `importing`, of the import before it)
/// on the pool and on incusd's images: one load for each file system.
pub fn loads(pool: &Spot, images: Option<&Spot>, plan: &Plan, importing: bool) -> Vec<Load> {
    // T-2085: on the file system of the store's tree that the import
    // removes, the root of the instance takes its place.
    let (root, root_what) = if importing && pool.dev.can_gain(plan.freed) {
        (
            Need::default(),
            "the root of the instance (in the place of podvm's tree, which the import removes)",
        )
    } else if pool.clones && (importing || plan.imported) {
        (
            plan.tree,
            "the volume of the image (the root of the instance is a clone of it)",
        )
    } else if pool.clones {
        (
            Need::default(),
            "the root of the instance (a clone of the volume of its image)",
        )
    } else {
        (plan.tree, "the root of the instance")
    };
    // A gzip upload has no lower bound before it is written; its estimate
    // is the bytes of the layers. A plain one needs the bytes of the tree.
    let (upload, upload_extra, upload_what) = if plan.gzip {
        (
            Need {
                bytes: 0,
                entries: 1,
            },
            plan.upload,
            "the upload of the image (gzip)",
        )
    } else {
        (
            Need {
                bytes: plan.tree.bytes,
                entries: 1,
            },
            0,
            "the upload of the image",
        )
    };
    match images {
        Some(i) if importing && i.dev.same(pool.dev) => vec![Load {
            spot: Spot {
                label: format!("{} (also incusd's images)", pool.label),
                ..pool.clone()
            },
            what: match (plan.gzip, root == Need::default()) {
                (true, true) => {
                    "the upload of the image (gzip) and the root of the instance (in the place \
                     of podvm's tree, which the import removes)"
                }
                (true, false) => "the upload of the image (gzip) and the root of the instance",
                (false, true) => {
                    "the upload of the image and the root of the instance (in the place of \
                     podvm's tree, which the import removes)"
                }
                (false, false) => "the image of incusd and the root of the instance",
            },
            need: Need {
                bytes: root.bytes.saturating_add(upload.bytes),
                entries: root.entries.saturating_add(upload.entries),
            },
            extra: upload_extra,
        }],
        Some(i) if importing => vec![
            Load {
                spot: i.clone(),
                what: upload_what,
                need: upload,
                extra: upload_extra,
            },
            Load {
                spot: pool.clone(),
                what: root_what,
                need: root,
                extra: 0,
            },
        ],
        _ => vec![Load {
            spot: pool.clone(),
            what: root_what,
            need: root,
            extra: 0,
        }],
    }
}

/// The line of `verdict` for `load` with `room`; `None` where it fits.
pub fn line(load: &Load, room: Room, verdict: Verdict) -> Option<String> {
    let Load {
        spot, what, need, ..
    } = load;
    let at = spot.place();
    let why = "(TODO/machine.md T-2078, PodUtils/podrunner#346)";
    match verdict {
        Verdict::Fits => None,
        Verdict::NoBytes => Some(format!(
            "{at} has {} free and {what} needs at least {}: incusd would fill it and \
             fail the copy {why}; free space there, or give the default profile a root \
             disk on a pool with room",
            mib(room.bytes),
            mib(need.bytes)
        )),
        Verdict::NoInodes => Some(format!(
            "{at} has {} free inodes and {what} needs at least {} (one for each file \
             and directory): incusd would fail the copy {why}; free inodes there, or give \
             the default profile a root disk on a pool with room",
            room.inodes.unwrap_or(0),
            need.entries
        )),
        Verdict::Tight { estimate } => Some(format!(
            "warning: {at} has {} free; {what} needs at least {} and an estimated {} \
             (experiment 434 clause 21): incusd can fail the copy for lack of space {why}",
            mib(room.bytes),
            mib(need.bytes),
            mib(estimate)
        )),
    }
}

/// True where each storage driver of the server (`environment.storage`,
/// `dir | zfs` for two) is `dir`.
pub fn dir_only(storage: &str) -> bool {
    !storage.trim().is_empty() && storage.split('|').all(|d| d.trim() == "dir")
}

/// A name of a pool that is one path segment.
pub(super) fn segment(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\0'])
}

/// The class of a failed call in its transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// Not a fault of the transport: incusd answered.
    None,
    /// The answer was over the bound of the client.
    TooLarge,
    /// incusd's error says that a disk is full ("No space left on device").
    NoSpace,
    /// Another read or write fault of the connection.
    Other,
}

/// The class of `e`. pod-incus gives the refusal by size its own kind
/// (`ErrorKind::TooLarge`, podcrates `v0.83.0`, #651); a silence of incusd
/// (`ErrorKind::AnswerTimeout`) is a fault of the transport too.
pub fn transport(e: &pod_incus::Error) -> Transport {
    if super::cut::no_space(e) {
        return Transport::NoSpace;
    }
    match e.kind() {
        pod_incus::ErrorKind::TooLarge { .. }
        | pod_incus::ErrorKind::Incus { cut: Some(_), .. } => Transport::TooLarge,
        pod_incus::ErrorKind::Io(_) | pod_incus::ErrorKind::AnswerTimeout { .. } => {
            Transport::Other
        }
        _ => Transport::None,
    }
}

/// The plan of a tree at `rootfs` with no upload and no removal, or `None`
/// where the tree is not there (T-2085).
pub fn plan_of(rootfs: &Path) -> Option<Plan> {
    if !rootfs.is_dir() {
        return None;
    }
    let podvm = std::fs::metadata("/proc/self/exe").map_or(0, |m| m.len());
    Some(Plan {
        tree: need_of(rootfs, podvm),
        ..Plan::default()
    })
}
