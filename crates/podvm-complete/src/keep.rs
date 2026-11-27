//! The `/etc/hosts` of a run, kept across the completion of an `exec`
//! (TODO/cli.md T-2079, OR-B39-15).
//!
//! The run writes `/etc/hosts` once, at its start. `pod-rootfs` writes it
//! again at each completion, and an `exec` completes the root of the run
//! again, so the edits of the payload to the file were lost at the next
//! `exec`. [`Held::take`] reads the file before the completion, and
//! [`Held::put_back`] gives its bytes back after it; the row of the
//! completion then says that the file was kept.
//!
//! ⚠ `pod-rootfs` replaces the file first (an unlink, then a create), so for
//! a short time the file holds the lines that podvm generates. The remedy is
//! a setting of the crate that leaves a file as it is (the ask is in
//! TODO/cli.md T-2079); this module then goes.

use std::path::Path;

use pod_rootfs::write::{Kind, Root};

use super::{Action, Fixup};

const REL: &str = "etc/hosts";

/// The file as the run or the payload left it.
pub(crate) struct Held {
    bytes: Vec<u8>,
    mode: u32,
}

impl Held {
    /// The regular file `etc/hosts` of `rootfs`, read with no follow of the
    /// last name and no way out of the root; `None` where it is not a
    /// regular file that can be read (the completion then writes it).
    pub(crate) fn take(rootfs: &Path) -> Option<Held> {
        let root = Root::open(rootfs).ok()?;
        let Ok(Kind::Regular { mode, .. }) = root.kind(REL) else {
            return None;
        };
        let bytes = root.read(REL).ok()??;
        Some(Held { bytes, mode })
    }

    /// Give the bytes back where the completion changed them, and change
    /// the row of the file. A failed write is a degraded row, never a
    /// silent loss.
    pub(crate) fn put_back(self, rootfs: &Path, rows: &mut [Fixup]) {
        let row = rows.iter_mut().find(|f| f.id == "hosts");
        let done = Root::open(rootfs)
            .and_then(|r| r.write(REL, &self.bytes, self.mode))
            .map(|_| ())
            .map_err(|e| e.chain());
        let Some(row) = row else { return };
        match done {
            Ok(()) => {
                row.action = Action::Unchanged;
                row.detail = "kept as the run and its payload left it (the run wrote it at its \
                              start)"
                    .to_string();
                row.degraded = false;
            }
            Err(why) => {
                row.action = Action::Failed;
                row.detail = format!("the file of the payload could not be put back: {why}");
                row.degraded = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("podvm-keep-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("etc")).unwrap();
        d
    }

    /// TODO/cli.md T-2079 (OR-B39-15): a second completion with the file
    /// kept leaves the edits of the payload; one table of the files that
    /// the first completion finds.
    #[test]
    fn a_kept_hosts_file_survives_the_completion() {
        // (the file before, the file after the completion with `keep`)
        let rows: [(Option<&str>, &str, bool); 3] = [
            (
                Some("127.0.0.1 localhost\n10.0.0.9 edited\n"),
                "10.0.0.9 edited",
                true,
            ),
            (Some(""), "", true),
            // No file: the completion writes it, and there is nothing to keep.
            (None, "localhost", false),
        ];
        for (n, (before, want, kept)) in rows.iter().enumerate() {
            let d = root(&n.to_string());
            if let Some(b) = before {
                std::fs::write(d.join(REL), b).unwrap();
            }
            let held = Held::take(&d);
            assert_eq!(held.is_some(), *kept, "{before:?}");
            let opts = super::super::Options::default();
            let mut r = super::super::complete(d.to_str().unwrap(), &opts).unwrap();
            if let Some(h) = held {
                h.put_back(&d, &mut r.fixups);
                let row = r.fixups.iter().find(|f| f.id == "hosts").unwrap();
                assert_eq!(row.action, Action::Unchanged, "{row:?}");
                assert!(!row.degraded);
            }
            let got = std::fs::read_to_string(d.join(REL)).unwrap();
            assert!(got.contains(want), "{before:?}: {got:?}");
            if *kept {
                assert_eq!(&got, before.as_ref().unwrap());
            }
            let _ = std::fs::remove_dir_all(&d);
        }
        // A link is not a file to keep.
        let d = root("link");
        std::os::unix::fs::symlink("/etc/passwd", d.join(REL)).unwrap();
        assert!(Held::take(&d).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
