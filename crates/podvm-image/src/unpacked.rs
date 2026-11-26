//! TODO/image.md T-2104 (PodUtils/podrunner#372): the bytes of the unpacked
//! root file system of an image, known before the first run.
//!
//! The number is the sum of the sizes of the regular files of the layers
//! after the whiteouts and the replacements of newer layers, with each inode
//! once: the bytes that the root of a guest holds. It is not the size of the
//! layers on the wire ([`Record::stored_bytes`]).
//!
//! One walk of the layers gives it: `pod_extract::flatten`, the stream that
//! the incus tier already measures (T-2085, `podvm::incus::image`). The
//! stream writes a hard link as an entry of size 0 that names the first
//! name of its inode, so the sum of the sizes of the file entries counts each
//! inode once. [`count_tar`] is that count, for both callers. Nothing is
//! written to the disk, and each layer is read once, as it is verified.
//!
//! `pull`, `load` and `import` call [`fill`] before they write the record
//! ([`Record::unpacked_bytes`]). A failure here never fails them: the field
//! stays `None`, and `inspect` measures again and gives the reason.

use std::io::Read;

use pod_extract::flatten::{flatten, Options};
use pod_extract::Layer;

use crate::error::Result;
use crate::oci;
use crate::store::{Record, Store};

/// What [`count_tar`] counted below a prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Count {
    /// The sizes of the regular files, in bytes.
    pub bytes: u64,
    /// The entries that need an inode of their own: all but the hard links.
    pub entries: u64,
}

/// Count the tar stream `read` of `flatten`: the entries below `prefix`
/// (empty: all), with `each` called for the path (the prefix removed) and
/// the kind of each of them. The stream is read to its end.
pub fn count_tar(
    read: impl Read,
    prefix: &[u8],
    mut each: impl FnMut(&[u8], tar::EntryType),
) -> std::io::Result<Count> {
    let mut count = Count::default();
    let mut archive = tar::Archive::new(std::io::BufReader::new(read));
    // pod-extract writes the full size into its headers, so the count does
    // not have to interpret the binary PAX attributes of a sparse file.
    for entry in archive.entries()?.raw(true) {
        let entry = entry?;
        let path = entry.path_bytes();
        let Some(path) = path.strip_prefix(prefix) else {
            continue;
        };
        let kind = entry.header().entry_type();
        each(path, kind);
        if !kind.is_hard_link() {
            count.entries = count.entries.saturating_add(1);
        }
        if kind.is_file() {
            count.bytes = count.bytes.saturating_add(entry.size());
        }
    }
    // The reader stops at the first end block. The writer ends after both.
    std::io::copy(&mut archive.into_inner(), &mut std::io::sink())?;
    Ok(count)
}

/// The layers of `manifest` in order, with their blobs in `store`.
pub fn layers(store: &Store, manifest: &oci::Manifest) -> Result<Vec<Layer>> {
    let mut out = Vec::new();
    for d in &manifest.layers {
        let parsed = d.parsed_digest()?;
        out.push(Layer {
            digest: parsed.to_string(),
            media_type: d.media_type.clone(),
            size: d.size,
            blob: store.blob_path(&parsed),
        });
    }
    Ok(out)
}

/// The unpacked bytes of `layers`, or the reason in one line. Layers that a
/// stream cannot hold (a digest that is not sha256, a media type that
/// `extract` refuses, a path past the bound) give the reason, not a number.
pub fn measure(layers: &[Layer]) -> std::result::Result<u64, String> {
    let (read, write) =
        std::io::pipe().map_err(|e| format!("a pipe for measuring the image: {e}"))?;
    std::thread::scope(|scope| {
        let writer = std::thread::Builder::new()
            .spawn_scoped(scope, move || {
                flatten(
                    layers,
                    &mut std::io::BufWriter::new(write),
                    &Options::default(),
                )
            })
            .map_err(|e| format!("the writer for measuring the image: {e}"))?;
        // `read` is dropped where the count ends or fails, so a writer that
        // is still writing gets EPIPE and ends: the join cannot hang.
        let counted = count_tar(read, b"", |_, _| {});
        let flattened = writer
            .join()
            .map_err(|_| "the writer for measuring the image ended with a panic".to_string())?;
        // The failure of the stream is the cause; the count of a cut stream
        // is a symptom.
        if let Err(e) = flattened {
            return Err(e.chain());
        }
        counted
            .map(|c| c.bytes)
            .map_err(|e| format!("the stream of the layers: {e}"))
    })
}

/// The unpacked bytes of the layers of `manifest` in `store`, or the reason.
pub fn of_manifest(store: &Store, manifest: &oci::Manifest) -> std::result::Result<u64, String> {
    let layers = layers(store, manifest).map_err(|e| e.to_string())?;
    if let Some(l) = layers.iter().find(|l| !l.blob.is_file()) {
        return Err(format!(
            "the layer {} is not in the store ({})",
            l.digest,
            l.blob.display()
        ));
    }
    measure(&layers)
}

/// The unpacked bytes of the image of `r`: the value of the record, else a
/// measure of its layers in `store`; or the reason, in one line.
pub fn of(store: &Store, r: &Record) -> std::result::Result<u64, String> {
    if let Some(n) = r.unpacked_bytes {
        return Ok(n);
    }
    let manifest = crate::digest::Digest::parse(&r.manifest_digest)
        .map_err(|e| format!("the manifest digest of the record: {e}"))
        .and_then(|d| {
            store
                .read_blob(&d)
                .map_err(|e| format!("the manifest {} is not readable: {e}", r.manifest_digest))
        })
        .and_then(|b| {
            serde_json::from_slice::<oci::Manifest>(&b).map_err(|e| {
                format!(
                    "the stored manifest {} does not parse: {e}",
                    r.manifest_digest
                )
            })
        })?;
    of_manifest(store, &manifest)
}

/// Set `record.unpacked_bytes` for a writer (`pull`, `load`, `import`),
/// before it writes the record. The value of another record of the same
/// manifest is taken as it is: the bytes follow from the manifest alone. A
/// failure leaves the field `None`: the pull itself did not fail.
pub fn fill(store: &Store, manifest: &oci::Manifest, record: &mut Record) {
    if let Ok(index) = store.read_index()
        && let Some(n) = index
            .images
            .iter()
            .filter(|r| r.manifest_digest == record.manifest_digest)
            .find_map(|r| r.unpacked_bytes)
    {
        record.unpacked_bytes = Some(n);
        return;
    }
    record.unpacked_bytes = of_manifest(store, manifest).ok();
}

/// For `inspect`: `r` with its unpacked bytes set where they can be known,
/// and the reason where they cannot. A value that this call measured is kept
/// in the store, where the store accepts a write (a reader ignores a
/// refusal).
pub fn complete(store: &Store, r: Record) -> (Record, Option<String>) {
    if r.unpacked_bytes.is_some() {
        return (r, None);
    }
    match of(store, &r) {
        Ok(n) => {
            let _ = store.set_unpacked(&r.manifest_digest, n);
            (
                Record {
                    unpacked_bytes: Some(n),
                    ..r
                },
                None,
            )
        }
        Err(why) => (r, Some(why)),
    }
}
