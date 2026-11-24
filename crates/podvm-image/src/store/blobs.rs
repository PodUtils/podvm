use super::*;

impl Store {
    /// A staging file inside the store, for a blob whose digest is not yet
    /// proved. ⚠ Inside the store because the commit is a `rename(2)`, and a
    /// rename across filesystems fails with `EXDEV`.
    ///
    /// ⛔ **Invariant I6: unique per CALL.** The name carried only the pid until
    /// 2026-09-09, and `TODO/probe.md` T-0113 is that same defect one crate
    /// over: two threads of one process take one name and overwrite each other.
    /// `cargo test` runs tests in threads, and [T-0210](../../../TODO/image.md)'s
    /// own bounded-concurrency sibling would make it reachable in production.
    ///
    /// ⛔ **Invariant I5: the writer holds it.** The returned handle carries an
    /// exclusive `flock` for as long as it lives, which is what lets
    /// [`Store::sweep_staging`] tell an abandoned file from one being written
    /// without asking about a pid.
    pub fn stage(&self, hint: &str) -> Result<(PathBuf, StagedFile)> {
        let safe: String = hint
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .take(24)
            .collect();
        let path = self.root.join(STAGING).join(format!(
            "{safe}.{}.{}.partial",
            std::process::id(),
            STAGE_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let f = StagedFile::create(&path)?;
        Ok((path, f))
    }

    /// ⛔ **Invariant I5.** Remove every abandoned staging file, deciding what
    /// is abandoned by trying to lock it.
    ///
    /// ⚠ Returns what it removed rather than printing: the caller decides
    /// whether a sweep is worth a line, and `Store::open` runs on every command.
    /// ⛔ Only inside this store's own `staging/`, resolved through
    /// [`crate::contain`], because a sweep is an unlink and every unlink this
    /// crate performs is gated the same way.
    pub fn sweep_staging(&self) -> Vec<PathBuf> {
        self.sweep_staging_aged(MEMO_AGE)
    }

    /// [`Store::sweep_staging`] with the age of an unlocked memo of a run:
    /// `container prune` asks for none (TODO/cli.md T-2063).
    pub fn sweep_staging_aged(&self, memo_age: std::time::Duration) -> Vec<PathBuf> {
        let dir = self.root.join(STAGING);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut swept = Vec::new();
        for e in entries.flatten() {
            let path = e.path();
            // TODO/cli.md T-2063 (OQ-12): the memo of a foreground run too.
            let memo = is_memo(&path);
            if !memo && path.extension().and_then(|x| x.to_str()) != Some("partial") {
                continue;
            }
            let Ok(resolved) = contain::within(&self.root, &path) else {
                continue;
            };
            // PodUtils/podvm#472, #501: the table of containers is written
            // to a `.partial` with no lock and renamed (pod-supervise), so a
            // sweep of another process deleted it before its rename, and the
            // launcher lost the end of a run (`Dead`, no exit code), or `run`
            // failed with "containers.json: No such file or directory". A
            // young file is a write in progress, locked or not.
            if !abandoned_by_age(&resolved, if memo { memo_age } else { STAGING_AGE }) {
                continue;
            }
            // ⭐ The lock test: a live writer holds this file, so a lock this
            // process can take means nobody is writing it.
            match try_lock_at(&resolved, sys::LOCK_EX) {
                Ok(Some(_held)) => {
                    if std::fs::remove_file(&resolved).is_ok() {
                        swept.push(resolved);
                    }
                }
                _ => continue,
            }
        }
        swept
    }

    /// Name a staged file by its proved digest. ⛔ The caller has already
    /// verified it: [`crate::digest::Verifier`] is what produces the digest
    /// passed here, so a blob that failed verification has no path to this
    /// function.
    pub fn commit(&self, staged: &Path, d: &Digest) -> Result<()> {
        let dest = self.blob_path(d);
        contain::within(&self.root, &dest)?;
        std::fs::rename(staged, &dest).map_err(|e| Error::io(dest.display().to_string(), e))
    }

    /// A small blob already in memory, verified before it is written.
    pub fn put_bytes(&self, bytes: &[u8], want: &Digest, what: &str) -> Result<()> {
        let got = Digest::of(bytes);
        if got != *want {
            return Err(Error::DigestMismatch {
                what: what.to_string(),
                want: want.to_string(),
                got: got.to_string(),
            });
        }
        let (staged, mut f) = self.stage(what)?;
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|e| Error::io(staged.display().to_string(), e))?;
        drop(f);
        self.commit(&staged, want)
    }

    pub fn read_blob(&self, d: &Digest) -> Result<Vec<u8>> {
        let p = self.blob_path(d);
        let bytes = std::fs::read(&p).map_err(|e| Error::io(p.display().to_string(), e))?;
        // ⛔ Re-verified on the way out. `docs/conventions/code.md`: a guard
        // re-checking an invariant the happy path already knows earns its keep
        // the one time it fires, and here it fires when something outside
        // podvm has edited the store.
        let got = Digest::of(&bytes);
        if got != *d {
            return Err(Error::DigestMismatch {
                what: format!("stored blob {}", p.display()),
                want: d.to_string(),
                got: got.to_string(),
            });
        }
        Ok(bytes)
    }

    // ------------------------------------------------------------- the locks

    /// The store-wide lock, held across an index read-modify-write.
    pub fn lock(&self) -> Result<Lock> {
        lock_at(&self.root.join(STORE_LOCK), sys::LOCK_EX)
    }

    pub(super) fn image_lock_path(&self, record: &Record) -> Result<PathBuf> {
        let d = Digest::parse(&record.digest)?;
        Ok(self.root.join(LOCKS).join(format!("{}.lock", d.hex())))
    }

    /// ⭐ T-0204's mechanism. A **shared** advisory lock, released only when the
    /// last holder's fd is closed, however that process ends. Several
    /// containers may share one image, so the lock is shared; the GC asks for
    /// an exclusive one and is refused while any holder remains.
    ///
    /// ⛔ Two defences, because they cover different failures, and
    /// [`TODO/image.md`](../../../TODO/image.md) T-0211 is what having neither
    /// cost. `O_CLOEXEC` keeps the lock out of an unrelated **exec**, and
    /// registering the fd with [`sys::close_in_children`] keeps it out of an
    /// unrelated **fork**, which `O_CLOEXEC` cannot do, because there is no
    /// close-on-fork and `flock` is held on the open file description a fork
    /// duplicates. The one caller that wants a payload to inherit it says so
    /// with [`Lock::hand_to_payload`], which undoes both.
    ///
    /// ⚠ Both defences come from [`Lock`] itself now, and neither is this
    /// function's to take: `Lock::open` sets `O_CLOEXEC` and
    /// [`Lock::try_acquire`] registers. T-0215 is why this one is no longer
    /// special.
    pub fn hold(&self, record: &Record) -> Result<Lock> {
        let path = self.image_lock_path(record)?;
        // ⛔ INVARIANT I4 AND I7. The index lock is taken first and dropped at
        // the end of this function, so a `prune` cannot be between its own
        // `in_use` check and its unlink while this hold is being taken. Without
        // it the check is stale before it is used: measured as a reading of the
        // code on 2026-09-09, `prune` asked `in_use`, a `run` took its hold, and
        // `prune` then deleted the blobs the run was about to execute out of
        // AND the lock file it was holding. Both take index-then-image, so
        // neither can wait on the other.
        let _index = self.lock()?;
        lock_at(&path, sys::LOCK_SH)
    }

    /// The lock of a *fill*: the spare tree a detached `podvm tree-prefill`
    /// copies (TODO/image.md T-1781).
    ///
    /// ⛔ A second file, because the two holders mean opposite things to the
    /// GC. A payload hold must refuse a removal; a fill hold must not. The
    /// fill reads an extraction that is already on disk and writes a cache,
    /// so losing its source costs one copy and nothing else: `fill` removes
    /// its own partial tree when a read fails, and `sweep` removes the partial
    /// of a process that is gone. Measured before this file existed: `rm`
    /// starts the fill, the fill took the payload hold, and an `rmi` in the
    /// next second was refused with "in use by a running container" while
    /// `podvm ps -a` was empty, 4 times in 8.
    pub(super) fn image_fill_lock_path(&self, record: &Record) -> Result<PathBuf> {
        let d = Digest::parse(&record.digest)?;
        Ok(self.root.join(LOCKS).join(format!("{}.fill.lock", d.hex())))
    }

    /// Hold the *fill* of this image: a shared lock on its own file, which
    /// [`Store::in_use`] does not ask about, so a removal goes ahead and this
    /// copy then fails and cleans up after itself.
    pub fn hold_fill(&self, record: &Record) -> Result<Lock> {
        let path = self.image_fill_lock_path(record)?;
        // ⛔ I4, as in `hold`: the index lock is taken first and dropped here,
        // so a removal cannot remove the file between its creation and this
        // hold.
        let _index = self.lock()?;
        lock_at(&path, sys::LOCK_SH)
    }

    /// The lock file of an image's **first extraction**, keyed by the manifest
    /// digest: that digest names the tree, and two tags that resolve to one
    /// manifest share the tree and must share the lock.
    pub(super) fn image_extract_lock_path(&self, record: &Record) -> Result<PathBuf> {
        let d = Digest::parse(&record.manifest_digest)?;
        Ok(self
            .root
            .join(LOCKS)
            .join(format!("{}.extract.lock", d.hex())))
    }

    /// ⭐ The **exclusive** lock of one image's extraction.
    ///
    /// ⛔ Two callers that both read [`Store::hold`]'s image as unextracted
    /// both write the same tree. Measured 2026-11-14, eight `podvm create` at
    /// once of an image that had never been extracted: `v0.2.0-beta.16`
    /// created 12 of 48 containers over six rounds and lost all eight in four
    /// of them. The failures were `EEXIST` of a symlink of a layer, a copy of
    /// a tree another process had removed, or a payload that "names no file
    /// in the image". A caller takes this lock, asks
    /// [`podvm_extract::is_extracted`] again, and extracts only when the tree
    /// is still absent.
    ///
    /// ⚠ The wait is bounded and it does not block: `LOCK_NB` in a loop, as
    /// `RULES.md` section 8 requires. The bound is longer than `lock_at`'s
    /// five seconds, because an extraction of a large image runs for minutes.
    ///
    /// ⛔ The index lock is **not** taken here. It would be held across the
    /// wait, and the process that holds this lock needs the index to record
    /// the container that its extraction is for: both would wait on the other.
    /// The file cannot be unlinked under the holder either, because every
    /// caller takes [`Store::hold`] first and a removal asks for that lock
    /// exclusively.
    pub fn hold_extract(&self, record: &Record) -> Result<Lock> {
        let path = self.image_extract_lock_path(record)?;
        let started = std::time::Instant::now();
        loop {
            if let Some(l) = try_lock_at(&path, sys::LOCK_EX)? {
                return Ok(l);
            }
            if started.elapsed() >= EXTRACT_WAIT {
                return Err(Error::Store(format!(
                    "{} is being extracted by another process after {:?}. The wait has a \
                     bound and does not block: try again when that process ends",
                    record.name(),
                    EXTRACT_WAIT
                )));
            }
            std::thread::sleep(EXTRACT_PAUSE);
        }
    }

    /// Whether any process holds [`Store::hold`] on this image.
    ///
    /// ⚠ Tested by asking for the exclusive lock and reading `EWOULDBLOCK`,
    /// then dropping it immediately. There is no way to ask "is this locked"
    /// without trying, and a pid file that could be asked is the stale-state
    /// race this replaces.
    pub fn in_use(&self, record: &Record) -> Result<bool> {
        let path = self.image_lock_path(record)?;
        if !path.exists() {
            return Ok(false);
        }
        match try_lock_at(&path, sys::LOCK_EX) {
            Ok(Some(_probe)) => Ok(false),
            Ok(None) => Ok(true),
            Err(e) => Err(e),
        }
    }

    /// TODO/machine.md T-2085 (PodUtils/podrunner#347 ask 1): the locks of
    /// a removal of the tree of `manifest_digest` where no other process
    /// uses it: the exclusive [`Store::hold`] of each record that names
    /// that tree, and its exclusive extraction lock. `None` where another
    /// process holds one of them. The caller drops its own hold first: a
    /// shared lock of this process on another descriptor refuses the
    /// exclusive one too.
    ///
    /// ⛔ I4: the index lock is taken first and dropped at the end, as in
    /// [`Store::hold`], so no hold starts between the check of the records
    /// and the locks.
    pub fn try_hold_tree_alone(&self, manifest_digest: &str) -> Result<Option<Vec<Lock>>> {
        let _index = self.lock()?;
        let mut held = Vec::new();
        let named: Vec<Record> = self
            .list()?
            .into_iter()
            .filter(|r| r.manifest_digest == manifest_digest)
            .collect();
        let Some(first) = named.first() else {
            return Ok(Some(held));
        };
        for path in named
            .iter()
            .map(|r| self.image_lock_path(r))
            .chain(std::iter::once(self.image_extract_lock_path(first)))
        {
            match try_lock_at(&path?, sys::LOCK_EX)? {
                Some(l) => held.push(l),
                None => return Ok(None),
            }
        }
        Ok(Some(held))
    }
}

/// The age after which an unlocked staging file is abandoned.
pub const STAGING_AGE: std::time::Duration = std::time::Duration::from_secs(600);

/// The age after which an unlocked memo of a run is abandoned. A run locks
/// its memo when it makes it (`interpose::ephemeral_memo_path`), so this only
/// covers the moment between the file and its lock.
pub const MEMO_AGE: std::time::Duration = std::time::Duration::from_secs(10);

/// The name of the memo of a foreground run: `memo-<pid>-<nanos>.tmp`. The
/// pid is no proof of an owner (it is not unique across PID namespaces); the
/// lock on the file is.
pub fn is_memo(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with("memo-") && n.ends_with(".tmp"))
}

/// True where the file at `p` was last written `limit` or more ago.
pub(super) fn abandoned_by_age(p: &Path, limit: std::time::Duration) -> bool {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age >= limit)
}
