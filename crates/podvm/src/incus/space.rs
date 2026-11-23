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
//! tree (the unified tarball writes each name with its bytes) and one inode
//! for each entry. incusd writes the image file and the root of the
//! instance; on one file system that is twice the bytes. The free space is
//! that of a user (`f_bavail`): the blocks that ext4 keeps for root stay
//! for the host. The estimate of [`ESTIMATE_PERCENT`] comes from
//! experiment 434 clause 21 (incusd's image, its volume and its own files:
//! 2.38 times the tree, bound 2.6), and gives a warning only.
//!
//! The path of a pool is `INCUS_DIR/storage-pools/NAME`, where each driver
//! mounts its pool, and the root of an instance goes into its `containers`
//! directory; only the driver `dir` keeps the files of an instance there,
//! so another driver gives no check. pod-incus has no call of
//! `GET /1.0/storage-pools/NAME/resources` (podcrates `v0.82.0`): the
//! request to podcrates is in T-2078.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

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

/// The verdict of `need` on `room`.
pub fn judge(need: Need, room: Room) -> Verdict {
    if room.bytes < need.bytes {
        return Verdict::NoBytes;
    }
    if room.inodes.is_some_and(|i| i < need.entries) {
        return Verdict::NoInodes;
    }
    let estimate = need.bytes.saturating_mul(ESTIMATE_PERCENT) / 100;
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
    let mut stack = vec![rootfs.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(md) = std::fs::symlink_metadata(e.path()) else {
                continue;
            };
            need.entries += 1;
            if md.is_dir() {
                stack.push(e.path());
            } else if md.is_file() {
                need.bytes = need.bytes.saturating_add(md.len());
            }
        }
    }
    need
}

/// A file system that incusd writes: its words, a path on it, and its
/// device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spot {
    pub label: String,
    pub path: PathBuf,
    pub dev: u64,
}

/// What one step writes on one file system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Load {
    pub spot: Spot,
    /// What goes there: "the root of the instance", ...
    pub what: &'static str,
    pub need: Need,
}

/// The loads of a create (and, with `importing`, of the import before it)
/// on the pool and on incusd's images: one load for each file system.
pub fn loads(pool: &Spot, images: Option<&Spot>, need: Need, importing: bool) -> Vec<Load> {
    let image_need = Need {
        bytes: need.bytes,
        entries: 1,
    };
    match images {
        Some(i) if importing && i.dev == pool.dev => vec![Load {
            spot: Spot {
                label: format!("{} (also incusd's images)", pool.label),
                ..pool.clone()
            },
            what: "the image of incusd and the root of the instance",
            need: Need {
                bytes: need.bytes.saturating_mul(2),
                entries: need.entries.saturating_add(1),
            },
        }],
        Some(i) if importing => vec![
            Load {
                spot: i.clone(),
                what: "the image of incusd",
                need: image_need,
            },
            Load {
                spot: pool.clone(),
                what: "the root of the instance",
                need,
            },
        ],
        _ => vec![Load {
            spot: pool.clone(),
            what: "the root of the instance",
            need,
        }],
    }
}

/// The line of `verdict` for `load` with `room`; `None` where it fits.
pub fn line(load: &Load, room: Room, verdict: Verdict) -> Option<String> {
    let Load { spot, what, need } = load;
    let at = format!("{} at {}", spot.label, spot.path.display());
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
fn segment(name: &str) -> bool {
    !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\0'])
}

/// The directory of the incusd of `socket`: `INCUS_DIR`, else the directory
/// of the socket.
pub fn incus_dir(socket: &Path) -> Option<PathBuf> {
    match std::env::var_os("INCUS_DIR") {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => socket.parent().map(Path::to_path_buf),
    }
}

/// The free space at `path`, or `None` where `statfs` fails.
pub fn room_at(path: &Path) -> Option<Room> {
    let have = pod_hostprobe::space::read(path.to_str()?).ok()?;
    Some(Room {
        bytes: have.free.bytes,
        inodes: have.inodes_are_counted.then_some(have.free.inodes),
    })
}

/// The pool of the root disk of new instances and incusd's images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub pool: Spot,
    pub images: Option<Spot>,
}

impl Target {
    /// The target in the incusd directory `dir` for the pool `name`; `None`
    /// where the name is not one segment or the pool does not show the
    /// `containers` directory of the driver to this user. The root of a
    /// new instance goes into that directory, so podvm reads its file
    /// system. An incusd in a mount namespace of its own (route A of
    /// `docs/incus.md`) mounts a pool with a `source` where podvm does not
    /// see it: the mount point is then empty, and podvm gives no check.
    pub fn at(dir: &Path, name: &str) -> Option<Target> {
        if !segment(name) {
            return None;
        }
        let path = dir.join("storage-pools").join(name).join("containers");
        let md = std::fs::metadata(&path).ok().filter(|m| m.is_dir())?;
        let images = dir.join("images");
        let images = std::fs::metadata(&images).ok().map(|m| Spot {
            label: "incusd's images".to_string(),
            path: images,
            dev: m.dev(),
        });
        Some(Target {
            pool: Spot {
                label: format!("the storage pool `{name}`"),
                path,
                dev: md.dev(),
            },
            images,
        })
    }

    /// The target of the client: the pool of the `root` disk of the
    /// default profile, where each driver of the server is `dir`.
    pub async fn find(c: &pod_incus::Client, storage: &str) -> Option<Target> {
        if !dir_only(storage) {
            return None;
        }
        let profile = c.profile("default").await.ok()?;
        let name = profile.devices.get("root")?.get("pool")?.clone();
        Target::at(&incus_dir(&c.settings().socket)?, &name)
    }

    /// The loads of `need`, each with its free space now.
    fn measured(&self, need: Need, importing: bool) -> Vec<(Load, Room)> {
        loads(&self.pool, self.images.as_ref(), need, importing)
            .into_iter()
            .filter_map(|l| room_at(&l.spot.path).map(|r| (l, r)))
            .collect()
    }

    /// Before a create (and the import with `importing`): the refusal where
    /// the image cannot fit, else the warnings.
    pub fn check(&self, need: Need, importing: bool) -> Result<Vec<String>, String> {
        let mut warn = Vec::new();
        for (l, r) in self.measured(need, importing) {
            let v = judge(l.need, r);
            match (v, line(&l, r, v)) {
                (Verdict::NoBytes | Verdict::NoInodes, Some(t)) => return Err(t),
                (_, Some(t)) => warn.push(t),
                _ => {}
            }
        }
        Ok(warn)
    }

    /// The line after a call that failed with `transport`: the free space
    /// and the need of each load. A refusal by size gives it each time;
    /// another fault of the transport where a load does not fit.
    pub fn after(
        &self,
        need: Need,
        importing: bool,
        transport: Transport,
        bound: usize,
    ) -> Option<String> {
        let m = self.measured(need, importing);
        let short = m.iter().any(|(l, r)| judge(l.need, *r) != Verdict::Fits);
        let rooms = m
            .iter()
            .map(|(l, r)| {
                let inodes = r
                    .inodes
                    .map(|i| format!(" and {i} free inodes"))
                    .unwrap_or_default();
                format!(
                    "{} at {} has {} free{inodes} now, and {} needs at least {} and {} inodes",
                    l.spot.label,
                    l.spot.path.display(),
                    mib(r.bytes),
                    l.what,
                    mib(l.need.bytes),
                    l.need.entries
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        if rooms.is_empty() {
            return None;
        }
        match transport {
            Transport::TooLarge => Some(format!(
                "the answer of incusd was over podvm's bound of {}: where a pool fills, \
                 incusd's error holds a line of tar for each file that it could not write \
                 (measured), and incusd then removes what it wrote; {rooms} \
                 (TODO/machine.md T-2078, PodUtils/podrunner#346)",
                mib(bound as u64)
            )),
            Transport::Other if short => Some(format!(
                "{rooms}: a lack of space can cut the answer of incusd \
                 (TODO/machine.md T-2078)"
            )),
            _ => None,
        }
    }
}

/// The class of a failed call in its transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// Not a fault of the transport: incusd answered.
    None,
    /// The answer was over the bound of the client.
    TooLarge,
    /// Another read or write fault of the connection.
    Other,
}

/// The class of `e`. pod-incus gives the refusal by size as an I/O error of
/// the kind `InvalidData` with the text of pod-proxy (podcrates `v0.82.0`);
/// T-2078 asks podcrates for a kind of its own.
pub fn transport(e: &pod_incus::Error) -> Transport {
    match e.kind() {
        pod_incus::ErrorKind::Io(io)
            if io.kind() == std::io::ErrorKind::InvalidData
                && io.to_string().contains("larger than allowed") =>
        {
            Transport::TooLarge
        }
        pod_incus::ErrorKind::Io(_) => Transport::Other,
        _ => Transport::None,
    }
}

/// The line after a create or an import that failed with `transport`, for
/// the root at `rootfs`: the target of the server, measured now.
pub async fn after_failure(
    c: &pod_incus::Client,
    rootfs: &Path,
    importing: bool,
    transport: Transport,
) -> Option<String> {
    if transport == Transport::None {
        return None;
    }
    let server = c.server().await.ok()?;
    let target = Target::find(c, &server.environment.storage).await?;
    let podvm = std::fs::metadata("/proc/self/exe").map_or(0, |m| m.len());
    target.after(
        need_of(rootfs, podvm),
        importing,
        transport,
        c.settings().max_body,
    )
}
