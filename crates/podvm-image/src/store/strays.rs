//! PodUtils/podvm#527: the blobs that no record names.
//!
//! A pull commits each blob before it writes its record. A pull that a
//! signal stops between the two leaves blobs that `images`, `verify`,
//! `image prune` and `system df` did not see. A blob of a writer that is
//! alive is no stray: its record comes after it.
//!
//! ⛔ Two rules, because one of them can be absent:
//!
//! 1. Each writer (`pull`, `load`, `import`) holds [`Store::writing`], a
//!    SHARED lock on `writers.lock`, until its record is written. A sweep
//!    takes it EXCLUSIVE with no wait, under the index lock, and removes
//!    nothing where a writer is alive.
//! 2. A sweep keeps each blob written less than [`STAGING_AGE`] ago, also
//!    where no writer holds the lock: a new writer that does not take it
//!    keeps its blobs for that long. `system df` counts such a blob; `image
//!    prune` names it as kept.

use super::*;

const WRITERS_LOCK: &str = "writers.lock";

/// What a sweep of the strays found.
#[derive(Debug, Default)]
pub struct Strays {
    /// Each blob that no record names, with its bytes.
    pub blobs: Vec<(String, u64)>,
    /// The blobs that no record names and that the sweep kept: young ones.
    pub young: usize,
    /// True where a writer was alive, so the sweep looked at no blob.
    pub writer_alive: bool,
}

impl Strays {
    pub fn bytes(&self) -> u64 {
        self.blobs.iter().map(|(_, b)| *b).sum()
    }
}

impl Store {
    /// The shared lock of a writer: hold it from the first blob to the
    /// record (PodUtils/podvm#527).
    pub fn writing(&self) -> Result<Lock> {
        lock_at(&self.root.join(WRITERS_LOCK), sys::LOCK_SH)
    }

    /// The blobs that no record names. With `remove`, unlink each one,
    /// where no writer is alive; the list then holds what was removed.
    pub fn strays(&self, remove: bool) -> Result<Strays> {
        let _guard = self.lock()?;
        let Some(_quiet) = try_lock_at(&self.root.join(WRITERS_LOCK), sys::LOCK_EX)? else {
            return Ok(Strays {
                blobs: Vec::new(),
                writer_alive: true,
                ..Strays::default()
            });
        };
        let index = self.read_index()?;
        let mut named: Vec<&str> = index.images.iter().flat_map(|r| r.blobs()).collect();
        named.sort_unstable();
        let dir = self.root.join(BLOBS).join(crate::digest::SHA256);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Ok(Strays::default());
        };
        let mut found = Strays::default();
        for e in entries.flatten() {
            let Some(hex) = e.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let digest = format!("{}:{hex}", crate::digest::SHA256);
            // A name that is no digest is no blob of podvm: keep it.
            let Ok(d) = Digest::parse(&digest) else {
                continue;
            };
            if named.binary_search(&digest.as_str()).is_ok() {
                continue;
            }
            let Ok(meta) = e.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let young = !super::blobs::abandoned_by_age(&e.path(), super::blobs::STAGING_AGE);
            if remove && young {
                found.young += 1;
                continue;
            }
            if remove {
                let resolved = contain::within(&self.root, &self.blob_path(&d))?;
                if std::fs::remove_file(&resolved).is_err() {
                    continue;
                }
            }
            found.blobs.push((digest, meta.len()));
        }
        Ok(found)
    }
}
