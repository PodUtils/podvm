//! TODO/image.md T-1320. Images move without a registry: save, load, import.
//!
//! One interchange shape: a tarball holding an OCI image layout
//! (`oci-layout`, `index.json`, `blobs/sha256/<hex>`). `save` writes the
//! store's own bytes under those names; `load` verifies every blob
//! against its descriptor on the way in, the same verification `pull`
//! performs, and registers the record; `import` builds a runnable record
//! from a plain rootfs tar by synthesizing the config and manifest the
//! store's shape requires.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::digest::Digest;
use crate::error::{Error, Result};
use crate::oci;
use crate::reference::Reference;
use crate::store::{Record, StagedFile, Store};

/// What `save` wrote, for the caller's report line.
pub struct Saved {
    pub record: Record,
    pub blobs: usize,
    pub bytes: u64,
}

// ⭐ The constants of the OCI layout, the entry filter and the shape of
// `index.json` are `pod_oci::layout` (PodUtils/podcrates).
use pod_oci::layout::{allowed_entry, MEDIA_UNTAR, OCI_LAYOUT, REF_ANNOTATION};

/// `podvm save`: the record's blobs as an OCI-layout tarball on `out`.
/// Blobs come from `read_blob`, so they are verified on the way out the
/// same way every other read is. ⚠ Blob bytes travel through memory: the
/// store's read API is memory-based, and save reuses it rather than
/// growing a second read path around verification.
pub fn save(store: &Store, want: &str, out: &mut dyn Write) -> Result<Saved> {
    save_for(store, want, None, out)
}

/// [`save`] of the record of `platform`, where the reference has more
/// than one (TODO/image.md T-1841).
pub fn save_for(
    store: &Store,
    want: &str,
    platform: Option<&crate::platform::Platform>,
    out: &mut dyn Write,
) -> Result<Saved> {
    let record = store.find_one_for(want, platform)?;
    let manifest_digest = Digest::parse(&record.manifest_digest)?;
    let manifest_bytes = store.read_blob(&manifest_digest)?;
    let manifest: oci::Manifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| Error::Oci(format!("the stored manifest does not parse: {e}")))?;
    let mut blobs: Vec<(String, Vec<u8>)> = vec![(manifest_digest.to_string(), manifest_bytes)];
    let manifest_len = blobs[0].1.len();
    let config_digest = manifest.config.parsed_digest()?;
    blobs.push((config_digest.to_string(), store.read_blob(&config_digest)?));
    for layer in &manifest.layers {
        let digest = layer.parsed_digest()?;
        blobs.push((digest.to_string(), store.read_blob(&digest)?));
    }
    blobs.sort_by(|a, b| a.0.cmp(&b.0));
    blobs.dedup_by(|a, b| a.0 == b.0);

    let refname = match &record.tag {
        Some(tag) => format!("{}:{tag}", record.repository),
        None => record.repository.clone(),
    };
    let index_bytes = pod_oci::layout::index_json(
        &manifest.media_type,
        &manifest_digest,
        manifest_len,
        &refname,
    )?;

    let mut archive = tar::Builder::new(out);
    append(&mut archive, "oci-layout", OCI_LAYOUT.as_bytes())?;
    append(&mut archive, "index.json", &index_bytes)?;
    let mut bytes = 0u64;
    for (digest, blob) in &blobs {
        let hex = digest.trim_start_matches("sha256:");
        append(&mut archive, &format!("blobs/sha256/{hex}"), blob)?;
        bytes += blob.len() as u64;
    }
    let count = blobs.len();
    archive
        .into_inner()
        .map_err(|e| Error::Oci(format!("the save tarball does not finish: {e}")))?;
    Ok(Saved {
        record,
        blobs: count,
        bytes,
    })
}

fn append(archive: &mut tar::Builder<&mut dyn Write>, path: &str, data: &[u8]) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, path, data)
        .map_err(|e| Error::Oci(format!("the save tarball does not take {path}: {e}")))?;
    Ok(())
}

/// A blob streamed out of the tarball into a staging file, hashed on the
/// way. The name it arrived under is untrusted; only the computed digest
/// decides where it goes.
struct StagedBlob {
    digest: Digest,
    path: PathBuf,
    // ⭐ The in-store staging handle, held until the commit. Its exclusive
    // lock is what tells `sweep_staging` this file is being written rather
    // than abandoned (invariant I5).
    file: StagedFile,
}

/// A blob of the tarball after its commit: its computed digest and the
/// name of its entry (untrusted; for the refusal).
struct LoadedBlob {
    digest: Digest,
    entry: String,
    referenced: bool,
}

/// `podvm load`: an OCI-layout tarball into the store. Every blob is
/// hashed on the way in and matched against a descriptor before the
/// record is written, so a tarball whose bytes changed in transit is
/// refused the way a bad pull is.
///
/// ⛔ TODO/image.md T-2068 (PodUtils/podrunner#344): each blob is
/// committed by its computed digest as soon as its bytes are written, so
/// a load holds one staging lock at a time, as each worker of a pull
/// does. Before, the load held the staging lock of each blob until the
/// index was read, and the 15th blob passed the slots of the lock table
/// (`FORK_CLOSE_SLOTS`): an image of 85 layers did not load. A blob
/// before the record is safe as the blob of a pull is: the sweep of the
/// strays removes none while [`Store::writing`] is held, and the record
/// is written only where each blob that it names is in the store
/// ([`Store::put_written`]). A blob that the manifest does not name stays
/// a stray, which `image prune` removes.
///
/// `shown` is the name of the archive in each message: `src` where the
/// caller gave a file, and words of stdin where `src` is the spool of the
/// store that took stdin (TODO/cli.md T-2100: the user did not name it).
pub fn load(store: &Store, src: &Path, shown: &Path) -> Result<Record> {
    // PodUtils/podvm#527: a sweep keeps each blob until the record.
    let _writing = store.writing()?;
    let file = std::fs::File::open(src).map_err(|e| Error::io(src.display().to_string(), e))?;
    let mut archive = tar::Archive::new(file);
    load_inner(store, shown, &mut archive)
}

fn load_inner(
    store: &Store,
    src: &Path,
    archive: &mut tar::Archive<std::fs::File>,
) -> Result<Record> {
    let mut index_bytes: Option<Vec<u8>> = None;
    let mut loaded: Vec<LoadedBlob> = Vec::new();
    let entries = archive
        .entries()
        .map_err(|e| Error::Oci(format!("{} is not a tarball: {e}", src.display())))?;
    for entry in entries {
        let mut entry =
            entry.map_err(|e| Error::Oci(format!("{} does not unpack: {e}", src.display())))?;
        let path = entry.path().map_err(|e| {
            Error::Oci(format!(
                "{} has an unreadable entry name: {e}",
                src.display()
            ))
        })?;
        let name = path.to_string_lossy().replace('\\', "/");
        let name = name.trim_start_matches("./").to_string();
        // PodUtils/podvm#477: `tar -cf X .` of a layout writes its own
        // directories too; they carry nothing.
        let dir = name.trim_end_matches('/');
        if entry.header().entry_type().is_dir()
            && matches!(dir, "" | "." | "blobs" | "blobs/sha256")
        {
            continue;
        }
        if !allowed_entry(&name) {
            return Err(Error::Oci(format!(
                "{} carries {name:?}, which is not part of a single-image OCI layout. \
                 podvm loads one image per tarball",
                src.display()
            )));
        }
        if name == "oci-layout" {
            continue;
        }
        if name == "index.json" {
            let mut bytes = Vec::new();
            entry
                .read_to_end(&mut bytes)
                .map_err(|e| Error::Oci(format!("{} does not read: {e}", src.display())))?;
            index_bytes = Some(bytes);
            continue;
        }
        // ⛔ T-2068: committed at once, so the staging lock goes before
        // the next entry.
        let digest = commit_entry(store, src, &mut entry)?;
        loaded.push(LoadedBlob {
            digest,
            entry: name,
            referenced: false,
        });
    }
    let Some(index_bytes) = &index_bytes else {
        return Err(Error::Oci(format!(
            "{} has no index.json, so it names no image",
            src.display()
        )));
    };
    let index: oci::Index = serde_json::from_slice(index_bytes).map_err(|e| {
        Error::Oci(format!(
            "{} has an index that does not parse: {e}",
            src.display()
        ))
    })?;
    // TODO/image.md T-1839: the OCI image layout requires schemaVersion 2.
    if index.schema_version != 2 {
        return Err(Error::Oci(format!(
            "{} has an index of schemaVersion {}; the OCI image layout has 2 \
             (TODO/image.md T-1839)",
            src.display(),
            index.schema_version
        )));
    }
    if index.manifests.len() != 1 {
        return Err(Error::Oci(format!(
            "{} names {} manifests; podvm loads one image per tarball",
            src.display(),
            index.manifests.len()
        )));
    }
    let descriptor = &index.manifests[0];
    let manifest_digest = descriptor.parsed_digest()?;
    take_loaded(&mut loaded, &manifest_digest, src)?;
    let manifest_bytes = store.read_blob(&manifest_digest)?;
    let manifest: oci::Manifest = serde_json::from_slice(&manifest_bytes).map_err(|e| {
        Error::Oci(format!(
            "{} has a manifest that does not parse: {e}",
            src.display()
        ))
    })?;
    let config_digest = manifest.config.parsed_digest()?;
    take_loaded(&mut loaded, &config_digest, src)?;
    let config_bytes = store.read_blob(&config_digest)?;
    let config: oci::Config = serde_json::from_slice(&config_bytes).map_err(|e| {
        Error::Oci(format!(
            "{} has a config that does not parse: {e}",
            src.display()
        ))
    })?;
    for layer in &manifest.layers {
        take_loaded(&mut loaded, &layer.parsed_digest()?, src)?;
    }
    let extras = loaded.iter().filter(|blob| !blob.referenced).count();
    if extras != 0 {
        return Err(Error::Oci(format!(
            "{} carries {} blob(s) the manifest does not name; refusing rather than \
             recording bytes no record points at. `podvm image prune` removes them",
            src.display(),
            extras
        )));
    }
    // TODO/image.md T-1839: with no name, the record has no tag and is
    // found by its digest, as docker's `<none>`. The digest read as a name
    // gave the repository `sha256` and the tag of its hex.
    let named = descriptor.annotations.get(REF_ANNOTATION).cloned();
    let reference = Reference::parse(named.as_deref().unwrap_or(UNNAMED))?;
    let platform = if config.os.is_empty() || config.architecture.is_empty() {
        // ⚠ Degenerate configs exist; the host platform is the honest
        // fallback, and the record says exactly that.
        crate::platform::Platform::host().to_string()
    } else {
        format!("{}/{}", config.os, config.architecture)
    };
    let record = crate::store::record_of(
        &reference,
        &manifest_digest,
        &descriptor.media_type,
        &manifest_digest,
        &manifest,
        &config,
        &platform,
    )?;
    let mut record = record;
    if named.is_none() {
        record.repository = "<none>".to_string();
        record.tag = None;
    }
    store.put_written(record.clone())?;
    Ok(record)
}

/// Stream one tarball entry to a staging file, hashing on the way.
///
/// ⭐ The staging file lives inside the store (`Store::stage`), because
/// the commit is a `rename(2)` and a rename across filesystems is
/// `EXDEV`: staging into `temp_dir()` broke `load` wherever /tmp and
/// the store sit on different filesystems (found by the T-1338 perf
/// harness on the lane: tmpfs /tmp against an overlayfs store).
/// Blobs stream to files, so the tarball is never fully in memory.
fn stage_entry(store: &Store, src: &Path, entry: &mut dyn Read) -> Result<StagedBlob> {
    use sha2::Digest as _;
    let (path, mut out) = store.stage("load")?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let copied = loop {
        let n = match entry.read(&mut buf) {
            Ok(n) => n,
            Err(e) => break Err(Error::Oci(format!("{} does not read: {e}", src.display()))),
        };
        if n == 0 {
            break Ok(());
        }
        hasher.update(&buf[..n]);
        if let Err(e) = out.write_all(&buf[..n]) {
            break Err(Error::io(path.display().to_string(), e));
        }
    };
    if let Err(e) = copied {
        // The staging file goes on each path out, under its lock.
        let _ = std::fs::remove_file(&path);
        return Err(e);
    }
    let hex = crate::digest::hex_of(&hasher.finalize());
    Ok(StagedBlob {
        digest: Digest::parse(&format!("sha256:{hex}"))?,
        path,
        file: out,
    })
}

/// Stage one tarball entry and commit it by its computed digest
/// (TODO/image.md T-2068). The commit is the one of a pull
/// (`pod_oci::pull::Blobs`): the flush, the sync and the rename, under the
/// staging lock, so a crash leaves a whole blob or a `*.partial` that the
/// sweep removes. The lock goes when this returns.
fn commit_entry(store: &Store, src: &Path, entry: &mut dyn Read) -> Result<Digest> {
    let blob = stage_entry(store, src, entry)?;
    let path = blob.path.clone();
    match pod_oci::pull::Blobs::commit(store, &blob.path, blob.file, &blob.digest) {
        Ok(()) => Ok(blob.digest),
        Err(e) => {
            let _ = std::fs::remove_file(&path);
            Err(e)
        }
    }
}

/// Match one reference against verified bytes from this archive. Prefer a
/// physical entry that has not been matched yet, since a tar can repeat it.
fn take_loaded(loaded: &mut [LoadedBlob], want: &Digest, src: &Path) -> Result<()> {
    if let Some(blob) = loaded
        .iter_mut()
        .find(|b| !b.referenced && b.digest == *want)
    {
        blob.referenced = true;
        return Ok(());
    }
    // TODO/image.md T-1836: name a remaining entry with the expected name
    // and wrong bytes even when an earlier reference matched this digest.
    let hex = want.to_string();
    let hex = hex.trim_start_matches("sha256:");
    let mismatched = loaded
        .iter()
        .find(|b| !b.referenced && b.entry.ends_with(hex));
    if mismatched.is_none() && loaded.iter().any(|b| b.referenced && b.digest == *want) {
        return Ok(());
    }
    let got = mismatched
        .map(|b| format!("{} for the entry {}", b.digest, b.entry))
        .unwrap_or_else(|| "no entry of the tarball".to_string());
    Err(Error::DigestMismatch {
        what: format!("tarball {}", src.display()),
        want: want.to_string(),
        got,
    })
}

/// The placeholder name of an index entry with no name (T-1839); the
/// record replaces it with `<none>` and no tag.
const UNNAMED: &str = "podvm.invalid/unnamed:none";

mod import_tar; // out of this file for the limit of 500 lines
pub use import_tar::import;

#[cfg(test)]
mod tests;
