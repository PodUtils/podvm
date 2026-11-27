//! TODO/cli.md T-2108 (OR-B42-06): `cp - NAME:PATH`, the tar stream of stdin
//! unpacked into a directory of a container. The rules are in the module
//! doc of `cp_tar.rs`: the name stays below the destination, no link on the
//! way, no special file, the opens through `pod-beneath`.

use std::ffi::OsStr;
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

use super::{broke, refuse, Fail, MAX_DIRS};
use crate::lifecycle::*;

/// What a name of the archive is below the destination: the normal parts
/// only. A `..`, a name with nothing in it and a name that is not under
/// the destination are refused; a leading `/` and `.` parts are dropped.
fn below(raw: &[u8]) -> Result<Option<PathBuf>, String> {
    let mut out = PathBuf::new();
    for c in Path::new(OsStr::from_bytes(raw)).components() {
        match c {
            Component::Normal(n) => out.push(n),
            Component::CurDir | Component::RootDir => {}
            Component::ParentDir | Component::Prefix(_) => {
                return Err(format!(
                    "the entry {:?} goes above the destination (`..`)",
                    String::from_utf8_lossy(raw)
                ));
            }
        }
    }
    Ok((!out.as_os_str().is_empty()).then_some(out))
}

/// A reader that counts the bytes of the stream.
struct Seen<R> {
    inner: R,
    n: u64,
}

impl<R: Read> Read for Seen<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let k = self.inner.read(buf)?;
        self.n += k as u64;
        Ok(k)
    }
}

/// Unpack the tar of `input` into the directory `dest` of the root `root`.
///
/// Each entry is checked as it comes (a stream cannot be checked before it
/// is read): the name is below `dest`, no component of the way is a link, a
/// file never replaces a directory and a directory never replaces a file,
/// and an entry of a kind other than file, directory, symbolic link and
/// hard link is refused by name. An entry that was written stays where an
/// error of a later entry stops the stream; the line names the entry.
pub(super) fn read_in<R: Read>(root: &Path, dest: &Path, input: R) -> Result<String, Fail> {
    let shown = |p: &Path| crate::lifecycle::cp_root::shown(root, p);
    let mut seen = Seen { inner: input, n: 0 };
    let mut dirs: Vec<(PathBuf, u32, u64)> = Vec::new();
    let (mut files, mut bytes, mut links) = (0u64, 0u64, 0u64);
    {
        let mut ar = tar::Archive::new(&mut seen);
        let damaged =
            |e: std::io::Error| refuse(format!("the tar stream on stdin is damaged or cut: {e}"));
        for entry in ar.entries().map_err(damaged)? {
            let mut entry = entry.map_err(damaged)?;
            let raw = entry.path_bytes().into_owned();
            let label = String::from_utf8_lossy(&raw).into_owned();
            let rel = match below(&raw).map_err(refuse)? {
                Some(r) => r,
                // `./`: the destination itself, as it is.
                None => continue,
            };
            let to = dest.join(&rel);
            let kind = entry.header().entry_type();
            let mode = entry.header().mode().map_err(damaged)? & 0o7777;
            let mtime = entry.header().mtime().map_err(damaged)?;
            let io = |e: std::io::Error| broke(format!("{}: {e}", shown(&to)));
            // A kind that podvm does not unpack is refused before the entry
            // makes a directory or a name.
            if !(kind.is_dir()
                || kind.is_file()
                || kind.is_contiguous()
                || kind.is_symlink()
                || kind.is_hard_link())
            {
                return Err(refuse(format!(
                    "the entry {label:?} is of the kind {kind:?}; podvm unpacks files, \
                     directories and links, and refuses special files by name"
                )));
            }
            // No link on the way; the last name may be one for a name that
            // is replaced, never for a directory.
            let last_link = !kind.is_dir();
            clear_of_symlinks(dest, &to, root, last_link).map_err(refuse)?;
            let parent = to.parent().unwrap_or(dest);
            std::fs::create_dir_all(parent).map_err(io)?;
            let existing = std::fs::symlink_metadata(&to).ok();
            let is_dir_now = existing.as_ref().is_some_and(|m| m.is_dir());
            match kind {
                t if t.is_dir() => {
                    if existing.is_some() && !is_dir_now {
                        return Err(refuse(format!(
                            "{}: the entry {label:?} is a directory and a file is there",
                            shown(&to)
                        )));
                    }
                    if !is_dir_now {
                        std::fs::create_dir(&to).map_err(io)?;
                    }
                    if dirs.len() >= MAX_DIRS {
                        return Err(refuse(format!(
                            "the stream holds more than {MAX_DIRS} directories"
                        )));
                    }
                    dirs.push((to, mode, mtime));
                }
                t if t.is_file() || t.is_contiguous() => {
                    if is_dir_now {
                        return Err(refuse(format!(
                            "{}: the entry {label:?} is a file and a directory is there",
                            shown(&to)
                        )));
                    }
                    if existing.is_some() {
                        std::fs::remove_file(&to).map_err(io)?;
                    }
                    let mut f = create_file(root, &to)
                        .map_err(|e| broke(format!("{}: {e}", shown(&to))))?;
                    let n = std::io::copy(&mut entry, &mut f).map_err(|e| {
                        refuse(format!(
                            "the tar stream on stdin is damaged or cut inside {}: {e}",
                            shown(&to)
                        ))
                    })?;
                    f.set_permissions(std::fs::Permissions::from_mode(mode))
                        .map_err(io)?;
                    drop(f);
                    keep(&to, mtime)?;
                    files += 1;
                    bytes += n;
                }
                t if t.is_symlink() || t.is_hard_link() => {
                    if is_dir_now {
                        return Err(refuse(format!(
                            "{}: the entry {label:?} is a link and a directory is there",
                            shown(&to)
                        )));
                    }
                    let target = entry
                        .link_name_bytes()
                        .ok_or_else(|| refuse(format!("the link {label:?} has no target")))?
                        .into_owned();
                    if existing.is_some() {
                        std::fs::remove_file(&to).map_err(io)?;
                    }
                    if t.is_symlink() {
                        // The bytes of the target are opaque: a link is never resolved.
                        std::os::unix::fs::symlink(OsStr::from_bytes(&target), &to).map_err(io)?;
                        keep(&to, mtime)?;
                    } else {
                        let from = dest.join(below(&target).map_err(refuse)?.ok_or_else(|| {
                            refuse(format!("the hard link {label:?} names the destination"))
                        })?);
                        clear_of_symlinks(dest, &from, root, false).map_err(refuse)?;
                        if !std::fs::symlink_metadata(&from).is_ok_and(|m| m.is_file()) {
                            return Err(refuse(format!(
                                "{}: the hard link {label:?} names {}, which is not a file of the stream",
                                shown(&to),
                                shown(&from)
                            )));
                        }
                        std::fs::hard_link(&from, &to).map_err(io)?;
                    }
                    links += 1;
                }
                other => {
                    return Err(refuse(format!(
                        "the entry {label:?} is of the kind {other:?}; podvm unpacks files, \
                         directories and links, and refuses special files by name"
                    )));
                }
            }
        }
    }
    if seen.n == 0 {
        return Err(refuse(
            "stdin holds no tar stream (it has no byte)".to_string(),
        ));
    }
    // The deepest first: a closed directory cannot refuse its child, and
    // the time of a directory is the last write into it.
    dirs.sort_by_key(|(p, ..)| std::cmp::Reverse(p.components().count()));
    for (p, mode, mtime) in &dirs {
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(*mode))
            .map_err(|e| broke(format!("{}: {e}", shown(p))))?;
        keep(p, *mtime)?;
    }
    Ok(format!(
        "{files} files ({bytes} bytes), {} directories, {links} links from stdin into {}",
        dirs.len(),
        shown(dest)
    ))
}

fn keep(path: &Path, mtime: u64) -> Result<(), Fail> {
    let sec = i64::try_from(mtime)
        .map_err(|_| broke(format!("{}: a time out of range", path.display())))?;
    super::times::set(path, sec, 0).map_err(|e| {
        broke(format!(
            "{}: the modification time is not kept: {e}",
            path.display()
        ))
    })
}

/// A new file below `root`, with no link on the way, mode 0 until the
/// caller sets the mode from the entry.
fn create_file(root: &Path, host: &Path) -> Result<std::fs::File, String> {
    use std::os::fd::FromRawFd;
    let mut buf = [0u8; 8192];
    match pod_beneath::open_under_base(
        &pod_beneath::Libc,
        root.as_os_str().as_bytes(),
        host.as_os_str().as_bytes(),
        pod_beneath::O_CREAT | pod_beneath::O_EXCL | pod_beneath::O_WRONLY,
        &mut buf,
    ) {
        // SAFETY: the descriptor is new and the caller's.
        Ok(fd) => Ok(unsafe { std::fs::File::from_raw_fd(fd) }),
        Err(r) => Err(format!("{} (a link or a path out of the root)", r.text())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The names of the archive that may go below the destination, in one
    /// table: normal parts stay, a leading `/` and `.` go, `..` is refused,
    /// and a name with nothing in it is the destination.
    #[test]
    fn a_name_of_the_archive_stays_below_the_destination() {
        let rows: &[(&str, Result<Option<&str>, ()>)] = &[
            ("a/b", Ok(Some("a/b"))),
            ("./a/b/", Ok(Some("a/b"))),
            ("/etc/passwd", Ok(Some("etc/passwd"))),
            (".", Ok(None)),
            ("./", Ok(None)),
            ("", Ok(None)),
            ("..", Err(())),
            ("a/../../b", Err(())),
            ("../x", Err(())),
        ];
        for (raw, want) in rows {
            let got = below(raw.as_bytes()).map(|o| o.map(|p| p.display().to_string()));
            let want = want.as_ref().map(|o| o.map(String::from)).map_err(|_| ());
            assert_eq!(got.map_err(|_| ()), want, "{raw:?}");
        }
    }
}
