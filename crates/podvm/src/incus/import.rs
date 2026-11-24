//! The image of a run of the incus tier in incusd: imported now where its
//! alias is not there (TODO/machine.md T-1718, T-1997, T-2078), one import
//! of an alias at a time in a store (TODO/machine.md T-2081, OR-B39-02).

use pod_incus::{Client, ImageData, ImageImport, Payload};

use super::image::{alias, unified_tarball_to};
use super::run::{Fault, Job};

/// The bound of the wait for the import of the same image by another podvm
/// of the store (T-2081).
pub const IMPORT_WAIT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// TODO/machine.md T-2081 (OR-B39-02): the lock of the import of `alias`
/// in the store of `job`, so that two runs of one store import an image
/// once. `None` where the run has no store. A holder that ends frees it.
async fn import_lock(
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

/// A writer that counts the SHA-256 of the bytes that went through it:
/// the fingerprint that incusd gives the unified tarball (T-2081).
struct Hashing<W> {
    inner: W,
    sum: std::sync::Arc<std::sync::Mutex<sha2::Sha256>>,
}

impl<W: std::io::Write> std::io::Write for Hashing<W> {
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
    let name = alias(job.digest, &crate::machine::guest::kernel::sha256_hex(&me));
    let there =
        |r: Result<Option<pod_incus::ImageAlias>, pod_incus::Error>| -> Result<bool, Fault> {
            r.map(|a| a.is_some()).map_err(|e| (e.to_string(), None))
        };
    if there(c.alias(&name).await)? {
        return Ok((name, true));
    }
    let _lock = import_lock(job, &name).await?;
    // Another podvm of the store imported it while this one waited.
    if there(c.alias(&name).await)? {
        return Ok((name, true));
    }
    if let Some(t) = super::space::Target::find(c, storage).await {
        let need = super::space::need_of(job.rootfs, me.len() as u64);
        for w in t.check(need, true).map_err(|e| (e, None))? {
            eprintln!("podvm {}: incus tier: {w}", job.verb);
        }
    }
    // TODO/machine.md T-1997 (PodUtils/podrunner#316): the tarball goes to
    // incusd through a pipe, in chunks, as a thread writes it: no tarball
    // file in the store beside the tree and the image of incusd.
    let (r, w) = std::io::pipe().map_err(|e| (format!("a pipe for the import: {e}"), None))?;
    let rootfs = job.rootfs.to_path_buf();
    let sum = std::sync::Arc::new(std::sync::Mutex::new(<sha2::Sha256 as sha2::Digest>::new()));
    let hashing = Hashing {
        inner: w,
        sum: sum.clone(),
    };
    let writer = std::thread::spawn(move || {
        // T-2080: a reader that went gives EPIPE here, not a SIGPIPE that
        // ends podvm.
        let mut old = 0u64;
        let _ = podvm_probe::sys::sigblock(1 << (13 - 1), &mut old);
        unified_tarball_to(&rootfs, hashing, &me)
    });
    let reader = tokio::fs::File::from_std(std::fs::File::from(std::os::fd::OwnedFd::from(r)));
    let mut import = ImageImport::new(ImageData::Unified(Payload::new(reader, None)));
    import.aliases = vec![name.clone()];
    // TODO/machine.md T-2082 (PodUtils/podrunner#347 ask 2): incusd that
    // cannot store the upload (a full disk) closes the connection while
    // podvm still writes; with SIGPIPE ignored the write gives EPIPE and
    // the error below, not a SIGPIPE that ends podvm with no line (141).
    let done = {
        let _quiet = super::space::NoSigpipe::now();
        c.import_image(import).await
    };
    // The writer ends at the end of the tree, or with EPIPE where incusd
    // stopped reading; incusd's own error is the one that names the cause.
    let wrote = writer
        .join()
        .unwrap_or_else(|_| Err("the writer of the tarball ended with a panic".into()));
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
        let hint = match super::space::after_failure(c, job.rootfs, true, t).await {
            Some(h) => Some(h),
            None if super::space::cut(&e) => super::space::after_cut(c, job.rootfs, true).await,
            None if super::space::no_space(&e) => {
                super::space::after_cut(c, job.rootfs, false).await
            }
            None => None,
        };
        return Err((e.to_string(), hint));
    }
    wrote.map_err(|e| (e, None))?;
    Ok((name, false))
}
