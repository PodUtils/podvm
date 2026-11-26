use super::*;

/// What [`Store::delete`] does about an image a holder is using.
///
/// ⛔ Two behaviours and one check, because the check has to happen under the
/// index lock (invariant I4) and only the caller knows whether being held is a
/// refusal (`rmi`, which names one image) or a skip (`prune`, which sweeps).
/// Public for the same reason `delete` is: the CLI's record-gate
/// (TODO/image.md T-1322) deletes the unreferenced remainder through it.
#[derive(Debug, Clone, Copy)]
pub enum Held {
    Refuse,
    Skip,
}

#[derive(Debug)]
pub struct Removed {
    pub untagged: Vec<String>,
    pub deleted: Vec<String>,
    pub freed_bytes: u64,
    /// ⛔ Named, never silent. T-0204: `prune` and `rmi` skip anything locked
    /// **and say which**.
    pub skipped: Vec<String>,
}

/// A staging file, held exclusively for as long as this value lives.
///
/// ⛔ **Invariant I5's mechanism.** [`Store::sweep_staging`] decides what is
/// abandoned by trying to lock each `*.partial`, so a file being written has to
/// be locked or the sweep would delete it out from under its writer. The lock
/// goes when this value does, however the process ends.
///
/// ⚠ **Two open file descriptions on one inode, not one.** `flock` is held on
/// the description [`Lock`] opened, and the writes go through a second one this
/// type opens after it. They are independent: closing the writer releases no
/// lock, and the lock's own `Drop` is the only thing that does. ⛔ So a
/// `StagedFile` costs two descriptors, and a reader counting descriptors while
/// chasing a lock that outlived its holder has to know that
/// ([`TODO/image.md`](../../../TODO/image.md) T-0215).
pub struct StagedFile {
    file: std::fs::File,
    /// ⚠ Held for its `Drop`, and never read. The lock is the point.
    _lock: Lock,
}

impl StagedFile {
    pub(super) fn create(path: &Path) -> Result<StagedFile> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::io(parent.display().to_string(), e))?;
        }
        // ⛔ The lock FIRST. Creating the file and locking it afterwards leaves
        // a window in which a sweep sees an unlocked `*.partial` and removes it.
        let lock = match try_lock_at(path, sys::LOCK_EX)? {
            Some(l) => l,
            None => {
                // TODO/image.md T-2069: the name holds 64 random bits, so
                // a clash is two writers that drew one name, or a process
                // that holds a file of the store that it did not make.
                return Err(Error::Store(format!(
                    "another process holds the staging file {}: this write stops, and \
                     the store is as it was (TODO/image.md T-2069)",
                    path.display()
                )));
            }
        };
        let file = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(|e| Error::io(path.display().to_string(), e))?;
        Ok(StagedFile { file, _lock: lock })
    }

    pub fn sync_all(&self) -> std::io::Result<()> {
        self.file.sync_all()
    }

    /// [`StagedFile::create`] for the tests of the store.
    #[cfg(test)]
    pub(crate) fn create_for_test(path: &Path) -> Result<StagedFile> {
        StagedFile::create(path)
    }
}

impl Write for StagedFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

/// ⭐ [`TODO/image.md`](../../../TODO/image.md) T-0214. A transfer that was cut
/// short starts again on this same file, under the same lock and at the same
/// name: the alternative is a second staging path per attempt, and a sweep that
/// has to tell them apart.
///
/// ⛔ Truncate AND rewind. `set_len(0)` alone leaves the offset where the failed
/// attempt left it, so the next byte lands two megabytes in and the file is a
/// hole followed by the retry.
impl crate::registry::Restart for StagedFile {
    fn restart(&mut self) -> std::io::Result<()> {
        use std::io::Seek;
        self.file.set_len(0)?;
        self.file.seek(std::io::SeekFrom::Start(0))?;
        Ok(())
    }
}

/// An advisory lock held for as long as this value lives.
///
/// It is `pod_supervise::lock::Lock` (PodUtils/podcrates) since T-060 of
/// podcrates: `O_CLOEXEC` from its open, registered with
/// [`sys::close_in_children`] at its one constructor, released with an
/// explicit `LOCK_UN` in `Drop`, and [`Lock::hand_to_payload`] for the one
/// lock a payload keeps (T-0204, T-0211, T-0215).
pub use pod_supervise::lock::Lock;

/// ⛔ The T-0215 instrument runs at the `EWOULDBLOCK` itself, before the
/// refusal returns: test builds only, and silent unless
/// `PODVM_T0215_CAPTURE` is set.
#[cfg(test)]
const NOTE: &dyn Fn(&Path, i64) = &tests::note_the_refusal;
#[cfg(not(test))]
const NOTE: &dyn Fn(&Path, i64) = &|_, _| {};

/// [`Lock::try_acquire`] with podvm's errors: `None` where somebody else
/// holds it. ⛔ Always `LOCK_NB`: `RULES.md` section 8.
pub(super) fn try_lock_at(path: &Path, op: u64) -> Result<Option<Lock>> {
    Lock::try_acquire_noting(path, op, NOTE).map_err(lock_error)
}

/// [`Lock::acquire`] with podvm's errors: a bounded number of attempts.
pub(super) fn lock_at(path: &Path, op: u64) -> Result<Lock> {
    Lock::acquire_noting(path, op, NOTE).map_err(lock_error)
}

/// A failure of `pod_supervise::lock` as this crate's error. An I/O failure
/// (a parent directory that cannot be made) keeps its kind and its path;
/// the rest (a NUL, an errno of `open` or `flock`, the slots, the bound) is
/// `Store`.
fn lock_error(f: pod_core::Failure) -> Error {
    match f.io_cause() {
        Some(e) if f.text().ends_with(&format!(": {e}")) => {
            let path = f.text().strip_suffix(&format!(": {e}")).unwrap_or(f.text());
            Error::io(path, std::io::Error::new(e.kind(), e.to_string()))
        }
        _ => Error::Store(f.chain()),
    }
}

/// Build the record a completed pull writes.
#[allow(clippy::too_many_arguments)]
pub fn record_of(
    reference: &Reference,
    resolved: &Digest,
    resolved_media_type: &str,
    manifest_digest: &Digest,
    manifest: &oci::Manifest,
    config: &oci::Config,
    platform: &str,
) -> Result<Record> {
    Ok(Record {
        repository: reference.canonical_repository(),
        tag: reference.tag.clone(),
        digest: resolved.to_string(),
        digest_media_type: resolved_media_type.to_string(),
        manifest_digest: manifest_digest.to_string(),
        config_digest: manifest.config.parsed_digest()?.to_string(),
        platform: platform.to_string(),
        layers: manifest.layers.iter().map(|l| l.digest.clone()).collect(),
        stored_bytes: manifest.stored_bytes(),
        architecture: config.architecture.clone(),
        os: config.os.clone(),
        created: config.created.clone(),
        pulled_at: clock::now(),
    })
}

/// Create the store root with mode 0700 where it does not exist yet; its
/// parents get the default modes. ⛔ TODO/supervise.md T-1715: the
/// container table and the logs live under the root, and the table keeps
/// each `-e` value, so only the owner may reach them. A root that exists
/// keeps its mode ([`Store::exposed_mode`] reports it).
pub(super) fn create_private_root(root: &Path) -> Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    if root.is_dir() {
        return Ok(());
    }
    if let Some(parent) = root.parent() {
        std::fs::create_dir_all(parent).map_err(|e| root_error(root, parent, e))?;
    }
    match std::fs::DirBuilder::new().mode(0o700).create(root) {
        Ok(()) => Ok(()),
        // Another podvm made it between the check and the create.
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && root.is_dir() => Ok(()),
        Err(e) => Err(root_error(root, root, e)),
    }
}

/// The error of a directory `at` of the store root `root` that podvm
/// cannot make. TODO/cli.md T-2100 (OR-B41-17): it names the store, and a
/// file on the path by name, where a bare `File exists` named neither.
pub(super) fn root_error(root: &Path, at: &Path, e: std::io::Error) -> Error {
    let file = at
        .ancestors()
        .find(|a| a.symlink_metadata().is_ok_and(|m| !m.is_dir()) && !a.is_dir());
    match file {
        Some(f) => Error::Store(format!(
            "{} cannot be the store root: {} is a file, not a directory \
             (PODVM_STORE sets the root)",
            root.display(),
            f.display()
        )),
        None => Error::Store(format!(
            "{} cannot be the store root: {}: {e}",
            root.display(),
            at.display()
        )),
    }
}
