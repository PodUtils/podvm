//! TODO/enter.md T-1953 (PodUtils/podvm#547): the rung that a run enters
//! where pod-enter falls back from the namespace rung by a fact of the
//! image.
//!
//! pod-enter mounts its private tmpfs on the `/tmp` of the image. Where
//! the image has none that can hold a mount (missing, a file or a link),
//! the entry enters chroot and writes one line. The banner, the record,
//! the report, `inspect` and `--strict` read the rung that `prepare`
//! decides, so `prepare` takes the same decision before the entry, from
//! the same function (`podvm_enter::ns_tmp_target`), and each of them names
//! the rung that runs. A fallback that depends on the host at the entry (a
//! mount that the host refuses after the probe) stays pod-enter's, and the
//! foreground run records the rung that its child reports.

/// Why the namespace rung that the probe selects enters chroot instead, or
/// `None` where it does not. A forced launch rung, a loader family and an
/// init of the image (`own_entry`) enter by their own path.
pub(super) fn cause(
    entered: podvm_probe::select::Rung,
    userland: bool,
    own_entry: bool,
    rootfs: &str,
) -> Option<String> {
    (entered == podvm_probe::select::Rung::Namespace
        && !userland
        && !own_entry
        && podvm_enter::ns_tmp_target(rootfs).is_none())
    .then(|| format!("the image holds no /tmp mount point under {rootfs}"))
}

/// [`cause`] for the findings of the probe. A foreground run goes on in
/// chroot, and the line of the fallback goes to stderr once, as pod-enter
/// writes it at the entry. A detached run (`run -d`, `create`) is refused
/// at once: its launcher gives the container a PID namespace, which only
/// the namespace rung has, and the refusal came at `start` with the same
/// words, after the record existed.
pub(super) fn decide(
    verb: &str,
    findings: &podvm_probe::Findings,
    userland: bool,
    own_entry: bool,
    detached: bool,
    rootfs: &str,
) -> std::result::Result<Option<String>, i32> {
    let selected = podvm_probe::select::Selection::choose(findings).rung;
    let Some(why) = cause(
        podvm_enter::entered_rung(selected),
        userland,
        own_entry,
        rootfs,
    ) else {
        return Ok(None);
    };
    if detached {
        eprintln!("podvm {verb}: the PID namespace needs the namespace rung, and {why}");
        return Err(podvm_image::error::EXIT_RUNTIME_ERROR);
    }
    eprintln!("{}", podvm_enter::ns_fallback_line(&why));
    Ok(Some(why))
}

#[cfg(test)]
mod tests {
    use super::cause;
    use podvm_probe::select::Rung;

    /// One rule for each tree: where `/tmp` can hold a mount point the
    /// rung stays; a missing `/tmp`, a file and a link downgrade it; and
    /// an entry of its own, a family or another rung is never downgraded
    /// by it.
    #[test]
    fn a_tree_with_no_tmp_mount_point_enters_chroot() {
        let d = std::env::temp_dir().join(format!("podvm-fallback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let mk = |name: &str| {
            let r = d.join(name);
            std::fs::create_dir_all(&r).unwrap();
            r
        };
        let (none, dir, file, link) = (mk("none"), mk("dir"), mk("file"), mk("link"));
        std::fs::create_dir(dir.join("tmp")).unwrap();
        std::fs::write(file.join("tmp"), b"x").unwrap();
        std::os::unix::fs::symlink("/var/tmp", link.join("tmp")).unwrap();
        let s = |p: &std::path::Path| p.to_str().unwrap().to_string();
        let ns = |p: &std::path::Path| cause(Rung::Namespace, false, false, &s(p));
        assert_eq!(ns(&dir), None);
        for bad in [&none, &file, &link] {
            let why = ns(bad).unwrap_or_else(|| panic!("{} stays", bad.display()));
            assert!(why.contains("no /tmp mount point"), "{why}");
        }
        assert_eq!(cause(Rung::Namespace, true, false, &s(&none)), None);
        assert_eq!(cause(Rung::Namespace, false, true, &s(&none)), None);
        assert_eq!(cause(Rung::Chroot, false, false, &s(&none)), None);
        assert_eq!(cause(Rung::Userland, false, false, &s(&none)), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
