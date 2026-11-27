//! The container side of `cp`, resolved in the root of the container.
//!
//! TODO/cli.md T-2087 (OR-B40-16, OR-B41-04): docker resolves the path of
//! `cp` as the kernel of the container does. A link with an absolute target
//! (stock alpine `/var/run -> /run`) starts again at the root of the
//! container, not at the root of the host. The walk is the walk of the
//! payload (`podvm_enter::abi::resolve_dir_in`), so a link never takes the
//! path out of the root.

use std::path::{Path, PathBuf};

use podvm_enter::abi::ResolveKind;

use super::*;

/// The words for `p` in a line of `cp`: the path of the container where
/// `p` is under its root `root` (`/tmp/x`), and `p` as it is where not.
/// A line never names a path of the store (TODO/cli.md T-2070, OR-B38-14).
pub(super) fn shown(root: &Path, p: &Path) -> String {
    match p.strip_prefix(root) {
        Ok(rel) if rel.as_os_str().is_empty() => "/".to_string(),
        Ok(rel) => format!("/{}", rel.display()),
        Err(_) => p.display().to_string(),
    }
}

/// The host path of the container path `inside` under `root`.
///
/// Each name before the last is resolved in the root, links included. The
/// last name stays as it is: a link at the last name is copied as a link,
/// as docker does. A path that ends in `/`, `/.` or `..` names a directory,
/// so the walk resolves its last name too.
///
/// With `follow_last` (the destination of a copy into the container), a
/// last name that is a link to a directory takes that directory, as the
/// docker client follows a link at the destination.
///
/// Where a directory on the way does not exist, the longest part that
/// resolves takes the rest of the names as written, and the rule of the
/// destination (`cp_rule`) names the missing directory. A link whose target
/// is not in the container is refused by name: its target is a path of the
/// container, never a path of the host.
pub(super) fn in_root(root: &Path, inside: &str, follow_last: bool) -> Result<PathBuf, String> {
    let names: Vec<&str> = inside.split('/').filter(|s| !s.is_empty()).collect();
    let whole = names.is_empty()
        || inside.ends_with('/')
        || matches!(names.last(), Some(&".") | Some(&".."));
    let (walk, last) = if whole {
        (names.as_slice(), None)
    } else {
        (&names[..names.len() - 1], names.last().copied())
    };
    let dir = walk.join("/");
    match podvm_enter::abi::resolve_dir_in(root, &dir) {
        Ok(host) => Ok(match last {
            Some(name) if follow_last && is_link(&host.join(name)) => {
                podvm_enter::abi::resolve_dir_in(root, inside).unwrap_or_else(|_| host.join(name))
            }
            Some(name) => host.join(name),
            None => host,
        }),
        Err(ResolveKind::Absent) => missing(root, inside, walk, last),
        Err(ResolveKind::Escapes) => Err(format!(
            "{inside} goes above the root of the container (a `..` or a link): podvm \
             resolves a path of a container in its root, and a `..` above that root is refused"
        )),
        Err(ResolveKind::Loop) => Err(format!(
            "{inside} passes through more than 40 links in the container (a loop)"
        )),
    }
}

/// The path of a walk that stopped at a missing name: the longest part of
/// `walk` that resolves, then the names after it as written.
fn missing(
    root: &Path,
    inside: &str,
    walk: &[&str],
    last: Option<&str>,
) -> Result<PathBuf, String> {
    // The root itself resolves, so `k` stops at 0 at the latest.
    let (k, mut host) = (0..walk.len())
        .rev()
        .find_map(|k| {
            podvm_enter::abi::resolve_dir_in(root, &walk[..k].join("/"))
                .ok()
                .map(|h| (k, h))
        })
        .ok_or_else(|| "the root of the container is not a directory".to_string())?;
    let next = host.join(walk[k]);
    if let Ok(target) = std::fs::read_link(&next) {
        return Err(format!(
            "{inside}: /{} is a link to {} in the container, and no directory of the \
             container is there (podvm resolves a link of a container in its root)",
            walk[..=k].join("/"),
            target.display()
        ));
    }
    for name in walk[k..].iter().chain(last.iter()) {
        host.push(name);
    }
    Ok(host)
}

/// Copy one path between the host and the root `root` of a container or
/// an image, in the direction `out_of`.
pub(super) fn copy_one(
    root: &std::path::Path,
    inside: &str,
    outside: &std::path::Path,
    out_of: bool,
    recursive: bool,
) -> i32 {
    // TODO/cli.md T-2087 (OR-B40-16): the links on the way resolve in the
    // root of the container, as docker resolves them.
    let joined = match in_root(root, inside, !out_of) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("podvm cp: the copy is refused: {e}");
            return EXIT_CLI_ERROR; // TODO/cli.md T-2062: a refusal after the parse
        }
    };
    // ⚠ Stat BEFORE gating: a symlink source is gated on its parent, never
    // resolved; `within` resolves, and `/etc/mtab -> /proc/self/mounts` of
    // a distro escapes nothing (TODO/extract.md T-0305).
    let src_side: &std::path::Path = if out_of { &joined } else { outside };
    let src_is_link = is_link(src_side);
    // ⛔ THE SAME CONTAINMENT GATE as each other path: `cp` is the verb most
    // likely to be handed a `../../etc/shadow`.
    let target = if src_is_link && out_of {
        match joined.parent() {
            Some(parent) => match podvm_image::contain::within(root, parent) {
                Ok(_) => joined.clone(),
                Err(e) => {
                    eprintln!("podvm cp: the copy is refused: {e}");
                    return EXIT_CLI_ERROR; // TODO/cli.md T-2062: a refusal after the parse
                }
            },
            None => {
                eprintln!("podvm cp: {} has no parent to gate", shown(root, &joined));
                return EXIT_RUNTIME_ERROR;
            }
        }
    } else {
        match podvm_image::contain::within(root, &joined) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("podvm cp: the copy is refused: {e}");
                return EXIT_CLI_ERROR; // TODO/cli.md T-2062: a refusal after the parse
            }
        }
    };
    // TODO/cli.md T-2108 (OR-B42-06): `-` is the tar stream, as docker has it.
    if outside == Path::new(super::cp_tar::STREAM) {
        return super::cp_tar::stream(root, &target, inside, out_of);
    }
    let (src, dst) = if out_of {
        (target, outside.to_path_buf())
    } else {
        (outside.to_path_buf(), target)
    };
    let (src_named, dest_named) = if out_of {
        (inside.to_string(), outside.to_string_lossy().into_owned())
    } else {
        (outside.to_string_lossy().into_owned(), inside.to_string())
    };
    // ⚠ Stat the SOURCE side first: its kind decides the shape of the copy,
    // and a missing source makes nothing on the other side.
    let kind = match std::fs::symlink_metadata(&src).map(|m| m.file_type()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("podvm cp: {src_named}: {e}");
            return EXIT_CLI_ERROR; // T-2062
        }
    };
    if kind.is_dir() && !recursive {
        eprintln!(
            "podvm cp: {} is a directory; pass -r to copy it",
            shown(root, &src)
        );
        // TODO/cli.md T-2108 (OR-B42-06): the flags parsed, so a refusal.
        return EXIT_CLI_ERROR;
    }
    if !kind.is_dir() && !kind.is_file() && !kind.is_symlink() {
        eprintln!(
            "podvm cp: {} is not a file or directory; special files are refused",
            shown(root, &src)
        );
        return EXIT_CLI_ERROR; // TODO/cli.md T-2108 (OR-B42-06)
    }
    // The basename of SRC as it was named; `..` takes its resolved name.
    let name = std::path::Path::new(&src_named)
        .file_name()
        .map(|n| n.to_os_string())
        .or_else(|| {
            src.canonicalize()
                .ok()?
                .file_name()
                .map(|n| n.to_os_string())
        });
    // ⛔ TODO/cli.md T-2079 (OR-B39-04, OR-B39-14): docker's rule, in one
    // place (`cp_rule`), for each direction and an image as the source.
    let ask = super::cp_rule::Ask {
        src: if kind.is_dir() {
            super::cp_rule::Src::Dir
        } else {
            super::cp_rule::Src::File
        },
        contents: super::cp_rule::names_contents(&src_named),
        name: name.as_deref(),
        dest: super::cp_rule::dest_of(&dst),
        slash: super::cp_rule::names_dir(&dest_named),
    };
    let dst = match super::cp_rule::target(ask, &dst) {
        Ok(p) => p,
        Err(r) => {
            eprintln!("podvm cp: {}", r.line(&dest_named));
            return EXIT_CLI_ERROR; // T-2062
        }
    };
    // ⛔ Never write through a pre-existing symlink: the path is gated but
    // the write would land where the link points.
    if is_link(&dst) {
        eprintln!(
            "podvm cp: {} is a symlink; writing through one is refused",
            shown(root, &dst)
        );
        return EXIT_CLI_ERROR; // TODO/cli.md T-2108
    }
    if !out_of && let Err(msg) = clear_of_symlinks(root, &dst, root, false) {
        eprintln!("podvm cp: {msg}");
        return EXIT_CLI_ERROR; // T-2062
    }
    if kind.is_symlink() {
        return replicate_link(&src, &dst, root);
    }
    if kind.is_dir() {
        return copy_tree(&src, &dst, root, out_of);
    }
    match std::fs::copy(&src, &dst) {
        Ok(n) => {
            // TODO/cli.md T-2108 (OR-B42-07): the modification time stays.
            if let Err(e) = super::copy_tree::keep_time(&src, &dst) {
                eprintln!(
                    "podvm cp: {}: the modification time is not kept: {e}",
                    shown(root, &dst)
                );
                return EXIT_RUNTIME_ERROR;
            }
            eprintln!(
                "podvm cp: {} bytes {} -> {}",
                n,
                shown(root, &src),
                shown(root, &dst)
            );
            0
        }
        Err(e) => {
            eprintln!(
                "podvm cp: {} -> {}: {e}",
                shown(root, &src),
                shown(root, &dst)
            );
            EXIT_RUNTIME_ERROR
        }
    }
}
