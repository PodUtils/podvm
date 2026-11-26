//! The exit code of a refused archive of `load` and `import`.

use podvm_image::error::EXIT_CLI_ERROR;

/// The name of the archive of `load` from stdin in a message. The spool of
/// the store that holds it is podvm's, and the user did not name it
/// (TODO/cli.md T-2100, OR-B41-17).
pub(super) const STDIN_ARCHIVE: &str = "the archive on stdin";

/// A writer that keeps whether a write or a flush to it failed: `save` to
/// stdout then gives one line that names stdout, and the check of the exit
/// adds none (TODO/cli.md T-1816, T-2100).
pub(super) struct Watched<W> {
    inner: W,
    pub(super) failed: bool,
}

impl<W> Watched<W> {
    pub(super) fn new(inner: W) -> Self {
        Watched {
            inner,
            failed: false,
        }
    }
}

impl<W: std::io::Write> std::io::Write for Watched<W> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        let r = self.inner.write(b);
        self.failed |= r.is_err();
        r
    }
    fn flush(&mut self) -> std::io::Result<()> {
        let r = self.inner.flush();
        self.failed |= r.is_err();
        r
    }
}

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
