use super::*;

/// Whether the path itself is a symlink (never followed).
pub(super) fn is_link(path: &std::path::Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
}

/// The modification time of `from` goes to `to`; neither is followed.
pub(super) fn keep_time(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    super::cp_tar::times::keep(&std::fs::symlink_metadata(from)?, to)
}

/// Replicate one symlink as a symlink: the target bytes are copied, never
/// resolved, so an absolute target escapes nothing. The caller gates the
/// link's own path; the target is opaque here by design.
pub(super) fn replicate_link(
    src: &std::path::Path,
    dst: &std::path::Path,
    root: &std::path::Path,
) -> i32 {
    let shown = |p: &std::path::Path| super::cp_root::shown(root, p);
    let target = match std::fs::read_link(src) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("podvm cp: {}: {e}", shown(src));
            return EXIT_RUNTIME_ERROR;
        }
    };
    if let Some(parent) = dst.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("podvm cp: {}: {e}", shown(parent));
        return EXIT_RUNTIME_ERROR;
    }
    match std::os::unix::fs::symlink(&target, dst) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if dst.is_dir() && !is_link(dst) {
                eprintln!("podvm cp: {} exists and is a directory", shown(dst));
                return EXIT_CLI_ERROR; // TODO/cli.md T-2108
            }
            if let Err(e) = std::fs::remove_file(dst) {
                eprintln!("podvm cp: {}: {e}", shown(dst));
                return EXIT_RUNTIME_ERROR;
            }
            if let Err(e) = std::os::unix::fs::symlink(&target, dst) {
                eprintln!("podvm cp: {}: {e}", shown(dst));
                return EXIT_RUNTIME_ERROR;
            }
        }
        Err(e) => {
            eprintln!("podvm cp: {}: {e}", shown(dst));
            return EXIT_RUNTIME_ERROR;
        }
    }
    if let Err(e) = keep_time(src, dst) {
        eprintln!(
            "podvm cp: {}: the modification time is not kept: {e}",
            shown(dst)
        );
        return EXIT_RUNTIME_ERROR;
    }
    eprintln!("podvm cp: symlink {} -> {}", shown(dst), target.display());
    0
}

/// Every component of `path` below `base`, checked without following: a
/// write through a pre-existing symlink would land where the link points,
/// so the first symlink refuses the copy by name. `base` itself was gated
/// by the caller and is not re-checked. `root` is the root of the container,
/// so that the line names a path of the container, not of the store.
///
/// With `last_may_link`, the last name may be a link: the copy replaces it
/// by the file or the link of the source and never writes through it
/// (TODO/cli.md T-2070, OR-B38-07: docker replaces a link of the
/// destination).
pub(super) fn clear_of_symlinks(
    base: &std::path::Path,
    path: &std::path::Path,
    root: &std::path::Path,
    last_may_link: bool,
) -> std::result::Result<(), String> {
    let mut probe = base.to_path_buf();
    let rel = path.strip_prefix(base).map_err(|_| {
        format!(
            "{} escapes {}",
            super::cp_root::shown(root, path),
            super::cp_root::shown(root, base)
        )
    })?;
    let n = rel.components().count();
    for (i, component) in rel.components().enumerate() {
        probe.push(component);
        if is_link(&probe) && !(last_may_link && i + 1 == n) {
            return Err(format!(
                "{} is a symlink; writing through one is refused",
                super::cp_root::shown(root, &probe)
            ));
        }
    }
    Ok(())
}

/// Copy a directory tree without following anything. Every rootfs-side
/// path passes `contain::within`: destinations on the way in, sources on
/// the way out: except a symlink source, which is gated on its own path
/// (its parent) and replicated verbatim, never resolved, per T-0305's
/// rule. Two passes: the first validates the whole tree, so a special
/// file or a destination through a pre-existing symlink refuses before a
/// byte lands and no half-made tree is left behind (TODO/cli.md T-1323).
pub(super) fn copy_tree(
    src: &std::path::Path,
    dst: &std::path::Path,
    root: &std::path::Path,
    out_of: bool,
) -> i32 {
    let gate = |p: &std::path::Path| podvm_image::contain::within(root, p);
    // The lines name a path of the container, never a path of the store.
    let shown = |p: &std::path::Path| super::cp_root::shown(root, p);
    // The destination side's base for the write-through check: the rootfs
    // on the way in, the fresh-or-merging top on the way out. A write
    // through a pre-existing link would land where the link points, so
    // the first such destination refuses the copy by name.
    let dst_base: &std::path::Path = if out_of { dst } else { root };
    // Pass one: walk without following, gating and collecting. A special
    // file anywhere, or a destination through a pre-existing symlink,
    // refuses the whole copy by name.
    let mut files: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    let mut links: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    // TODO/supervise.md T-1822: each directory and its source, for its
    // mode after its content (a 0500 directory would refuse its files).
    let mut modes: Vec<(std::path::PathBuf, std::path::PathBuf)> =
        vec![(src.to_path_buf(), dst.to_path_buf())];
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                // ⭐ T-0414: the listing is required (a tree copy walks
                // it), so the refusal names the operation and the path
                // with the kernel's errno, never a missing file.
                eprintln!("podvm cp: cannot list {}: {e}", shown(&dir));
                return EXIT_RUNTIME_ERROR;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("podvm cp: {e}");
                    return EXIT_RUNTIME_ERROR;
                }
            };
            let src_path = entry.path();
            let rel = match src_path.strip_prefix(src) {
                Ok(r) => r,
                Err(_) => {
                    eprintln!("podvm cp: {} escapes {}", shown(&src_path), shown(src));
                    return EXIT_RUNTIME_ERROR;
                }
            };
            let dst_path = dst.join(rel);
            // ⛔ Never follow: metadata, not metadata-through.
            let kind = match std::fs::symlink_metadata(&src_path) {
                Ok(m) => m.file_type(),
                Err(e) => {
                    eprintln!("podvm cp: {}: {e}", shown(&src_path));
                    return EXIT_RUNTIME_ERROR;
                }
            };
            if kind.is_symlink() {
                // ⚠ Gated on its own path, never resolved: the target bytes
                // are opaque, and an absolute target escapes nothing.
                let own = if out_of {
                    match src_path.parent() {
                        Some(parent) => parent,
                        None => {
                            eprintln!("podvm cp: {} has no parent to gate", shown(&src_path));
                            return EXIT_RUNTIME_ERROR;
                        }
                    }
                } else {
                    // TODO/cli.md T-2070 (OR-B38-07): the parent, so that a
                    // link of the destination that points out of the root
                    // (an absolute target) is replaced, not resolved.
                    dst_path.parent().unwrap_or(&dst_path)
                };
                if let Err(e) = gate(own) {
                    eprintln!("podvm cp: the copy is refused: {e}");
                    return EXIT_CLI_ERROR; // TODO/cli.md T-2062: a refusal after the parse
                }
                if let Err(msg) = clear_of_symlinks(dst_base, &dst_path, root, true) {
                    eprintln!("podvm cp: {msg}");
                    return EXIT_CLI_ERROR; // T-2062
                }
                links.push((src_path, dst_path));
                continue;
            }
            // A link of the destination under a file of the source is
            // replaced (T-2070), so the gate takes its parent.
            let gated = match (out_of, is_link(&dst_path), dst_path.parent()) {
                (true, _, _) => &src_path,
                (false, true, Some(parent)) if kind.is_file() => parent,
                _ => &dst_path,
            };
            if let Err(e) = gate(gated) {
                eprintln!("podvm cp: the copy is refused: {e}");
                return EXIT_CLI_ERROR; // TODO/cli.md T-2062: a refusal after the parse
            }
            if let Err(msg) = clear_of_symlinks(dst_base, &dst_path, root, kind.is_file()) {
                eprintln!("podvm cp: {msg}");
                return EXIT_CLI_ERROR; // T-2062
            }
            if kind.is_dir() {
                modes.push((src_path.clone(), dst_path.clone()));
                dirs.push(dst_path);
                stack.push(src_path);
                continue;
            }
            if !kind.is_file() {
                eprintln!(
                    "podvm cp: {} is not a file or directory; special files are refused",
                    shown(&src_path)
                );
                return EXIT_CLI_ERROR; // TODO/cli.md T-2108: a refusal after the parse
            }
            files.push((src_path, dst_path));
        }
    }
    // Pass two: the tree is validated, so only I/O can still fail, and
    // each such error names its file. A refusal in pass one leaves no
    // half-made tree behind.
    if is_link(dst) {
        eprintln!(
            "podvm cp: {} is a symlink; writing through one is refused",
            shown(dst)
        );
        return EXIT_CLI_ERROR; // TODO/cli.md T-2108
    }
    if dst.is_file() {
        eprintln!("podvm cp: {} exists and is not a directory", shown(dst));
        return EXIT_CLI_ERROR; // TODO/cli.md T-2108
    }
    if let Err(e) = std::fs::create_dir_all(dst) {
        eprintln!("podvm cp: {}: {e}", shown(dst));
        return EXIT_RUNTIME_ERROR;
    }
    for dir in &dirs {
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("podvm cp: {}: {e}", shown(dir));
            return EXIT_RUNTIME_ERROR;
        }
    }
    let mut bytes = 0u64;
    // TODO/image.md T-1768: a file of several names is copied once, and
    // each other name is a hard link to that copy, as `cp -a` and docker
    // do. busybox's /bin has 406 names of one file; each was a copy.
    let mut first: std::collections::HashMap<(u64, u64), &std::path::Path> =
        std::collections::HashMap::new();
    let mut hard = 0usize;
    for (from, to) in &files {
        if let Some(parent) = to.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            eprintln!("podvm cp: {}: {e}", shown(parent));
            return EXIT_RUNTIME_ERROR;
        }
        let id = std::fs::symlink_metadata(from).ok().and_then(|m| {
            use std::os::unix::fs::MetadataExt;
            (m.nlink() > 1).then_some((m.dev(), m.ino()))
        });
        if let Some(earlier) = id.and_then(|k| first.get(&k)) {
            let _ = std::fs::remove_file(to);
            if std::fs::hard_link(earlier, to).is_ok() {
                hard += 1;
                continue;
            }
        }
        if let Some(k) = id {
            first.insert(k, to.as_path());
        }
        // TODO/cli.md T-2070 (OR-B38-07): a link of the destination is
        // replaced by the file, never written through.
        if is_link(to)
            && let Err(e) = std::fs::remove_file(to)
        {
            eprintln!("podvm cp: {}: {e}", shown(to));
            return EXIT_RUNTIME_ERROR;
        }
        match std::fs::copy(from, to) {
            Ok(n) => bytes += n,
            Err(e) => {
                eprintln!("podvm cp: {}: {e}", shown(from));
                return EXIT_RUNTIME_ERROR;
            }
        }
        // TODO/cli.md T-2108 (OR-B42-07): the modification time stays.
        if let Err(e) = keep_time(from, to) {
            eprintln!(
                "podvm cp: {}: the modification time is not kept: {e}",
                shown(to)
            );
            return EXIT_RUNTIME_ERROR;
        }
    }
    for (from, to) in &links {
        if replicate_link(from, to, root) != 0 {
            return EXIT_RUNTIME_ERROR;
        }
    }
    // The deepest first, so that a closed parent cannot refuse its child.
    modes.sort_by_key(|(_, to)| std::cmp::Reverse(to.components().count()));
    for (from, to) in &modes {
        if let Ok(m) = std::fs::symlink_metadata(from) {
            let _ = std::fs::set_permissions(to, m.permissions());
        }
        // After the mode and after each entry of the directory: a new
        // entry would move the time again (TODO/cli.md T-2108).
        if let Err(e) = keep_time(from, to) {
            eprintln!(
                "podvm cp: {}: the modification time is not kept: {e}",
                shown(to)
            );
            return EXIT_RUNTIME_ERROR;
        }
    }
    eprintln!(
        "podvm cp: {} files ({bytes} bytes), {hard} hard links, {} symlinks {} -> {}",
        files.len() - hard,
        links.len(),
        shown(src),
        shown(dst)
    );
    0
}
