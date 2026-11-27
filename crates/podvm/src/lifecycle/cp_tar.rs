//! TODO/cli.md T-2108 (OR-B42-06): the tar stream of `cp`, as docker has it.
//!
//! * `cp NAME:PATH -` writes a tar of PATH to stdout. The entries are named
//!   by the last name of PATH (`etc/`, `etc/hostname`), or `./` and `./x`
//!   where PATH ends with `/.`, as docker names them.
//! * `cp - NAME:PATH` reads a tar from stdin into the directory PATH.
//!
//! Both sides keep the rule of the files copy: the container side is
//! resolved in the root of the container (`cp_root::in_root`) and gated
//! (`contain::within`) before a byte moves; a link is never followed, in
//! the tree or at a destination; a special file (a FIFO, a device) is
//! refused by name. The opens of files go through `pod-beneath`, so a link
//! that a running payload plants after the check moves no read or write
//! out of the root of the container.
//!
//! The ownership of an entry going out is 0:0 (the files of the store carry
//! the id of the host user, and the ownership of the container is a record
//! of the memo that the stream does not read). An entry coming in keeps its
//! mode and its modification time and takes the owner of the caller.

use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use super::*;

pub(super) mod times; // TODO/cli.md T-2108 (OR-B42-07)
mod untar;

/// A refusal (exit 1: the input or the tree is not one podvm takes) or a
/// failure of the host (exit 125), each with its one true line.
pub(super) type Fail = (i32, String);

/// The name that stands for stdin or stdout in place of a path.
pub(super) const STREAM: &str = "-";

/// `cp` of a container path to stdout (`out_of`) or from stdin, after the
/// gate: `target` is the host path inside `root` that the container path
/// `inside` names.
pub(super) fn stream(root: &Path, target: &Path, inside: &str, out_of: bool) -> i32 {
    use std::io::IsTerminal;
    // The gate gave paths of the root as it was spelled; the open and the
    // checks below compare prefixes, so both are of the resolved root.
    let croot = match root.canonicalize() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("podvm cp: the root of the container does not resolve: {e}");
            return EXIT_RUNTIME_ERROR;
        }
    };
    let target = match target.strip_prefix(root) {
        Ok(rel) => croot.join(rel),
        Err(_) => target.to_path_buf(),
    };
    if out_of {
        if std::io::stdout().is_terminal() {
            eprintln!("podvm cp: stdout is a terminal; podvm does not write a tar stream to one");
            return EXIT_CLI_ERROR;
        }
        let name: PathBuf = if super::cp_rule::names_contents(inside) {
            PathBuf::from(".")
        } else {
            Path::new(inside)
                .file_name()
                .or_else(|| target.file_name())
                .map_or_else(|| PathBuf::from("."), PathBuf::from)
        };
        let out = std::io::BufWriter::with_capacity(64 * 1024, std::io::stdout().lock());
        return report(write_out(&croot, &target, &name, out));
    }
    if std::io::stdin().is_terminal() {
        eprintln!("podvm cp: stdin is a terminal; `cp - NAME:PATH` reads a tar stream from it");
        return EXIT_CLI_ERROR;
    }
    match std::fs::metadata(&target) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => {
            eprintln!(
                "podvm cp: {inside} is not a directory; a tar stream unpacks into a directory"
            );
            return EXIT_CLI_ERROR;
        }
        Err(e) => {
            eprintln!("podvm cp: {inside}: {e}; a tar stream unpacks into a directory that exists");
            return EXIT_CLI_ERROR;
        }
    }
    report(untar::read_in(&croot, &target, std::io::stdin().lock()))
}

/// Directories of one stream whose mode and time wait for the end of it.
pub(super) const MAX_DIRS: usize = 1_000_000;

pub(super) fn refuse(msg: String) -> Fail {
    (EXIT_CLI_ERROR, msg)
}

pub(super) fn broke(msg: String) -> Fail {
    (EXIT_RUNTIME_ERROR, msg)
}

/// Print the line of a `Fail` and give its code.
pub(super) fn report(r: Result<String, Fail>) -> i32 {
    match r {
        Ok(line) => {
            eprintln!("podvm cp: {line}");
            0
        }
        Err((code, msg)) => {
            eprintln!("podvm cp: {msg}");
            code
        }
    }
}

// ---------------------------------------------------------------- out

/// One entry of the walk: where it is, how the archive names it.
struct Item {
    host: PathBuf,
    name: PathBuf,
    meta: std::fs::Metadata,
}

/// Walk `top` without following a link, parents before children, each
/// directory in the order of its names. `top` is named `name` in the
/// archive. `visit` sees each entry.
fn walk(
    top: &Path,
    name: &Path,
    shown: &dyn Fn(&Path) -> String,
    visit: &mut dyn FnMut(Item) -> Result<(), Fail>,
) -> Result<(), Fail> {
    let first =
        std::fs::symlink_metadata(top).map_err(|e| refuse(format!("{}: {e}", shown(top))))?;
    let mut stack = vec![Item {
        host: top.to_path_buf(),
        name: name.to_path_buf(),
        meta: first,
    }];
    while let Some(item) = stack.pop() {
        let kind = item.meta.file_type();
        if !(kind.is_dir() || kind.is_file() || kind.is_symlink()) {
            return Err(refuse(format!(
                "{} is not a file or directory; special files are refused",
                shown(&item.host)
            )));
        }
        if kind.is_dir() {
            let mut names: Vec<std::ffi::OsString> = std::fs::read_dir(&item.host)
                .and_then(|d| d.map(|e| e.map(|e| e.file_name())).collect())
                .map_err(|e| refuse(format!("cannot list {}: {e}", shown(&item.host))))?;
            names.sort();
            // The last name goes on the stack first, so the first comes out first.
            for n in names.iter().rev() {
                let host = item.host.join(n);
                let meta = std::fs::symlink_metadata(&host)
                    .map_err(|e| refuse(format!("{}: {e}", shown(&host))))?;
                stack.push(Item {
                    name: item.name.join(n),
                    host,
                    meta,
                });
            }
        }
        visit(item)?;
    }
    Ok(())
}

fn header(meta: &std::fs::Metadata, size: u64) -> tar::Header {
    let mut h = tar::Header::new_gnu();
    h.set_metadata_in_mode(meta, tar::HeaderMode::Complete);
    h.set_uid(0);
    h.set_gid(0);
    h.set_size(size);
    h
}

/// A reader that counts what the file gave, so that a file that changed
/// while it was read is a named failure and not a damaged archive.
struct Counted<R> {
    inner: R,
    n: u64,
}

impl<R: Read> Read for Counted<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let k = self.inner.read(buf)?;
        self.n += k as u64;
        Ok(k)
    }
}

/// Write the tar of `src` (a path under `root`) to `out`. `name` is the
/// name of `src` in the archive: its last name, or `.` for its contents.
/// The tree is checked for special files first, so that a refusal comes
/// before the first byte of the stream.
pub(super) fn write_out<W: Write>(
    root: &Path,
    src: &Path,
    name: &Path,
    out: W,
) -> Result<String, Fail> {
    let shown = |p: &Path| super::cp_root::shown(root, p);
    walk(src, name, &shown, &mut |_| Ok(()))?;
    let mut b = tar::Builder::new(out);
    b.mode(tar::HeaderMode::Complete);
    let mut hard: std::collections::HashMap<(u64, u64), PathBuf> = std::collections::HashMap::new();
    let (mut files, mut bytes) = (0u64, 0u64);
    walk(src, name, &shown, &mut |item| {
        let io = |e: std::io::Error| broke(format!("{}: {e}", shown(&item.host)));
        let kind = item.meta.file_type();
        if kind.is_dir() {
            let mut h = header(&item.meta, 0);
            h.set_entry_type(tar::EntryType::Directory);
            let mut n = item.name.clone().into_os_string();
            n.push("/");
            return b
                .append_data(&mut h, PathBuf::from(n), std::io::empty())
                .map_err(io);
        }
        if kind.is_symlink() {
            let target = std::fs::read_link(&item.host).map_err(io)?;
            let mut h = header(&item.meta, 0);
            h.set_entry_type(tar::EntryType::Symlink);
            return b.append_link(&mut h, &item.name, &target).map_err(io);
        }
        // A file with more than one name goes once; the other names are
        // hard links to it, as `cp -a` and docker have them (T-1768).
        if item.meta.nlink() > 1 {
            let id = (item.meta.dev(), item.meta.ino());
            if let Some(first) = hard.get(&id) {
                let mut h = header(&item.meta, 0);
                h.set_entry_type(tar::EntryType::Link);
                return b.append_link(&mut h, &item.name, first).map_err(io);
            }
            hard.insert(id, item.name.clone());
        }
        let file = open_file(root, &item.host)
            .map_err(|e| broke(format!("{}: {e}", shown(&item.host))))?;
        let meta = file.metadata().map_err(io)?;
        if !meta.is_file() {
            return Err(refuse(format!(
                "{} is not a file or directory; special files are refused",
                shown(&item.host)
            )));
        }
        let size = meta.len();
        let mut h = header(&meta, size);
        h.set_entry_type(tar::EntryType::Regular);
        let mut data = Counted {
            inner: file.take(size),
            n: 0,
        };
        b.append_data(&mut h, &item.name, &mut data).map_err(io)?;
        if data.n != size {
            return Err(broke(format!(
                "{} changed while it was read ({} of {size} bytes); the stream is damaged",
                shown(&item.host),
                data.n
            )));
        }
        files += 1;
        bytes += size;
        Ok(())
    })?;
    b.into_inner()
        .and_then(|mut w| w.flush())
        .map_err(|e| broke(format!("the tar stream did not reach stdout: {e}")))?;
    Ok(format!(
        "tar stream of {} ({files} files, {bytes} bytes) to stdout",
        shown(src)
    ))
}

/// Open a file for reading, below `root`, with no link on the way or at the
/// last name (`openat2` with `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`).
fn open_file(root: &Path, host: &Path) -> Result<std::fs::File, String> {
    use std::os::fd::FromRawFd;
    let mut buf = [0u8; 8192];
    // O_NONBLOCK of x86_64 and aarch64: a FIFO that replaced the file
    // after the walk must not block the open.
    const O_NONBLOCK: u64 = 0o4000;
    match pod_beneath::open_under_base(
        &pod_beneath::Libc,
        root.as_os_str().as_bytes(),
        host.as_os_str().as_bytes(),
        O_NONBLOCK,
        &mut buf,
    ) {
        // SAFETY: the descriptor is new and the caller's.
        Ok(fd) => Ok(unsafe { std::fs::File::from_raw_fd(fd) }),
        Err(r) => Err(format!("{} (a link or a path out of the root)", r.text())),
    }
}
