//! TODO/machine.md T-2103 (PodUtils/podrunner#371): the record of the id
//! maps of the instances that wrote in a host directory.
//!
//! An instance of incus writes in a writable volume (a `disk` device) as
//! the host ids of its map: `BASE + uid` of the instance. The host user
//! cannot remove what such an id made in a directory of mode 0755 or 0700.
//! The root of an instance with the same map can, and only there: the
//! ids of each other map stay out of its reach. So each run of the incus
//! tier with a writable volume records the map of its instance (incusd's
//! `volatile.idmap.next`, which it sets at the create) under the store,
//! one file for each host directory, and `podvm volume rm` ([`super::volume`])
//! starts one short instance with each recorded map.
//!
//! The file `<store>/incus/volumes/HASH.map` (HASH: the first 32 hex digits
//! of the SHA-256 of the host path) holds, one item to a line:
//!
//! ```text
//! v1
//! host /the/host/directory
//! map BASE SIZE
//! ```
//!
//! A map is one range of ids for the uids and the same range for the gids,
//! from id 0 of the instance: the form of each map that incusd gives a
//! container of podvm (isolated or not). A map of another form is not
//! recorded, and the run says so.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// The most bytes of a record that a reader takes.
const MAX: u64 = 64 * 1024;
/// The most maps of one record; a record that is full takes no new map.
pub const MAX_MAPS: usize = 256;
/// The most records that a removal reads.
const MAX_RECORDS: usize = 4096;
/// The time that a writer waits for the lock of a record.
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// One id map: the host id of id 0 of the instance, and the count of ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Map {
    pub base: u64,
    pub size: u64,
}

/// One record: a host directory and the maps that wrote in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub host: PathBuf,
    pub maps: Vec<Map>,
}

/// The directory of the records in `store`.
pub fn dir(store: &Path) -> PathBuf {
    store.join("incus").join("volumes")
}

/// The name of the record of `host`, and its lock key.
fn key(host: &Path) -> String {
    use sha2::Digest;
    use std::os::unix::ffi::OsStrExt;
    let h = sha2::Sha256::digest(host.as_os_str().as_bytes());
    h.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// The map of the JSON of `volatile.idmap.next`, or the reason that it is
/// none of the form of a record.
pub fn of_json(text: &str) -> Result<Map, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
    let entries = v.as_array().ok_or("not a list")?;
    let (mut uid, mut gid) = (None, None);
    for e in entries {
        let n = |k: &str| e.get(k).and_then(Value::as_u64);
        let b = |k: &str| e.get(k).and_then(Value::as_bool).unwrap_or(false);
        let (Some(host), Some(0), Some(size)) = (n("Hostid"), n("Nsid"), n("Maprange")) else {
            return Err(format!("an entry that does not map from id 0: {e}"));
        };
        let m = Map { base: host, size };
        for (on, slot) in [(b("Isuid"), &mut uid), (b("Isgid"), &mut gid)] {
            if on && slot.replace(m).is_some() {
                return Err("two entries of one kind".to_string());
            }
        }
    }
    match (uid, gid) {
        (Some(u), Some(g)) if u == g && u.size > 0 => Ok(u),
        (Some(_), Some(_)) => Err("the uid and gid ranges differ".to_string()),
        _ => Err("no uid range or no gid range".to_string()),
    }
}

/// The text of `r`.
pub fn text(r: &Record) -> Result<String, String> {
    let host = r
        .host
        .to_str()
        .filter(|h| h.starts_with('/') && !h.contains('\n'))
        .ok_or_else(|| format!("the host path {:?} is not UTF-8 on one line", r.host))?;
    let mut s = format!("v1\nhost {host}\n");
    for m in &r.maps {
        s.push_str(&format!("map {} {}\n", m.base, m.size));
    }
    Ok(s)
}

/// The record of `text`, or the reason that it is none.
pub fn parse(text: &str) -> Result<Record, String> {
    let mut lines = text.lines();
    if lines.next() != Some("v1") {
        return Err("no version line `v1`".to_string());
    }
    let host = lines
        .next()
        .and_then(|l| l.strip_prefix("host "))
        .filter(|h| h.starts_with('/'))
        .ok_or("no line `host /PATH`")?;
    let mut maps = Vec::new();
    for l in lines {
        let w: Vec<&str> = l.split(' ').collect();
        let m = match w.as_slice() {
            ["map", b, s] => b.parse().ok().zip(s.parse().ok()),
            _ => None,
        };
        match m {
            Some((base, size)) if size > 0 && maps.len() < MAX_MAPS => {
                maps.push(Map { base, size })
            }
            _ => return Err(format!("the line {l:?} is not `map BASE SIZE`")),
        }
    }
    Ok(Record {
        host: PathBuf::from(host),
        maps,
    })
}

/// Read one record file, bounded.
fn read(p: &Path) -> Result<Record, String> {
    use std::io::Read;
    let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut s = String::new();
    f.take(MAX + 1)
        .read_to_string(&mut s)
        .map_err(|e| format!("{}: {e}", p.display()))?;
    if s.len() as u64 > MAX {
        return Err(format!("{}: over {MAX} bytes", p.display()));
    }
    parse(&s).map_err(|e| format!("{}: {e}", p.display()))
}

/// The lock of the record of `host`, waited for at most [`LOCK_WAIT`].
fn lock(store: &podvm_image::Store, host: &Path) -> Result<podvm_image::store::Lock, String> {
    let k = format!("incus-volume-{}", key(host));
    let start = std::time::Instant::now();
    loop {
        match store.try_lock_key(&k) {
            Ok(Some(l)) => return Ok(l),
            Ok(None) if start.elapsed() < LOCK_WAIT => {
                std::thread::sleep(std::time::Duration::from_millis(50))
            }
            Ok(None) => return Err(format!("another podvm held the lock {k} for 10 s")),
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// Add `map` to the record of `host` (a canonical path) under the lock.
pub fn add(store: &podvm_image::Store, host: &Path, map: Map) -> Result<(), String> {
    let _l = lock(store, host)?;
    let d = dir(store.root());
    std::fs::create_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
    let p = d.join(format!("{}.map", key(host)));
    let mut r = match read(&p) {
        Ok(r) if r.host == host => r,
        Ok(r) => return Err(format!("{} names {:?}", p.display(), r.host)),
        Err(_) if !p.exists() => Record {
            host: host.to_path_buf(),
            maps: Vec::new(),
        },
        Err(e) => return Err(e),
    };
    if r.maps.contains(&map) {
        return Ok(());
    }
    if r.maps.len() >= MAX_MAPS {
        return Err(format!("{} holds {MAX_MAPS} maps, the most", p.display()));
    }
    r.maps.push(map);
    pod_core::files::replace(
        &p,
        text(&r)?.as_bytes(),
        pod_core::files::Mode::Exact(0o600),
    )
    .map(drop)
    .map_err(|e| format!("{}: {e}", p.display()))
}

/// Whether `a` is `b` or a directory under it, by components.
pub fn at_or_under(a: &Path, b: &Path) -> bool {
    a.starts_with(b)
}

/// The records of `store` whose directory is `host`, under it or above
/// it: the maps that can own an entry under `host`. Each file that is no
/// record is an error by name: a map that podvm cannot read is a removal
/// that cannot be whole.
pub fn related(store: &Path, host: &Path) -> Result<Vec<(PathBuf, Record)>, String> {
    let d = dir(store);
    let it = match std::fs::read_dir(&d) {
        Ok(it) => it,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", d.display())),
    };
    let mut out = Vec::new();
    for (n, e) in it.enumerate() {
        if n >= MAX_RECORDS {
            return Err(format!(
                "{} holds more than {MAX_RECORDS} files",
                d.display()
            ));
        }
        let p = e.map_err(|e| format!("{}: {e}", d.display()))?.path();
        if p.extension().is_none_or(|x| x != "map") {
            continue;
        }
        let r = read(&p)?;
        if at_or_under(&r.host, host) || at_or_under(host, &r.host) {
            out.push((p, r));
        }
    }
    Ok(out)
}

/// The maps of `records`, each once, in order.
pub fn maps(records: &[(PathBuf, Record)]) -> Vec<Map> {
    let mut m: Vec<Map> = records.iter().flat_map(|(_, r)| r.maps.clone()).collect();
    m.sort();
    m.dedup();
    m
}

/// After a run creates the instance `inst` with `volumes`: the map of the
/// instance goes to the record of each writable volume. A failure is one
/// line: the run goes on, and `podvm volume rm` of that directory then
/// names the missing record.
pub async fn note(
    c: &pod_incus::Client,
    store: Option<&podvm_image::Store>,
    inst: &str,
    volumes: &[(String, String, bool)],
    verb: &str,
) {
    let writable: Vec<&String> = volumes.iter().filter(|v| !v.2).map(|v| &v.0).collect();
    let (Some(store), false) = (store, writable.is_empty()) else {
        return;
    };
    let map = match c.instance(inst).await {
        Ok(i) => i
            .config
            .get("volatile.idmap.next")
            .ok_or_else(|| "incusd gives no volatile.idmap.next".to_string())
            .and_then(|t| of_json(t)),
        Err(e) => Err(e.to_string()),
    };
    for host in writable {
        if let Err(e) = map.clone().and_then(|m| add(store, Path::new(host), m)) {
            eprintln!(
                "podvm {verb}: incus tier: the id map of the instance {inst} is not recorded \
                 for {host}: {e}; `podvm volume rm` cannot remove what the instance writes \
                 there (TODO/machine.md T-2103)"
            );
        }
    }
}

/// After a removal of `host`: the records of `host` and of each directory
/// under it go, under their locks. A record of a directory above `host`
/// stays: its other entries can still hold what its maps wrote.
pub fn forget(store: &podvm_image::Store, host: &Path, records: &[(PathBuf, Record)]) {
    for (p, r) in records.iter().filter(|(_, r)| at_or_under(&r.host, host)) {
        let removed = lock(store, &r.host)
            .and_then(|_l| std::fs::remove_file(p).map_err(|e| format!("{}: {e}", p.display())));
        if let Err(e) = removed {
            eprintln!("podvm volume rm: the record {} stays: {e}", p.display());
        }
    }
}
