//! TODO/cli.md T-2079 (OR-B39-04, OR-B39-14): where `podvm cp` writes, in
//! one place. docker's rule (the documentation of `docker cp`), in both
//! directions and for an image as the source:
//!
//! * SRC is a directory: DEST missing -> DEST is made and takes the
//!   contents of SRC; DEST a directory -> SRC goes into it as
//!   DEST/basename(SRC); SRC ends with `/.` -> the contents of SRC go into
//!   DEST; DEST a file -> refused.
//! * SRC is a file (or a symlink, copied as a link): DEST a directory ->
//!   DEST/basename(SRC); DEST a file -> DEST is written; DEST missing ->
//!   written as DEST, but refused where DEST ends with `/`.
//! * DEST missing: its parent must be a directory, or the copy is refused.

use std::path::{Path, PathBuf};

/// What the source is. A symlink copies as a link, so it is a `File` here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Src {
    File,
    Dir,
}

/// What is at the destination, read before the copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Dest {
    /// Nothing is there; `parent_dir` says whether its parent is a directory.
    Missing {
        parent_dir: bool,
    },
    Directory,
    /// A file or another object that is not a directory.
    NotDir,
}

/// The question: the kind of the source, whether its name ends with `/.`
/// (or is `.`), its basename, what is at the destination, and whether the
/// name of the destination ends with `/`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Ask<'a> {
    pub(super) src: Src,
    pub(super) contents: bool,
    pub(super) name: Option<&'a std::ffi::OsStr>,
    pub(super) dest: Dest,
    pub(super) slash: bool,
}

/// Why a copy is refused, before a byte lands. Each refusal exits 1
/// (T-2062: a refusal after the parse).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    /// `cp FILE DEST/` where DEST is missing.
    NoDirAtSlash,
    /// `cp FILE DEST/` where DEST is a file.
    FileAtSlash,
    /// `cp -r DIR FILE`.
    DirOntoFile,
    /// `cp FILE/. DEST`: a file has no contents to take.
    FileContents,
    /// DEST is missing and its parent is not a directory.
    NoParent,
}

impl Refusal {
    /// The one true line of the refusal, after `podvm cp: `.
    pub(super) fn line(self, dest: &str) -> String {
        match self {
            Self::NoDirAtSlash => {
                format!("{dest} ends with '/', which names a directory, and no directory is there")
            }
            Self::FileAtSlash => {
                format!("{dest} ends with '/', which names a directory, and it is not a directory")
            }
            Self::DirOntoFile => {
                format!("{dest} exists and is not a directory; a directory cannot copy onto it")
            }
            Self::FileContents => {
                "the source ends with '/.', which names the contents of a directory, and it \
                 is not a directory"
                    .to_string()
            }
            Self::NoParent => format!(
                "the parent directory of {dest} does not exist; podvm makes no parent, as docker"
            ),
        }
    }
}

/// The path that the copy writes: the file, or the top of the tree whose
/// contents are the contents of SRC (made where missing, merged where
/// present). `dest` is the resolved destination.
pub(super) fn target(ask: Ask<'_>, dest: &Path) -> Result<PathBuf, Refusal> {
    use Dest::*;
    use Src::*;
    let into = |dest: &Path| match ask.name {
        Some(n) => dest.join(n),
        // The root of a filesystem has no name: its contents go into DEST.
        None => dest.to_path_buf(),
    };
    match (ask.src, ask.contents, ask.dest, ask.slash) {
        (File, true, _, _) => Err(Refusal::FileContents),
        (_, _, Missing { parent_dir: false }, _) => Err(Refusal::NoParent),
        (Dir, _, NotDir, _) => Err(Refusal::DirOntoFile),
        (Dir, false, Directory, _) => Ok(into(dest)),
        (Dir, true, Directory, _) | (Dir, _, Missing { .. }, _) => Ok(dest.to_path_buf()),
        (File, false, Directory, _) => Ok(into(dest)),
        (File, false, Missing { .. }, true) => Err(Refusal::NoDirAtSlash),
        (File, false, NotDir, true) => Err(Refusal::FileAtSlash),
        (File, false, Missing { .. } | NotDir, false) => Ok(dest.to_path_buf()),
    }
}

/// A source name that ends with `/.` (or is `.`) names the contents of a
/// directory, as docker reads it.
pub(super) fn names_contents(src: &str) -> bool {
    src == "." || src.ends_with("/.")
}

/// A destination name that ends with `/` (or `/.`) names a directory.
pub(super) fn names_dir(dest: &str) -> bool {
    dest.ends_with('/') || dest.ends_with("/.")
}

/// What is at `dest` now. A symlink is followed, as before: the write
/// through a link is refused later by name.
pub(super) fn dest_of(dest: &Path) -> Dest {
    match std::fs::metadata(dest) {
        Ok(m) if m.is_dir() => Dest::Directory,
        Ok(_) => Dest::NotDir,
        // A relative name of one part has the empty parent: the cwd.
        Err(_) => Dest::Missing {
            parent_dir: match dest.parent() {
                Some(p) if p.as_os_str().is_empty() => Path::new(".").is_dir(),
                Some(p) => p.is_dir(),
                None => false,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// docker's table of `docker cp`, row by row. The rule has no
    /// direction: `copy_one` gives it the resolved paths of each side.
    #[test]
    fn the_destination_follows_the_rule_of_docker_cp() {
        use Dest::*;
        use Src::*;
        let gone = Missing { parent_dir: true };
        let orphan = Missing { parent_dir: false };
        let d = Path::new("/d");
        let as_dest = Ok(PathBuf::from("/d"));
        let inside = Ok(PathBuf::from("/d/s"));
        type Row = (Src, bool, Dest, bool, Result<PathBuf, Refusal>);
        #[rustfmt::skip]
        let rows: &[Row] = &[
            // SRC a directory.
            (Dir,  false, gone,      false, as_dest.clone()),
            (Dir,  false, gone,      true,  as_dest.clone()),
            (Dir,  false, Directory, false, inside.clone()),
            (Dir,  false, Directory, true,  inside.clone()),
            (Dir,  false, NotDir,    false, Err(Refusal::DirOntoFile)),
            (Dir,  false, orphan,    false, Err(Refusal::NoParent)),
            // SRC/. : the contents.
            (Dir,  true,  gone,      false, as_dest.clone()),
            (Dir,  true,  Directory, false, as_dest.clone()),
            (Dir,  true,  Directory, true,  as_dest.clone()),
            (Dir,  true,  NotDir,    false, Err(Refusal::DirOntoFile)),
            (Dir,  true,  orphan,    true,  Err(Refusal::NoParent)),
            // SRC a file.
            (File, false, Directory, false, inside.clone()),
            (File, false, Directory, true,  inside.clone()),
            (File, false, NotDir,    false, as_dest.clone()),
            (File, false, NotDir,    true,  Err(Refusal::FileAtSlash)),
            (File, false, gone,      false, as_dest.clone()),
            (File, false, gone,      true,  Err(Refusal::NoDirAtSlash)),
            (File, false, orphan,    false, Err(Refusal::NoParent)),
            (File, true,  Directory, false, Err(Refusal::FileContents)),
        ];
        for (src, contents, dest, slash, want) in rows {
            let ask = Ask {
                src: *src,
                contents: *contents,
                name: Some(std::ffi::OsStr::new("s")),
                dest: *dest,
                slash: *slash,
            };
            assert_eq!(&target(ask, d), want, "{ask:?}");
        }
        // The root of a filesystem has no name: its contents go into DEST.
        let root = Ask {
            src: Dir,
            contents: false,
            name: None,
            dest: Directory,
            slash: false,
        };
        assert_eq!(target(root, d), as_dest);
        for (s, c) in [
            (".", true),
            ("a/.", true),
            ("a", false),
            ("a/", false),
            ("a/..", false),
        ] {
            assert_eq!(names_contents(s), c, "{s}");
        }
        for (s, c) in [("a/", true), ("a/.", true), ("a", false), (".", false)] {
            assert_eq!(names_dir(s), c, "{s}");
        }
    }
}
