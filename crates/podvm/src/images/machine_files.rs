//! TODO/machine.md T-1994 (2026-11-26): the files of the machine tier that
//! belong to one image, and their removal with the image.
//!
//! `store/machine/` holds, for the manifest digest `HEX` of an image:
//!
//! * `base-HEX-vN.cpio`, the base archive of the initramfs root;
//! * `root-HEX-vN.img`, its `.ok` record and its `.lock`: the read-only
//!   root disk (T-1994);
//! * `state-HEX-KEY.*`: the saved states of a guest (T-2008), with a lock.
//!
//! A file of the same group has the same name up to the first dot (the
//! digits of a digest hold no dot), so the group is the unit: a build or a
//! save holds the lock of its group, and its partial files carry the stem.
//! The kernel, the emulator and the `run-*` directories are no image's, and
//! no name here matches them.
//!
//! The rules:
//!
//! * a group goes only where its image has no record any more
//!   ([`drop_for`]: the digests of the removed records, and each digest that
//!   no record names, [`orphans`]). A second name or a container that keeps
//!   the image keeps its record, so it keeps these files;
//! * a group with a lock goes under the lock of the group, taken without a
//!   wait. A lock that another run holds (a build, a save) keeps the group,
//!   and the line names it;
//! * a file that cannot go is named with its error. No file outside the
//!   `machine/` directory is touched, and only a regular file is removed.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use podvm_image::store::Lock;

const KINDS: [&str; 3] = ["base-", "root-", "state-"];

/// The hex digits of a `sha256:` digest: 64 of them, or none (a name is
/// never made from another form).
fn hex_of(digest: &str) -> Option<&str> {
    digest
        .strip_prefix("sha256:")
        .filter(|h| h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

/// The digits of the digest that a name of the machine directory holds,
/// and the stem of its group.
fn parts(name: &str) -> Option<(&str, &str)> {
    let kind = KINDS.iter().find(|k| name.starts_with(**k))?;
    let rest = &name[kind.len()..];
    let hex = rest
        .get(..64)
        .filter(|h| hex_of(&format!("sha256:{h}")).is_some())?;
    // The digits end the digest part: a `-` follows them.
    rest[64..].starts_with('-').then_some(())?;
    let stem = name.split('.').next().unwrap_or(name);
    Some((hex, stem))
}

/// A group: its stem and its files.
struct Group {
    stem: String,
    files: Vec<PathBuf>,
}

/// Each group of the machine directory with the digits `want` (any digits
/// where `want` is `None`), by the digits.
fn groups(store_root: &Path, want: Option<&str>) -> Vec<(String, Group)> {
    let Ok(rd) = std::fs::read_dir(store_root.join("machine")) else {
        return Vec::new();
    };
    let mut out: Vec<(String, Group)> = Vec::new();
    for e in rd.flatten() {
        let Some(name) = e.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Some((hex, stem)) = parts(&name) else {
            continue;
        };
        if want.is_some_and(|w| w != hex) {
            continue;
        }
        match out.iter_mut().find(|(_, g)| g.stem == stem) {
            Some((_, g)) => g.files.push(e.path()),
            None => out.push((
                hex.to_string(),
                Group {
                    stem: stem.to_string(),
                    files: vec![e.path()],
                },
            )),
        }
    }
    out
}

fn size(p: &Path) -> u64 {
    match std::fs::symlink_metadata(p) {
        Ok(m) if m.is_file() => m.size(),
        _ => 0,
    }
}

/// The bytes of the machine files of each digest of `digests`.
pub(crate) fn bytes_of(store_root: &Path, digests: &[String]) -> u64 {
    let mut seen: Vec<&str> = Vec::new();
    let mut total = 0u64;
    for d in digests {
        let Some(hex) = hex_of(d).filter(|h| !seen.contains(h)) else {
            continue;
        };
        seen.push(hex);
        for (_, g) in groups(store_root, Some(hex)) {
            total = g.files.iter().fold(total, |t, f| t.saturating_add(size(f)));
        }
    }
    total
}

/// The bytes of all the machine files of images.
pub(crate) fn all_bytes(store_root: &Path) -> u64 {
    groups(store_root, None)
        .iter()
        .flat_map(|(_, g)| g.files.iter())
        .fold(0u64, |t, f| t.saturating_add(size(f)))
}

/// The digests (with their bytes) that machine files name and no record
/// of the store names, and no container references: what an `rmi` of an
/// older podvm left.
pub(crate) fn orphans(store: &podvm_image::Store) -> Vec<(String, u64)> {
    let records = store.list().unwrap_or_default();
    let mut seen: Vec<String> = Vec::new();
    for (hex, _) in groups(store.root(), None) {
        let digest = format!("sha256:{hex}");
        if seen.contains(&digest) || records.iter().any(|r| r.manifest_digest == digest) {
            continue;
        }
        // A container of a removed record still names the image: its files
        // stay until the container goes. A failed read keeps them too.
        if podvm_supervise::referencing(store, &digest).map_or(true, |c| !c.is_empty()) {
            continue;
        }
        seen.push(digest);
    }
    seen.into_iter()
        .map(|d| {
            let b = bytes_of(store.root(), std::slice::from_ref(&d));
            (d, b)
        })
        .collect()
}

/// What `rmi` or `image prune` of `rest` would remove: the bytes of each
/// digest of `rest` that no record outside `rest` names.
pub(crate) fn reclaimable(
    store_root: &Path,
    rest: &[podvm_image::Record],
    all: &[podvm_image::Record],
) -> u64 {
    let gone: Vec<String> = rest
        .iter()
        .filter(|r| {
            !all.iter().any(|o| {
                o.manifest_digest == r.manifest_digest
                    && !rest.iter().any(|d| {
                        d.repository == o.repository && d.tag == o.tag && d.digest == o.digest
                    })
            })
        })
        .map(|r| r.manifest_digest.clone())
        .collect();
    bytes_of(store_root, &gone)
}

/// Remove the machine files of `digest`, group by group. It gives the bytes
/// that went. A group that a lock holds, and a file that cannot go, are
/// named on stderr with `verb`.
pub(crate) fn remove(store_root: &Path, digest: &str, verb: &str) -> u64 {
    let Some(hex) = hex_of(digest) else {
        return 0;
    };
    let mut freed = 0u64;
    for (_, g) in groups(store_root, Some(hex)) {
        let lock_file = g
            .files
            .iter()
            .find(|f| f.file_name().and_then(|n| n.to_str()) == Some(&format!("{}.lock", g.stem)));
        let guard = match lock_file.map(|p| (p, Lock::try_exclusive(p))) {
            None => None,
            Some((_, Ok(Some(l)))) => Some(l),
            Some((p, Ok(None))) => {
                eprintln!(
                    "podvm {verb}: kept the machine files {}: a run holds {}",
                    g.stem,
                    p.display()
                );
                continue;
            }
            Some((p, Err(e))) => {
                eprintln!(
                    "podvm {verb}: kept the machine files {}: the lock {}: {e}",
                    g.stem,
                    p.display()
                );
                continue;
            }
        };
        // The lock file goes last, while the guard still holds it.
        let (locks, rest): (Vec<&PathBuf>, Vec<&PathBuf>) =
            g.files.iter().partition(|f| Some(*f) == lock_file);
        for f in rest.into_iter().chain(locks) {
            if !std::fs::symlink_metadata(f).is_ok_and(|m| m.is_file()) {
                continue;
            }
            let bytes = size(f);
            match std::fs::remove_file(f) {
                Ok(()) => freed = freed.saturating_add(bytes),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => eprintln!("podvm {verb}: {}: {e}", f.display()),
            }
        }
        drop(guard);
    }
    freed
}

/// The machine files of the digests of `before` that no record names now.
pub(crate) fn drop_for(
    store_root: &Path,
    before: &[String],
    now: &[podvm_image::Record],
    verb: &str,
) -> u64 {
    before
        .iter()
        .filter(|d| !now.iter().any(|r| &r.manifest_digest == *d))
        .fold(0u64, |t, d| t.saturating_add(remove(store_root, d, verb)))
}

#[cfg(test)]
#[path = "machine_files_tests.rs"]
mod tests;
