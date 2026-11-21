//! The exit code of a refused archive of `load` and `import`.

use podvm_image::error::EXIT_CLI_ERROR;

/// TODO/cli.md T-2062: the code of a failure to read the archive that the
/// caller named. A refusal of its content or of its file is a refusal after
/// the parse (`EXIT_CLI_ERROR`); a failure of the store is podvm's (125).
pub(super) fn archive_exit(e: &podvm_image::Error, archive: &std::path::Path) -> i32 {
    use podvm_image::Error as E;
    match e {
        E::Oci(_) | E::DigestMismatch { .. } => EXIT_CLI_ERROR,
        E::Io { path, .. } if std::path::Path::new(path) == archive => EXIT_CLI_ERROR,
        other => other.exit_code(),
    }
}
