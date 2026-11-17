//! The files and the identity of one run: the guard of the memo of a run
//! that `prepare` refuses (PodUtils/podvm#475), and the uid that a payload
//! runs as (TODO/gate.md T-1948).

/// The effective uid of this process: the uid that a payload of the
/// userland rung, of the syscall tier and of a chroot runs as (TODO/gate.md
/// T-1948, PodUtils/podvm#507). The owner of `/proc/self`; 0 where it cannot
/// be read, which is the answer of the old text.
pub fn own_uid() -> u32 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").map_or(0, |m| m.uid())
}

/// PodUtils/podvm#475: the memo of a run that `prepare` refuses after it
/// made the memo (a `--strict=all` refusal, a failed entry decision). The
/// guard removes the file when it drops, unless [`MemoGuard::keep`] ran.
pub struct MemoGuard(Option<std::path::PathBuf>);

impl MemoGuard {
    pub fn new(path: &std::path::Path) -> Self {
        Self(Some(path.to_path_buf()))
    }
    /// The run goes on: the memo stays.
    pub fn keep(mut self) {
        self.0 = None;
    }
}

impl Drop for MemoGuard {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            let _ = std::fs::remove_file(p);
        }
    }
}
