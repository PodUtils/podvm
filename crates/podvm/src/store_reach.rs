//! TODO/cli.md T-2108 (OR-B42-08): what other users can read of the store.
//!
//! TODO/supervise.md T-1715: the container table keeps each `-e` value, so
//! a store that other users can read gives them. The mode of the root alone
//! does not say that: a root of mode 755 lets other users list its names
//! and walk through it, but the table (0600) and `containers/` (0700) that
//! podvm made stay closed. The warning names the real modes of the root and
//! of the parts that hold the values, and it speaks only where one of them
//! is open.

use std::path::Path;

use podvm_supervise::table::{CONTAINERS_DIR, TABLE_FILE};

/// The modes (permission bits) of the store root and of the two parts that
/// hold the values of `-e`. `None` is a part that does not exist yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reach {
    pub(crate) root: u32,
    pub(crate) table: Option<u32>,
    pub(crate) containers: Option<u32>,
}

impl Reach {
    /// The modes of a store whose root group or other can reach
    /// (`Store::exposed_mode` is `Some`); `None` where only the owner can.
    pub(crate) fn of(store: &podvm_image::Store) -> Option<Reach> {
        let root = store.exposed_mode()?;
        Some(Reach::read(store.root(), root))
    }

    fn read(dir: &Path, root: u32) -> Reach {
        use std::os::unix::fs::PermissionsExt;
        let mode = |name: &str| {
            std::fs::symlink_metadata(dir.join(name))
                .ok()
                .map(|m| m.permissions().mode() & 0o777)
        };
        Reach {
            root,
            table: mode(TABLE_FILE),
            containers: mode(CONTAINERS_DIR),
        }
    }

    /// The parts that another user can read: the root must let a user
    /// walk through it (`x` for group or other), and the part must let one
    /// read it (`r` for the table; `r` and `x` for the directory).
    fn open(&self) -> Vec<(&'static str, &'static str, u32)> {
        let mut parts = Vec::new();
        if self.root & 0o011 == 0 {
            return parts;
        }
        if let Some(m) = self.table
            && m & 0o044 != 0
        {
            parts.push(("the container table", "which keeps each -e value", m));
        }
        if let Some(m) = self.containers
            && m & 0o055 != 0
        {
            parts.push(("containers/", "which holds the logs and records", m));
        }
        parts
    }

    /// The true sentence after "the store root X is": the modes, and what
    /// other users can read. `None` where no part that holds a value is open.
    pub(crate) fn warning(&self) -> Option<String> {
        let open = self.open();
        if open.is_empty() {
            return None;
        }
        let modes: Vec<String> = open
            .iter()
            .map(|(name, _, m)| format!("{name} is mode {m:o}"))
            .collect();
        let what: Vec<String> = open
            .iter()
            .map(|(name, why, _)| format!("{name}, {why}"))
            .collect();
        Some(format!(
            "mode {:o} and {}: other users can read {}",
            self.root,
            modes.join(" and "),
            what.join(" and ")
        ))
    }

    /// The row of `podvm doctor`: whether it is a note (a part is open), and
    /// its text.
    pub(crate) fn row(&self, root: &Path) -> (bool, String) {
        match self.warning() {
            Some(words) => (
                true,
                format!(
                    "{} is {words}. Run chmod 700 {} (TODO/supervise.md T-1715)",
                    root.display(),
                    root.display()
                ),
            ),
            None => (false, format!("{} is {}", root.display(), self.closed())),
        }
    }

    /// The sentence for a root that group or other can reach where no part
    /// is open: the modes that close it.
    pub(crate) fn closed(&self) -> String {
        let show = |name: &str, m: Option<u32>| match m {
            Some(m) => format!("{name} is mode {m:o}"),
            None => format!("{name} does not exist yet"),
        };
        format!(
            "mode {:o}; {} and {}, so other users can list the root but cannot read \
             the container table",
            self.root,
            show("the container table", self.table),
            show("containers/", self.containers)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One table of the modes of the root, the table and `containers/`, and
    /// what the warning says of each: it speaks only where a part that holds
    /// a value is open, and it names the real modes.
    #[test]
    fn the_warning_follows_the_modes_of_the_parts_and_not_the_root_alone() {
        let r = |root, table, containers| Reach {
            root,
            table,
            containers,
        };
        // (reach, the words that the warning holds; none for no warning)
        let rows: &[(Reach, Option<&[&str]>)] = &[
            // The report of OR-B42-08: root 755, files 600 and 700.
            (r(0o755, Some(0o600), Some(0o700)), None),
            (r(0o755, None, None), None),
            (
                r(0o755, Some(0o644), Some(0o700)),
                Some(&[
                    "mode 755",
                    "the container table is mode 644",
                    "each -e value",
                ]),
            ),
            (
                r(0o755, Some(0o600), Some(0o755)),
                Some(&["containers/ is mode 755", "logs and records"]),
            ),
            (
                r(0o755, Some(0o644), Some(0o755)),
                Some(&["table is mode 644", "containers/ is mode 755"]),
            ),
            // A root that other users cannot walk through hides open parts
            // (0o744: group and other may list the root, not enter it).
            (r(0o744, Some(0o644), Some(0o755)), None),
            (
                r(0o750, Some(0o640), Some(0o700)),
                Some(&["table is mode 640"]),
            ),
        ];
        for (reach, want) in rows {
            let got = reach.warning();
            match (got, want) {
                (None, None) => {}
                (Some(line), Some(words)) => {
                    for w in *words {
                        assert!(line.contains(w), "{reach:?}: {line}");
                    }
                }
                (got, want) => panic!("{reach:?}: {got:?} against {want:?}"),
            }
        }
        let closed = r(0o755, Some(0o600), Some(0o700)).closed();
        assert!(closed.contains("table is mode 600"), "{closed}");
        assert!(closed.contains("containers/ is mode 700"), "{closed}");
    }

    /// The modes are read from the files that podvm made, and a store of
    /// mode 755 whose parts are private gives no warning.
    #[test]
    fn a_store_of_mode_755_with_private_parts_gives_no_warning() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("podvm-reach-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(CONTAINERS_DIR)).unwrap();
        std::fs::write(dir.join(TABLE_FILE), "[]").unwrap();
        let set = |p: &Path, m| std::fs::set_permissions(p, std::fs::Permissions::from_mode(m));
        set(&dir.join(TABLE_FILE), 0o600).unwrap();
        set(&dir.join(CONTAINERS_DIR), 0o700).unwrap();
        let reach = Reach::read(&dir, 0o755);
        assert_eq!(reach.table, Some(0o600));
        assert_eq!(reach.containers, Some(0o700));
        assert_eq!(reach.warning(), None);
        set(&dir.join(TABLE_FILE), 0o644).unwrap();
        assert!(Reach::read(&dir, 0o755).warning().is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
