//! An upload of the image that incusd cuts, TODO/machine.md T-2082
//! (PodUtils/podrunner#347 ask 2): SIGPIPE ignored during the upload, the
//! class of the error, and the line with the free space of incusd's images
//! and of the pool. Out of [`super::space`] (T-2085), which holds the check
//! before the import.

use std::os::unix::fs::MetadataExt;

use pod_hostprobe::space::mib;

use super::space::{incus_dir, room_at, segment, Plan};

/// TODO/machine.md T-2082 (PodUtils/podrunner#347 ask 2): SIGPIPE ignored
/// while it lives, then back to the action before it (podvm's default,
/// T-1769). A write to a socket that incusd closed then gives EPIPE.
pub struct NoSigpipe(usize);

unsafe extern "C" {
    fn signal(sig: i32, handler: usize) -> usize;
}
const SIGPIPE: i32 = 13;
const SIG_ERR: usize = usize::MAX;

impl NoSigpipe {
    pub fn now() -> NoSigpipe {
        // SAFETY: SIG_IGN (1) for one signal; no handler runs.
        let old = unsafe { signal(SIGPIPE, 1) };
        NoSigpipe(if old == SIG_ERR { 0 } else { old })
    }
}

impl Drop for NoSigpipe {
    fn drop(&mut self) {
        // SAFETY: the action that was there before (SIG_DFL or SIG_IGN).
        unsafe { signal(SIGPIPE, self.0) };
    }
}

/// True where `e` is a connection that incusd closed while podvm wrote
/// (EPIPE, a reset).
pub fn cut(e: &pod_incus::Error) -> bool {
    matches!(e.kind(), pod_incus::ErrorKind::Io(io) if matches!(
        io.kind(),
        std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
    ))
}

/// The line after an upload of the image that incusd cut (`cut`) or
/// refused with "no space left on device", for any driver: what incusd
/// does, and the free space of its images and of the pool of the default
/// profile where podvm can read them, with the need.
pub async fn after_cut(c: &pod_incus::Client, plan: Option<&Plan>, cut: bool) -> Option<String> {
    let dir = incus_dir(&c.settings().socket)?;
    let mut rooms = Vec::new();
    let images = dir.join("images");
    let images_dev = std::fs::metadata(&images).ok().map(|m| m.dev());
    if let Some(r) = room_at(&images) {
        // T-2085: the upload is gzip; its estimate is the bytes of the
        // layers.
        let need = match plan {
            Some(p) if p.gzip => format!("an estimated {} (gzip)", mib(p.upload)),
            Some(p) => format!("at least {}", mib(p.tree.bytes)),
            None => "-".to_string(),
        };
        rooms.push(format!(
            "incusd's images at {} have {} free, and the upload needs {need}",
            images.display(),
            mib(r.bytes),
        ));
    }
    let pool = match c.profile("default").await {
        Ok(p) => p.devices.get("root").and_then(|d| d.get("pool")).cloned(),
        Err(_) => None,
    };
    if let Some(name) = pool.filter(|n| segment(n)) {
        let path = dir.join("storage-pools").join(&name);
        let dev = std::fs::metadata(&path).ok().map(|m| m.dev());
        if let (Some(d), Some(r)) = (dev, room_at(&path))
            && Some(d) != images_dev
        {
            rooms.push(format!(
                "the storage pool `{name}` at {} has {} free",
                path.display(),
                mib(r.bytes)
            ));
        }
    }
    let rooms = if rooms.is_empty() {
        "podvm can read the free space of neither".to_string()
    } else {
        rooms.join("; ")
    };
    let what = if cut {
        "incusd closed the connection during the upload of the image: incusd stops reading \
         where it cannot store the upload (a full disk: \"no space left on device\" in its \
         log), and its answer then reaches no reader"
    } else {
        "incusd has no room for the upload of the image"
    };
    Some(format!(
        "{what}; {rooms} (TODO/machine.md T-2082, PodUtils/podrunner#347)"
    ))
}

/// True where incusd's error of an import says that a disk is full.
pub fn no_space(e: &pod_incus::Error) -> bool {
    // tar writes "No space left on device"; incusd's own words are lower
    // case.
    e.incus_error()
        .is_some_and(|w| w.to_ascii_lowercase().contains("no space left on device"))
}
