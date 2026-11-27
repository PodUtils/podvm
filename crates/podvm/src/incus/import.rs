//! The image of a run of the incus tier in incusd: imported now where its
//! alias is not there (TODO/machine.md T-1718, T-1997, T-2078), one import
//! of an alias at a time in a store (TODO/machine.md T-2081, OR-B39-02).
//!
//! TODO/machine.md T-2085 (PodUtils/podrunner#347 ask 1): the OCI layers
//! stream to incusd with no tree in the store. The extraction path serves
//! a tree that the flattener cannot write. Its temporary tree goes before
//! the create. A tight incusd images directory gets a gzip upload.

use std::io::{self, Write};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::task::{Context, Poll};

use pod_incus::{Client, ImageData, ImageImport, Payload};
use tokio::io::{AsyncRead, ReadBuf};

use super::image::{alias, unified_tarball_to, Layers};
use super::run::{Fault, Job};

/// The bound of the wait for the import of the same image by another podvm
/// of the store (T-2081).
pub const IMPORT_WAIT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// TODO/machine.md T-2081 (OR-B39-02): the lock of the import of `alias`
/// in the store of `job`, so that two runs of one store import an image
/// once. `None` where the run has no store. A holder that ends frees it.
pub(super) async fn import_lock(
    job: &Job<'_>,
    alias: &str,
) -> Result<Option<podvm_image::store::Lock>, Fault> {
    let Some(store) = job.store else {
        return Ok(None);
    };
    let key = format!("incus-{alias}");
    let start = std::time::Instant::now();
    let mut said = false;
    loop {
        match store.try_lock_key(&key) {
            Ok(Some(l)) => return Ok(Some(l)),
            Ok(None) if start.elapsed() < IMPORT_WAIT => {
                if !said {
                    eprintln!(
                        "podvm {}: incus tier: another podvm of this store imports the image \
                         {alias}; this run waits for it (TODO/machine.md T-2081)",
                        job.verb
                    );
                    said = true;
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            Ok(None) => {
                return Err((
                    format!(
                        "another podvm of this store held the import of the image {alias} for \
                         {} min: podvm does not import it beside that one (TODO/machine.md \
                         T-2081)",
                        IMPORT_WAIT.as_secs() / 60
                    ),
                    None,
                ))
            }
            Err(e) => return Err((format!("the lock of the import of {alias}: {e}"), None)),
        }
    }
}

/// TODO/machine.md T-2085: the tree of the image in the store, for an
/// import only.
pub trait Tree {
    /// The cached OCI layers. `None` serves a caller with just a tree.
    fn layers(&self) -> Result<Option<Layers>, String> {
        Ok(None)
    }

    /// The tree at `Job::rootfs`: `Ok(true)` where this call extracted it
    /// now. The error is one line.
    fn ensure(&self) -> Result<bool, String>;
    /// The bytes of the compressed layers: the estimate of the upload.
    fn layer_bytes(&self) -> u64;
    /// After the import: remove the tree that [`Tree::ensure`] extracted
    /// now, where no other process uses the image. A line where the removal
    /// failed.
    fn release(&self) -> Option<String>;
}

/// A writer that counts the SHA-256 of the bytes that went through it:
/// the fingerprint that incusd gives the unified tarball (T-2081).
struct Hashing<W> {
    inner: W,
    sum: Arc<Mutex<sha2::Sha256>>,
}

impl<W: Write> Write for Hashing<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        use sha2::Digest;
        let n = self.inner.write(buf)?;
        if let Ok(mut s) = self.sum.lock() {
            s.update(&buf[..n]);
        }
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// The producer marks a complete archive before it closes the pipe. A
/// failure must not end an HTTP chunked upload as a complete request.
struct UploadRead {
    file: tokio::fs::File,
    complete: Arc<AtomicBool>,
}

impl AsyncRead for UploadRead {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let before = buf.filled().len();
        std::task::ready!(Pin::new(&mut self.file).poll_read(cx, buf))?;
        if buf.filled().len() == before && !self.complete.load(Ordering::Acquire) {
            return Poll::Ready(Err(io::Error::other("the unified tarball writer failed")));
        }
        Poll::Ready(Ok(()))
    }
}

enum Archive {
    Layers(Layers, super::space::Need),
    Tree { path: PathBuf, fresh: bool },
}

/// A layer failure can explain an interrupted upload. A pipe or an older
/// tree writer error leaves incusd's response as the first diagnostic.
enum WriteFault {
    Layer(String),
    Other(String),
}

/// TODO/machine.md T-2081: incusd's words where an image of the same
/// fingerprint is there (another import of the same bytes won the race).
pub const SAME_FINGERPRINT: &str = "Image with same fingerprint already exists";

/// What a failed import is, after a second look (T-2081).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AfterImport {
    /// The alias is there now: another podvm imported the image.
    Aliased,
    /// The bytes are there with no alias: make the alias to this
    /// fingerprint.
    Alias(String),
    /// A failure.
    Failed,
}

/// The class of a failed import of an image: `alias_there` after a
/// re-read of the alias, `words` the error of incusd, `fingerprint` the
/// SHA-256 of the tarball where the writer wrote all of it.
pub fn after_import(alias_there: bool, words: &str, fingerprint: Option<&str>) -> AfterImport {
    if alias_there {
        return AfterImport::Aliased;
    }
    match fingerprint {
        Some(f) if words.contains(SAME_FINGERPRINT) => AfterImport::Alias(f.to_string()),
        _ => AfterImport::Failed,
    }
}

/// The alias of the image in incus: imported now where it is not there.
/// Before an import (TODO/machine.md T-2078, PodUtils/podrunner#346), the
/// need of the image is compared with the free space of the pool of the
/// default profile and of incusd's images, where the server's pools are of
/// the driver `dir`: a refusal by name where it cannot fit, a warning where
/// it fits below the estimate. `storage` is the `environment.storage` of
/// the server.
///
/// TODO/machine.md T-2081 (OR-B39-02): one import of an alias at a time in a
/// store (a lock), and a race with another store on the same incusd ends in
/// the image that is there (the alias, or the bytes of the same
/// fingerprint).
pub(super) async fn ensure_image(
    c: &Client,
    job: &Job<'_>,
    storage: &str,
) -> Result<(String, bool), Fault> {
    let me = std::fs::read("/proc/self/exe").map_err(|e| (format!("/proc/self/exe: {e}"), None))?;
    ensure_image_with(c, job, storage, me).await
}

pub(super) async fn ensure_image_with(
    c: &Client,
    job: &Job<'_>,
    storage: &str,
    me: Vec<u8>,
) -> Result<(String, bool), Fault> {
    let name = alias(job.digest, &crate::machine::guest::kernel::sha256_hex(&me));
    let there =
        |r: Result<Option<pod_incus::ImageAlias>, pod_incus::Error>| -> Result<bool, Fault> {
            r.map(|a| a.is_some()).map_err(|e| (e.to_string(), None))
        };
    if there(c.alias(&name).await)? {
        // T-2078 (OR-B39-13): the room of the pool for the root, also where
        // no import comes first.
        super::held::check(c, job, storage, &name).await?;
        return Ok((name, true));
    }
    let _lock = import_lock(job, &name).await?;
    // Another podvm of the store imported it while this one waited.
    if there(c.alias(&name).await)? {
        super::held::check(c, job, storage, &name).await?;
        return Ok((name, true));
    }
    if let Some(tree) = job.tree
        && let Some(layers) = tree.layers().map_err(|e| (e, None))?
        && let Some(need) = layers.measure(me.len() as u64).map_err(|e| (e, None))?
    {
        return upload(c, job, storage, &name, me, Archive::Layers(layers, need)).await;
    }
    // T-2085: a tree needed by the fallback is made under the import lock.
    let fresh = match job.tree {
        Some(t) => t.ensure().map_err(|e| (e, None))?,
        None => false,
    };
    let archive = Archive::Tree {
        path: job.rootfs.to_path_buf(),
        fresh,
    };
    let done = upload(c, job, storage, &name, me, archive).await;
    // T-2085: the tree that this import made goes after the import, also
    // after a failure (it holds the disk that the failure lacked); a later
    // import or native run extracts it again.
    if fresh && let Some(w) = job.tree.and_then(|t| t.release()) {
        eprintln!("podvm {}: incus tier: {w}", job.verb);
    }
    done
}

/// Check the room before sending the unified tarball under `name`.
async fn upload(
    c: &Client,
    job: &Job<'_>,
    storage: &str,
    name: &str,
    me: Vec<u8>,
    archive: Archive,
) -> Result<(String, bool), Fault> {
    let name = name.to_string();
    let (tree, upload, freed) = match &archive {
        Archive::Layers(layers, need) => (*need, layers.compressed_bytes(), None),
        Archive::Tree { path, fresh } => (
            super::space::need_of(path, me.len() as u64),
            job.tree.map_or(0, |t| t.layer_bytes()),
            fresh
                .then(|| std::fs::metadata(path).ok())
                .flatten()
                .map(|m| std::os::unix::fs::MetadataExt::dev(&m)),
        ),
    };
    // T-2085: gzip where incusd's images lack room for the plain upload.
    let images = super::space::incus_dir(&c.settings().socket)
        .and_then(|d| super::space::room_at(&d.join("images")));
    let plan = super::space::Plan {
        tree,
        upload,
        gzip: super::gzip::wanted(images.map(|r| r.bytes), tree.bytes),
        freed,
        imported: true,
    };
    let _ = job.plan.set(plan);
    if let Some(t) = super::space::Target::find(c, storage).await {
        for w in t.check(&plan, true).map_err(|e| (e, None))? {
            eprintln!("podvm {}: incus tier: {w}", job.verb);
        }
    }
    // The unified archive goes to incusd through a pipe (T-1997).
    let (r, w) = std::io::pipe().map_err(|e| (format!("a pipe for the import: {e}"), None))?;
    let sum = Arc::new(Mutex::new(<sha2::Sha256 as sha2::Digest>::new()));
    let hashing = Hashing {
        inner: w,
        sum: sum.clone(),
    };
    let mut out = super::gzip::Upload::new(hashing, plan.gzip);
    let complete = Arc::new(AtomicBool::new(false));
    let written = Arc::clone(&complete);
    let writer = std::thread::Builder::new()
        .spawn(move || {
            super::cut::block_pipe();
            match archive {
                Archive::Layers(layers, _) => {
                    let mut buffered = io::BufWriter::new(&mut out);
                    layers.flatten(&mut buffered, &me, false).map_err(|e| {
                        if e.io_cause()
                            .is_some_and(|io| io.kind() == io::ErrorKind::BrokenPipe)
                        {
                            WriteFault::Other(e.chain())
                        } else {
                            WriteFault::Layer(e.chain())
                        }
                    })?;
                }
                Archive::Tree { path, .. } => {
                    out = unified_tarball_to(&path, out, &me).map_err(WriteFault::Other)?;
                }
            }
            let out = out.finish().map_err(|e| {
                WriteFault::Other(format!("the end of the upload of the unified tarball: {e}"))
            })?;
            written.store(true, Ordering::Release);
            drop(out);
            Ok(())
        })
        .map_err(|e| (format!("the writer of the unified tarball: {e}"), None))?;
    let reader = UploadRead {
        file: tokio::fs::File::from_std(std::fs::File::from(std::os::fd::OwnedFd::from(r))),
        complete,
    };
    let mut import = ImageImport::new(ImageData::Unified(Payload::new(reader, None)));
    import.aliases = vec![name.clone()];
    // TODO/machine.md T-2082 (PodUtils/podrunner#347 ask 2): incusd that
    // cannot store the upload (a full disk) closes the connection while
    // podvm still writes; with SIGPIPE ignored the write gives EPIPE and
    // the error below, not a SIGPIPE that ends podvm with no line (141).
    let done = {
        let _quiet = super::cut::NoSigpipe::now();
        c.import_image(import).await
    };
    // The writer ends at the end of the tree, or with EPIPE where incusd
    // stopped reading; incusd's own error is the one that names the cause.
    let wrote = match writer.join() {
        Ok(Ok(())) => Ok(()),
        Ok(Err(WriteFault::Layer(e))) => return Err((e, None)),
        Ok(Err(WriteFault::Other(e))) => Err(e),
        Err(_) => Err("the writer of the tarball ended with a panic".into()),
    };
    if let Err(e) = done {
        let fingerprint = wrote.is_ok().then(|| {
            let s = sum.lock().map(|s| s.clone()).unwrap_or_default();
            sha2::Digest::finalize(s)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        });
        let alias_there = matches!(c.alias(&name).await, Ok(Some(_)));
        let words = e.incus_error().unwrap_or_default().to_string();
        match after_import(alias_there, &words, fingerprint.as_deref()) {
            AfterImport::Aliased => return Ok((name, true)),
            AfterImport::Alias(f) => match c.create_alias(&name, &f, "").await {
                Ok(()) => return Ok((name, true)),
                // Made by another podvm in the meantime.
                Err(_) if matches!(c.alias(&name).await, Ok(Some(_))) => return Ok((name, true)),
                Err(e2) => return Err((format!("{e}; then {e2}"), None)),
            },
            AfterImport::Failed => {}
        }
        let t = super::space::transport(&e);
        let hint = match super::space::after_failure(c, Some(&plan), true, t).await {
            Some(h) => Some(h),
            None if super::cut::cut(&e) => super::cut::after_cut(c, Some(&plan), true).await,
            None if super::cut::no_space(&e) => super::cut::after_cut(c, Some(&plan), false).await,
            None => None,
        };
        return Err((e.to_string(), hint));
    }
    wrote.map_err(|e| (e, None))?;
    // T-2078 (OR-B39-13): the need of the root for a later run of the alias,
    // which has no tree in the store to measure.
    if let Some(s) = job.store
        && let Err(w) = super::held::write(s, &name, tree)
    {
        eprintln!(
            "podvm {}: incus tier: the record of the need of the image: {w}",
            job.verb
        );
    }
    Ok((name, false))
}
