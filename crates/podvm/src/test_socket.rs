//! TODO/gate.md T-2067: the directory of a unix socket of a stand-in in a
//! test (incus, the monitor of QEMU, the serial port).
//!
//! ⛔ Not under `TMPDIR`: a long `TMPDIR` makes a socket path longer than
//! `sun_path`, and the bind fails as uid 1000 with the `TMPDIR` of the
//! method of `docs/HANDOFF.md`.

use std::path::PathBuf;

/// The length of `sun_path` of `struct sockaddr_un`, with its NUL.
const SUN_PATH: usize = 108;

/// A new directory under `/tmp`, mode 0700, that goes at the drop (also
/// after a panic). Its name has 16 random hex digits, not a pid (a pid is
/// not unique across PID namespaces), and `create_dir` refuses one that
/// exists.
pub(crate) struct SocketDir(PathBuf);

impl SocketDir {
    pub(crate) fn new(tag: &str) -> SocketDir {
        use std::os::unix::fs::DirBuilderExt;
        let id = podvm_supervise::new_id();
        let short: String = id.chars().take(16).collect();
        let d = PathBuf::from(format!("/tmp/podvm-{tag}-{short}"));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&d)
            .unwrap_or_else(|e| panic!("{}: {e}", d.display()));
        SocketDir(d)
    }

    /// The path of the socket `name` in the directory; it fits `sun_path`.
    pub(crate) fn join(&self, name: &str) -> PathBuf {
        let p = self.0.join(name);
        assert!(p.as_os_str().len() < SUN_PATH, "{}", p.display());
        p
    }
}

impl Drop for SocketDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
