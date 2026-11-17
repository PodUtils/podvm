//! The access of the caller to a file that a flag names (`--report`,
//! `--env-file`), where podvm runs as uid 0 of its own user namespace
//! (PodUtils/podvm#513, #552), out of `values.rs` for the limit of 500 lines.

use podvm_image::error::EXIT_FLAG_ERROR;

/// TODO/cli.md T-1825: `--report PATH`, a flag of `run`, whose directory
/// takes a file. A report that cannot be written is refused before the
/// payload runs: it printed a notice after the run and exited 0.
pub(in crate::run) fn report_path(verb: &str, path: &str) -> std::result::Result<(), i32> {
    super::values::report(verb)?;
    // PodUtils/podvm#513: an empty path ran the payload, then failed.
    if path.is_empty() {
        eprintln!("podvm {verb}: --report needs a path");
        return Err(EXIT_FLAG_ERROR);
    }
    let p = std::path::Path::new(path);
    let dir = p
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    // PodUtils/podvm#513: the writer makes a temp file in the directory and
    // renames it, so the directory is probed each time (`/dev/full` exists
    // and its directory takes no file); a path that exists is opened as
    // well, with nothing truncated. #552: the caller's own bits decide.
    let probe = dir.join(format!(
        ".podvm-report-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let dir_ok = std::fs::metadata(dir).is_ok_and(|m| caller_may(&m, 0o300));
    let opened: Result<bool, String> = if !dir_ok {
        Err(format!(
            "the directory {} takes no file of the caller",
            dir.display()
        ))
    } else if p.exists()
        && !std::fs::metadata(p).is_ok_and(|m| m.is_file() && caller_may(&m, 0o200))
    {
        Err(format!(
            "{path} is not a regular file that the caller may write"
        ))
    } else {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
            .map(|_| true)
            .map_err(|e| format!("the directory {} takes no file: {e}", dir.display()))
    };
    match opened {
        Ok(made) => {
            if made {
                let _ = std::fs::remove_file(&probe);
            }
            crate::run::report::note_path(path);
            Ok(())
        }
        Err(why) => {
            eprintln!("podvm {verb}: --report {path:?}: {why} (TODO/cli.md T-1825)");
            Err(podvm_image::error::EXIT_RUNTIME_ERROR)
        }
    }
}

/// PodUtils/podvm#552: in podvm's own user namespace the caller is uid 0,
/// and the kernel lets that uid pass the owner bits of the caller's own
/// files (a file of another user stays denied: it maps to no uid there).
/// podvm keeps the caller's rule: on a file of the caller, its owner `bits`
/// decide.
pub(super) fn caller_may(meta: &std::fs::Metadata, bits: u32) -> bool {
    use std::os::unix::fs::MetadataExt;
    !(crate::userns::active() && meta.uid() == 0) || meta.mode() & bits == bits
}
