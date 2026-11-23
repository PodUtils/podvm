//! `podvm import` (TODO/image.md T-1839): a plain rootfs tar into a record.

use std::io::{Read, Write};
use std::path::Path;

use super::*;

/// TODO/image.md T-1839: a member that `extract` refuses is refused at `import`.
fn escaping_member(tar: &Path) -> Result<Option<String>> {
    let f = std::fs::File::open(tar).map_err(|e| Error::io(tar.display().to_string(), e))?;
    let mut a = ::tar::Archive::new(f);
    let entries = a
        .entries()
        .map_err(|e| Error::Oci(format!("{} does not read: {e}", tar.display())))?;
    let bad = |p: &str| p.starts_with('/') || p.split('/').any(|c| c == "..");
    // PodUtils/podvm#525: a hard link names a member that came before it.
    let mut seen = std::collections::HashSet::new();
    for e in entries {
        let e = e.map_err(|e| Error::Oci(format!("{} does not read: {e}", tar.display())))?;
        let raw = String::from_utf8_lossy(&e.path_bytes()).to_string();
        if bad(&raw) {
            return Ok(Some(raw));
        }
        // PodUtils/podvm#503: the target of a hard link takes the same rule.
        if e.header().entry_type() == ::tar::EntryType::Link
            && let Some(t) = e.link_name_bytes()
        {
            let t = String::from_utf8_lossy(&t).to_string();
            let norm = |p: &str| p.trim_start_matches("./").trim_end_matches('/').to_string();
            if bad(&t) || !seen.contains(&norm(&t)) {
                return Ok(Some(format!(
                    "{raw} (a hard link to {t}, which is absolute, holds .., or comes later)"
                )));
            }
        }
        seen.insert(
            raw.trim_start_matches("./")
                .trim_end_matches('/')
                .to_string(),
        );
    }
    Ok(None)
}

/// `podvm import`: a plain rootfs tar into a runnable record, around the
/// tar's own digest; a compressed file is refused, as the media type says
/// plain.
pub fn import(store: &Store, tar: &Path, reference: Option<&str>) -> Result<Record> {
    // PodUtils/podvm#527: a sweep keeps each blob until the record.
    let _writing = store.writing()?;
    // PodUtils/podvm#515: the magic of a compressed file is read from what
    // is there, also under 512 bytes, and xz and bzip2 are named too.
    let mut head = [0u8; 512];
    let n = {
        let f = std::fs::File::open(tar).map_err(|e| Error::io(tar.display().to_string(), e))?;
        f.take(512)
            .read(&mut head)
            .map_err(|e| Error::Oci(format!("{} does not read: {e}", tar.display())))?
    };
    const MAGIC: [(&[u8], &str); 4] = [
        (&[0x1f, 0x8b], "gzip"),
        (&[0x28, 0xb5, 0x2f, 0xfd], "zstd"),
        (&[0xfd, b'7', b'z', b'X', b'Z', 0x00], "xz"),
        (b"BZh", "bzip2"),
    ];
    if let Some((_, kind)) = MAGIC.iter().find(|(m, _)| head[..n].starts_with(m)) {
        return Err(Error::Oci(format!(
            "{} is compressed ({kind}); import takes a plain tar so the record's media type \
             stays true. Decompress it first",
            tar.display()
        )));
    }
    if n < 512 || &head[257..262] != b"ustar" {
        return Err(Error::Oci(format!(
            "{} is not a tar archive (no ustar magic); nothing runnable can be built from it",
            tar.display()
        )));
    }
    if let Some(member) = escaping_member(tar)? {
        return Err(Error::Oci(format!(
            "{} has the member {member:?}, which extract would refuse (it leaves the root \
             of the tree, or links to a member that is not there yet), so import refuses \
             it now (TODO/image.md T-1839)",
            tar.display()
        )));
    }
    let (staged_path, mut staged_file) = store.stage("import")?;
    let layer_digest = stream_file_hashed(tar, &mut staged_file)?;
    drop(staged_file);
    store.commit(&staged_path, &layer_digest)?;
    let layer_len = std::fs::metadata(store.blob_path(&layer_digest))
        .map(|m| m.len())
        .map_err(|e| Error::io(tar.display().to_string(), e))?;

    let host = crate::platform::Platform::host();
    let created = crate::clock::now();
    let config_value = serde_json::json!({
        "architecture": host.arch,
        "os": "linux",
        "created": created,
        "rootfs": {"type": "layers", "diff_ids": [layer_digest.to_string()]},
        "config": {},
    });
    let config_bytes = serde_json::to_vec(&config_value)
        .map_err(|e| Error::Oci(format!("the import config does not render: {e}")))?;
    let config_digest = Digest::of(&config_bytes);
    let manifest_value = serde_json::json!({
        "schemaVersion": 2,
        "mediaType": oci::MEDIA_OCI_MANIFEST,
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "digest": config_digest.to_string(),
            "size": config_bytes.len(),
        },
        "layers": [{
            "mediaType": MEDIA_UNTAR,
            "digest": layer_digest.to_string(),
            "size": layer_len,
        }],
    });
    let manifest_bytes = serde_json::to_vec(&manifest_value)
        .map_err(|e| Error::Oci(format!("the import manifest does not render: {e}")))?;
    let manifest_digest = Digest::of(&manifest_bytes);
    let manifest: oci::Manifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| Error::Oci(format!("the import manifest does not parse: {e}")))?;
    let config: oci::Config = serde_json::from_slice(&config_bytes)
        .map_err(|e| Error::Oci(format!("the import config does not parse: {e}")))?;
    store.put_bytes(&config_bytes, &config_digest, "import config")?;
    store.put_bytes(&manifest_bytes, &manifest_digest, "import manifest")?;
    // ⚠ The reference is a name, never an authority: `imported` with no tag
    // where the caller named nothing. `Reference::parse` applies docker's
    // `:latest` default centrally, so nothing here invents its own.
    let reference = match reference {
        Some(r) => Reference::parse(r)?,
        None => Reference::parse("imported")?,
    };
    let record = crate::store::record_of(
        &reference,
        &manifest_digest,
        oci::MEDIA_OCI_MANIFEST,
        &manifest_digest,
        &manifest,
        &config,
        &format!("{}/{}", config.os, config.architecture),
    )?;
    store.put_written(record.clone())?;
    Ok(record)
}

/// Stream a file into an open staging handle, returning its digest.
fn stream_file_hashed(path: &Path, out: &mut dyn Write) -> Result<Digest> {
    use sha2::Digest as _;
    let mut f = std::fs::File::open(path).map_err(|e| Error::io(path.display().to_string(), e))?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| Error::io(path.display().to_string(), e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n])
            .map_err(|e| Error::io(path.display().to_string(), e))?;
    }
    let hex = crate::digest::hex_of(&hasher.finalize());
    Ok(Digest::parse(&format!("sha256:{hex}"))?)
}
