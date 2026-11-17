//! `TODO/image.md`: can a file created under the store execute (`noexec`).
//!
//! `pod_hostprobe::exec` makes the probe. This module gives it podvm's
//! settings, so a denial names `$PODVM_STORE`, and turns its failures into
//! this crate's errors.

use std::path::Path;

pub use pod_hostprobe::exec::{ExecProbe, ExecVerdict};

use crate::error::Result;

/// Probe whether files created under `dir` execute. The probe file is
/// removed on every path.
///
/// PodUtils/podvm#501: the probe names its file by the process id, and each
/// foreground run is pid 2 in a PID namespace of its own (TODO/probe.md
/// T-1909), so two runs at once wrote, ran and removed one file, and one
/// failed with ENOENT. The probe runs in a new directory of `dir` that
/// `mkdir` makes for this process alone, on the same mount.
pub fn probe_store_exec(dir: &Path) -> Result<ExecProbe> {
    let Some(own) = own_dir(dir) else {
        return pod_hostprobe::exec::probe_dir_exec(&podvm_probe::settings(), dir)
            .map_err(crate::space::host_error);
    };
    let r = pod_hostprobe::exec::probe_dir_exec(&podvm_probe::settings(), &own);
    let _ = std::fs::remove_dir_all(&own);
    let mut p = r.map_err(crate::space::host_error)?;
    p.detail = p
        .detail
        .replace(&own.display().to_string(), &dir.display().to_string());
    p.dir = dir.to_path_buf();
    Ok(p)
}

/// A new directory under `dir` that no other process has, or `None` where
/// none can be made (the probe then names `dir` itself and its error).
fn own_dir(dir: &Path) -> Option<std::path::PathBuf> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    (0..16u32).find_map(|i| {
        let d = dir.join(format!(".podvm-exec-probe-{nanos:x}-{i}"));
        std::fs::create_dir(&d).ok().map(|()| d)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_directory_that_cannot_be_written_is_an_io_error_with_its_path() {
        let d = std::env::temp_dir()
            .join(format!("podvm-execprobe-{}", std::process::id()))
            .join("does-not-exist");
        let err = probe_store_exec(&d).unwrap_err();
        assert!(
            matches!(&err, crate::Error::Io { path, source }
                if path.starts_with(&*d.to_string_lossy())
                    && source.kind() == std::io::ErrorKind::NotFound),
            "{err:?}"
        );
    }
}
