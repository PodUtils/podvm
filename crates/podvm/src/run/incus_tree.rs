//! The image of the incus tier in the store, TODO/machine.md T-2085
//! (PodUtils/podrunner#347 ask 1).

/// TODO/machine.md T-2085 (PodUtils/podrunner#347 ask 1): the tree of the
/// incus tier in the store. The import streams its layers when possible.
/// A tree extracted as a fallback goes after the import where no other
/// process uses the image.
pub(super) struct IncusTree<'a> {
    pub(super) store: &'a podvm_image::Store,
    pub(super) held: &'a std::sync::Mutex<Option<podvm_image::store::Lock>>,
    pub(super) record: &'a podvm_image::Record,
    pub(super) loud: bool,
}

impl crate::incus::import::Tree for IncusTree<'_> {
    fn layers(&self) -> Result<Option<crate::incus::image::Layers>, String> {
        let d = podvm_image::digest::Digest::parse(&self.record.manifest_digest)
            .map_err(|e| e.to_string())?;
        let bytes = self.store.read_blob(&d).map_err(|e| e.to_string())?;
        let manifest = serde_json::from_slice(&bytes)
            .map_err(|e| format!("the manifest does not parse: {e}"))?;
        let blobs = podvm_extract::layers(self.store, &manifest).map_err(|e| e.to_string())?;
        Ok(Some(crate::incus::image::Layers {
            blobs,
            loud: self.loud,
        }))
    }

    fn ensure(&self) -> Result<bool, String> {
        let digest = &self.record.manifest_digest;
        if podvm_extract::is_extracted(self.store, digest) {
            return Ok(false);
        }
        super::extract_lined(self.store, self.record, self.loud).map_err(|(line, _)| line)?;
        Ok(true)
    }

    fn layer_bytes(&self) -> u64 {
        let Ok(d) = podvm_image::digest::Digest::parse(&self.record.manifest_digest) else {
            return 0;
        };
        self.store
            .read_blob(&d)
            .ok()
            .and_then(|b| serde_json::from_slice::<podvm_image::oci::Manifest>(&b).ok())
            .map_or(0, |m| {
                m.layers.iter().fold(0u64, |a, l| a.saturating_add(l.size))
            })
    }

    fn release(&self) -> Option<String> {
        // This run's own shared hold refuses the exclusive one: it goes
        // first; the instance needs no more of the store.
        if let Ok(mut h) = self.held.lock() {
            h.take();
        }
        let digest = &self.record.manifest_digest;
        let aside = match self.store.try_hold_tree_alone(digest) {
            // Another process uses the image (a run, a copy, an
            // extraction): the tree stays.
            Ok(None) => return None,
            Ok(Some(_locks)) => podvm_extract::put_aside(self.store, digest),
            Err(e) => {
                return Some(format!(
                    "the tree of the image stays in the store: its locks: {e}                      (TODO/machine.md T-2085)"
                ))
            }
        };
        match aside.map(|a| a.map(|p| (podvm_extract::remove_aside(&p), p))) {
            Ok(None) | Ok(Some((Ok(_), _))) => None,
            Ok(Some((Err(e), p))) => Some(format!(
                "the tree of the image could not be removed from {}: {e}; `podvm image \
                 prune` removes it (TODO/machine.md T-2085)",
                p.display()
            )),
            Err(e) => Some(format!(
                "the tree of the image stays in the store: {e} (TODO/machine.md T-2085)"
            )),
        }
    }
}
