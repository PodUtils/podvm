//! `TOOL.md` section 6.3: layers, whiteouts, ownership sidecar, path safety.
//! Milestone M2, [`TODO/milestones.md`](../../../TODO/milestones.md) T-1103 and
//! [`TODO/extract.md`](../../../TODO/extract.md) T-0301 to T-0307.
//!
//! `pod-extract` (PodUtils/podcrates) applies the layers: the entry-level
//! extraction, the ownership sidecar, the basename whiteouts, the
//! containment, the rebased symlink targets, the owner bits between layers,
//! the whiteouts before their layer's entries, the digest of each blob while
//! it streams (T-1315) and the completion marker. This crate gives it
//! podvm's store: the layers of a manifest as blob paths, the layout of an
//! image's rootfs under the store, and podvm's settings (`$PODVM_STORE` in
//! the space message).
//!
//! ⛔ **The sidecar changes no kernel permission check.** A file recorded as
//! `gid 42` is owned on disk by the id that extracted it, and every access
//! check the kernel makes uses that one.

#![forbid(unsafe_op_in_unsafe_fn)]

pub mod error;
pub mod live;
pub mod tree;

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use podvm_image::{oci, Store};

pub use error::{Error, Result};
pub use pod_extract::{
    apply, flatten, layer, refused, remove, safety, sidecar, whiteout, Extracted, Layer,
    DONE_MARKER, EXTRACT_HEADROOM_BYTES,
};

/// Where an image's rootfs and its sidecar live, given the store root.
///
/// ⛔ The sidecar sits BESIDE the rootfs, not inside it. A file inside the
/// rootfs is a file the payload can read, write and be confused by, and
/// T-0302's `Prove` reads it at `<rootfs>/../.meta.jsonl` for that reason.
pub fn paths(store: &Store, digest: &str) -> (PathBuf, PathBuf) {
    let p = place(store, digest);
    (p.rootfs, p.sidecar)
}

/// The rootfs, the sidecar and the completion marker of an image, under
/// `<store>/rootfs/<digest>/`.
fn place(store: &Store, digest: &str) -> pod_extract::Place {
    pod_extract::Place::under(&store.root().join("rootfs").join(digest.replace(':', "_")))
}

/// Extract every layer of `manifest` into the store, in order.
///
/// The space precheck (T-0203's second call site) is in `pod_extract::extract`,
/// before its per-layer loop. Each layer's line (`<digest>: Extracted (N
/// entries, M whiteouts)`) reaches `out` when the extraction ends, in order,
/// also when it fails.
pub fn extract(
    store: &Store,
    manifest: &oci::Manifest,
    digest: &str,
    out: &mut dyn Write,
) -> Result<Extracted> {
    let layers = layers(store, manifest)?;
    let lines = Arc::new(Mutex::new(Vec::new()));
    let keep = Arc::clone(&lines);
    let settings = pod_extract::Settings {
        host: podvm_probe::settings(),
        report: pod_core::Report {
            events: Arc::new(move |e: pod_core::Event| {
                if let Ok(mut l) = keep.lock() {
                    l.push(e.text);
                }
            }),
            ..pod_core::Report::default()
        },
        ..pod_extract::Settings::default()
    };
    let done = pod_extract::extract(&settings, digest, &layers, &place(store, digest));
    if let Ok(l) = lines.lock() {
        for line in l.iter() {
            let _ = writeln!(out, "{line}");
        }
    }
    Ok(done?)
}

/// The layers of a manifest in order, with their paths in this store.
pub fn layers(store: &Store, manifest: &oci::Manifest) -> Result<Vec<Layer>> {
    let mut layers = Vec::new();
    for d in &manifest.layers {
        // `?`: the error of the digest is the store's or `pod-oci`'s, and
        // `Error` takes both.
        let parsed = d.parsed_digest()?;
        layers.push(Layer {
            digest: parsed.to_string(),
            media_type: d.media_type.clone(),
            size: d.size,
            blob: store.blob_path(&parsed),
        });
    }
    Ok(layers)
}

/// Has this image already been extracted, completely?
///
/// ⛔ The marker, not the directory. See [`DONE_MARKER`].
pub fn is_extracted(store: &Store, digest: &str) -> bool {
    place(store, digest).is_extracted()
}

/// Remove an extracted rootfs and its sidecar.
///
/// ⛔ The removal never follows a symlink out of the tree it is clearing:
/// this tree was written from a hostile archive.
pub fn remove_extracted(store: &Store, digest: &str) -> Result<bool> {
    let p = place(store, digest);
    let Some(base) = p.rootfs.parent() else {
        return Ok(false);
    };
    Ok(pod_extract::remove_tree(base)?)
}

/// TODO/machine.md T-2085 (PodUtils/podrunner#347 ask 1): put the
/// extraction of `digest` aside with one rename, beside it under a name
/// that starts with [`tree::TRASH`], so that no reader finds a part of it.
/// The caller holds the locks of `Store::try_hold_tree_alone` across this
/// call, and removes the path with [`remove_aside`] after them. `None`
/// where there is no extraction.
pub fn put_aside(store: &Store, digest: &str) -> Result<Option<PathBuf>> {
    let p = place(store, digest);
    let Some(base) = p.rootfs.parent() else {
        return Ok(None);
    };
    if std::fs::symlink_metadata(base).is_err() {
        return Ok(None);
    }
    let io =
        |e: std::io::Error| Error::Image(podvm_image::Error::io(base.display().to_string(), e));
    let aside = base.with_file_name(format!("{}{}", tree::TRASH, tree::new_id().map_err(io)?));
    std::fs::rename(base, &aside).map_err(io)?;
    Ok(Some(aside))
}

/// Remove a tree that [`put_aside`] put aside. The removal never follows a
/// symlink out of it.
pub fn remove_aside(path: &std::path::Path) -> Result<bool> {
    Ok(pod_extract::remove_tree(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sidecar_sits_beside_the_rootfs_not_inside_it() {
        // TODO/gate.md T-1859: a name of this process, removed at the end.
        let d = std::env::temp_dir().join(format!("podvm-store-paths-{}", std::process::id()));
        let s = Store::open(&d).unwrap();
        let (rootfs, side) = paths(&s, "sha256:abc123");
        let _ = std::fs::remove_dir_all(&d);
        // T-0302's Prove reads `<rootfs>/../.meta.jsonl`.
        assert_eq!(rootfs.parent().unwrap().join(".meta.jsonl"), side);
        // ⛔ A colon is not a portable path character and the digest carries
        // one.
        assert!(!rootfs.to_string_lossy().contains(':'));
    }

    /// TODO/machine.md T-2085: one rename puts the extraction aside, under
    /// the trash prefix beside it, and the removal takes it away.
    #[test]
    fn an_extraction_put_aside_is_gone_with_one_rename() {
        let d = std::env::temp_dir().join(format!("podvm-store-aside-{}", std::process::id()));
        let s = Store::open(&d).unwrap();
        assert!(put_aside(&s, "sha256:abc").unwrap().is_none(), "no tree");
        let (rootfs, side) = paths(&s, "sha256:abc");
        std::fs::create_dir_all(rootfs.join("etc")).unwrap();
        std::fs::write(rootfs.join("etc/passwd"), b"root").unwrap();
        std::fs::write(&side, b"{}").unwrap();
        let aside = put_aside(&s, "sha256:abc").unwrap().expect("put aside");
        assert!(
            !rootfs.exists() && !side.exists(),
            "the tree is gone at once"
        );
        assert_eq!(aside.parent(), rootfs.parent().and_then(|p| p.parent()));
        let name = aside.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with(tree::TRASH), "{name}");
        assert!(aside.join("rootfs/etc/passwd").exists());
        assert!(remove_aside(&aside).unwrap());
        assert!(!aside.exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
