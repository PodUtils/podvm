//! Where incusd writes an image, and its free space, TODO/machine.md T-2078
//! (PodUtils/podrunner#346).
//!
//! The pool of the `root` disk of the default profile takes the root of a
//! new instance; incusd's images take the upload. podvm reads the free
//! space of a pool in one of two ways:
//!
//! 1. `statfs` of `INCUS_DIR/storage-pools/NAME/containers`, where each
//!    driver of the server is `dir` and podvm sees that directory. podvm
//!    then knows the device of the file system, so a load of the images on
//!    the same file system adds up, and the store's tree that an import
//!    removes counts as room.
//! 2. Else `GET /1.0/storage-pools/NAME/resources` (podcrates `v0.83.0`,
//!    `Client::storage_pool_resources`): incusd measures the pool in its own
//!    mount namespace, for each driver, also a `source` that podvm does not
//!    see (route A of `docs/incus.md`). incusd gives the free blocks of root
//!    (`f_bfree`), not those of a user: incusd writes as root. podvm does not
//!    know the device there; [`Dev::of`] gives what the driver and its
//!    `source` tell.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use super::space::{
    dir_only, judge, line, loads, segment, Dev, Load, Plan, Room, Spot, Transport, Verdict,
};
use pod_hostprobe::space::mib;

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

/// The spot of incusd's images in the incusd directory `dir`, where podvm
/// sees it.
fn images_at(dir: &Path) -> Option<Spot> {
    let images = dir.join("images");
    std::fs::metadata(&images).ok().map(|m| Spot {
        label: "incusd's images".to_string(),
        path: images,
        dev: Dev::Is(m.dev()),
        clones: false,
        room: None,
    })
}

/// The pool of the root disk of new instances and incusd's images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub pool: Spot,
    pub images: Option<Spot>,
}

impl Target {
    /// The target in the incusd directory `dir` for the pool `name` of the
    /// driver `dir`; `None` where the name is not one segment or the pool
    /// does not show the `containers` directory of the driver to this
    /// user. The root of a new instance goes into that directory, so podvm
    /// reads its file system. An incusd in a mount namespace of its own
    /// (route A of `docs/incus.md`) mounts a pool with a `source` where
    /// podvm does not see it: the mount point is then empty, and
    /// [`Target::find`] asks incusd.
    pub fn at(dir: &Path, name: &str) -> Option<Target> {
        if !segment(name) {
            return None;
        }
        let path = dir.join("storage-pools").join(name).join("containers");
        let md = std::fs::metadata(&path).ok().filter(|m| m.is_dir())?;
        Some(Target {
            pool: Spot {
                label: format!("the storage pool `{name}`"),
                path,
                dev: Dev::Is(md.dev()),
                clones: false,
                room: None,
            },
            images: images_at(dir),
        })
    }

    /// The target of the pool `pool` with the free space `room` that
    /// incusd measured; `None` where incusd gives no total (a driver that
    /// does not measure its pool).
    pub fn of_incusd(
        pool: &pod_incus::StoragePool,
        res: &pod_incus::PoolResources,
        dir: Option<&Path>,
    ) -> Option<Target> {
        if !segment(&pool.name) || res.space.total == 0 {
            return None;
        }
        let source = pool.source().unwrap_or_default();
        Some(Target {
            pool: Spot {
                label: format!(
                    "the storage pool `{}` (driver `{}`, as incusd measures it)",
                    pool.name, pool.driver
                ),
                path: PathBuf::from(source),
                dev: Dev::of(&pool.driver, source),
                clones: super::space::clones(&pool.driver, &pool.config),
                room: Some(Room {
                    bytes: res.free_bytes(),
                    inodes: res.free_inodes(),
                }),
            },
            images: dir.and_then(images_at),
        })
    }

    /// The target of the client: the pool of the `root` disk of the
    /// default profile. `statfs` where each driver of the server is `dir`
    /// and podvm sees the pool, else the resources that incusd gives.
    pub async fn find(c: &pod_incus::Client, storage: &str) -> Option<Target> {
        let profile = c.profile("default").await.ok()?;
        let name = profile.devices.get("root")?.get("pool")?.clone();
        if !segment(&name) {
            return None;
        }
        let dir = incus_dir(&c.settings().socket);
        if dir_only(storage)
            && let Some(t) = dir.as_deref().and_then(|d| Target::at(d, &name))
        {
            return Some(t);
        }
        let pool = c.storage_pool(&name).await.ok()?;
        let res = c.storage_pool_resources(&name).await.ok()?;
        Target::of_incusd(&pool, &res, dir.as_deref())
    }

    /// The loads of `plan`, each with its free space now.
    fn measured(&self, plan: &Plan, importing: bool) -> Vec<(Load, Room)> {
        loads(&self.pool, self.images.as_ref(), plan, importing)
            .into_iter()
            .filter_map(|l| {
                l.spot
                    .room
                    .or_else(|| room_at(&l.spot.path))
                    .map(|r| (l, r))
            })
            .collect()
    }

    /// Before a create (and the import with `importing`): the refusal where
    /// the image cannot fit, else the warnings.
    pub fn check(&self, plan: &Plan, importing: bool) -> Result<Vec<String>, String> {
        let mut warn = Vec::new();
        for (l, r) in self.measured(plan, importing) {
            let v = judge(l.need, l.extra, r);
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
    /// another fault of the transport where a load does not fit. With no
    /// plan (T-2085: no tree in the store, as incus had the image), the
    /// need is a dash.
    pub fn after(
        &self,
        plan: Option<&Plan>,
        importing: bool,
        transport: Transport,
        bound: usize,
    ) -> Option<String> {
        let m = self.measured(&plan.copied().unwrap_or_default(), importing);
        let short = plan.is_some()
            && m.iter()
                .any(|(l, r)| judge(l.need, l.extra, *r) != Verdict::Fits);
        let rooms = m
            .iter()
            .map(|(l, r)| {
                let inodes = r
                    .inodes
                    .map(|i| format!(" and {i} free inodes"))
                    .unwrap_or_default();
                let need = match plan {
                    Some(_) => format!(
                        "at least {} and {} inodes",
                        mib(l.need.bytes),
                        l.need.entries
                    ),
                    None => "- (podvm has no tree of the image)".to_string(),
                };
                format!(
                    "{} has {} free{inodes} now, and {} needs {need}",
                    l.spot.place(),
                    mib(r.bytes),
                    l.what,
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
            Transport::NoSpace => Some(format!(
                "incusd ran out of space: {rooms} (TODO/machine.md T-2078, \
                 PodUtils/podrunner#346)"
            )),
            Transport::Other if short => Some(format!(
                "{rooms}: a lack of space can cut the answer of incusd \
                 (TODO/machine.md T-2078)"
            )),
            _ => None,
        }
    }
}

/// The line after a create or an import that failed with `transport`, for
/// `plan` (`None`: no tree): the target of the server, measured now.
pub async fn after_failure(
    c: &pod_incus::Client,
    plan: Option<&Plan>,
    importing: bool,
    transport: Transport,
) -> Option<String> {
    if transport == Transport::None {
        return None;
    }
    let server = c.server().await.ok()?;
    let target = Target::find(c, &server.environment.storage).await?;
    target.after(plan, importing, transport, c.settings().max_body)
}
