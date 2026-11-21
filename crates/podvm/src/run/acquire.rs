use super::*;

/// The entry banner of `prepare`: the rung podvm enters with, and the notes
/// that ride beside it (the alias, the interposer, each device mapping and
/// the userland account). It answers the banner and the entered rung, which
/// the caller reuses for `--strict` and the container record.
pub(super) fn entry_banner(
    findings: &podvm_probe::Findings,
    facts: podvm_probe::isolation::Facts,
    userland: Option<&crate::lifecycle::Userland>,
    interpose_note: &str,
    devices: &[podvm_enter::device::Mapping],
    dropped_preload: Option<&str>,
    userland_note: &str,
) -> (
    String,
    podvm_probe::select::Rung,
    podvm_probe::isolation::Opened,
) {
    let selection = podvm_probe::select::Selection::choose(findings);
    // ⭐ T-0804 rule 4. The banner is built from the rung podvm ENTERS with,
    // not the one the machine would permit: the namespace rung where
    // selected (T-1339), the chroot sequence, or, where chroot is denied
    // and a no-chroot family runs, the userland one. One value, so the
    // banner, the container record and `--strict` cannot disagree.
    let entered = userland
        .map(|_| podvm_probe::select::Rung::Userland)
        .unwrap_or_else(|| podvm_enter::entered_rung(selection.rung));
    // TODO/cli.md T-2053 (OQ-07): what the entry opens, from its facts.
    let opened = podvm_probe::isolation::of_entry(entered, facts);
    let mut banner = podvm_probe::report::entry_banner_of(findings, &selection, entered, opened);
    // ⭐ TODO/cli.md T-0803. Where podvm was reached under somebody else's
    // name, the banner says which name was used and that this is podvm.
    // Taking the name is the product requirement; taking it silently is what
    // TOOL.md section 4.1 forbids.
    if let Some(note) = crate::names::alias_note() {
        banner.push_str(&note);
    }
    banner.push_str(&crate::userns::shown_line().unwrap_or_default()); // T-2054
    banner.push_str(interpose_note);
    // ⭐ TODO/enter.md T-0501. Every mapping is named before anything of
    // the payload's runs: what the guest spelling serves (a duplicate of a
    // host descriptor opened before the chroot, never a node), where it
    // serves (where the interposer holds), and what `m` does (nothing).
    for m in devices {
        banner.push_str(&podvm_enter::device::banner_line(m));
    }
    // ⭐ TODO/enter.md T-1317. The family account joins the banner beside
    // the mode: what runs the payload and what it does not isolate.
    if let Some(u) = userland {
        banner.push_str(&u.banner);
        banner.push('\n');
        if let Some(d) = dropped_preload {
            banner.push_str(&format!(
                "podvm: userland: caller LD_PRELOAD {d:?} dropped: it names guest \
                 paths, which resolve on the host without a chroot\n"
            ));
        }
        // ⭐ T-1407 and T-1409: the anchoring (or the opt-out) and the
        // exe substitution, named once beside the rung account above.
        banner.push_str(userland_note);
    }
    (banner, entered, opened)
}

/// TODO/cli.md T-2053 (OQ-07): the facts of the entry of `o`, for each
/// statement of what it opens (`podvm_probe::isolation`).
pub(super) fn facts(o: &Opts, detached: bool, rootfs: &str) -> podvm_probe::isolation::Facts {
    podvm_probe::isolation::Facts {
        userns_active: crate::userns::active(),
        host_root: crate::userns::host_root(),
        detached,
        hostname: o.hostname.is_some(),
        // pod-enter mounts a procfs on a directory of the image, not on a
        // link and not where the image has none.
        proc_dir: std::fs::symlink_metadata(std::path::Path::new(rootfs).join("proc"))
            .is_ok_and(|m| m.is_dir()),
    }
}

/// The hardening of the record of `o`, with the parts that its entry
/// opened (TODO/cli.md T-2053): `inspect` reads them.
pub(super) fn with_opened(
    o: &Opts,
    opened: podvm_probe::isolation::Opened,
) -> podvm_supervise::table::Hardening {
    podvm_supervise::table::Hardening {
        opened: opened.words(),
        ..super::report::with_detached_path(o.hardening.clone(), o.detach)
    }
}

/// Make sure the store holds the image, honouring `--pull`.
///
/// ⛔ A trust arm still verifies: presence means blobs on disk, not a row
/// in the store (TODO/image.md T-1419). A record whose blobs are gone
/// refuses by image name with the remedy, never a raw blob path, and
/// `--pull always` stays the recovery path.
fn trust_record(
    verb: &str,
    store: &podvm_image::Store,
    image: &str,
    r: podvm_image::Record,
) -> std::result::Result<podvm_image::Record, i32> {
    // A presence check, not a hash (T-1604): the extraction hashes each
    // layer before it reads one, and `podvm verify` hashes them all.
    match store.absent_record_blobs(&r) {
        Ok(mismatches) if mismatches.is_empty() => Ok(r),
        Ok(mismatches) => {
            let digest_list = mismatches
                .iter()
                .map(|m| format!("{} (got {})", m.want, m.got))
                .collect::<Vec<_>>()
                .join(", ");
            eprintln!(
                "podvm {verb}: {image} is in the store but {} of its blob(s) \
                 are missing: {digest_list}. The store is at {}. \
                 Re-fetch it with --pull always (TODO/image.md T-1419)",
                mismatches.len(),
                store.root().display()
            );
            Err(podvm_image::error::EXIT_RUNTIME_ERROR)
        }
        Err(e) => {
            eprintln!("podvm {verb}: {image}: the store's blobs could not be verified: {e}");
            Err(e.exit_code())
        }
    }
}

/// The banner line of a store root that other users can reach, or nothing.
///
/// ⛔ TODO/supervise.md T-1715: the container record keeps each `-e` value
/// under the store root, so a root that group or other can reach gives the
/// values to each local user.
pub(super) fn store_note(store: &podvm_image::Store) -> String {
    match store.exposed_mode() {
        None => String::new(),
        Some(mode) => format!(
            "podvm: the store root {} is mode {mode:o}: other users can read the \
             container table, which keeps each -e value; chmod 700 it \
             (TODO/supervise.md T-1715)\n",
            store.root().display()
        ),
    }
}

/// ⛔ TODO/supervise.md T-1708: kill each process of the payload's tree that
/// stayed after the payload, and name a count that is not 0.
pub(super) fn end_tree(reaper: bool, err: &mut dyn std::io::Write) {
    match podvm_supervise::tree::end_own_tree() {
        podvm_supervise::Residual::Count(0) => {}
        podvm_supervise::Residual::Count(n) => {
            let _ = writeln!(
                err,
                "podvm run: {n} process(es) of the payload's tree stayed after SIGKILL \
                 (TODO/supervise.md T-1708)"
            );
        }
        podvm_supervise::Residual::Unknown => {
            let why = if reaper {
                "/proc cannot list them"
            } else {
                "the kernel refused PR_SET_CHILD_SUBREAPER"
            };
            let _ = writeln!(
                err,
                "podvm run: podvm cannot count the processes of the payload's tree that \
                 stayed: {why} (TODO/supervise.md T-1708)"
            );
        }
    }
}

/// The read-only table of the run in `env`, or a refusal by name where the
/// entry has no read-only layer (TODO/cli.md T-1655); then the host name of
/// the run (TODO/enter.md T-1908, T-1926).
pub(super) fn ro_env(
    verb: &str,
    o: &mut Opts,
    rootfs: &std::path::Path,
    userland: Option<&crate::lifecycle::Userland>,
    findings: &podvm_probe::Findings,
    env: &mut Vec<String>,
) -> Result<(), i32> {
    crate::lifecycle::volume::read_only_env(
        verb,
        o.read_only,
        &o.ro_volumes,
        &o.volumes,
        userland,
        env,
    )?;
    hostname_env(verb, o, rootfs, userland, findings, env)?;
    super::cgroup::apply(verb) // TODO/cli.md T-1747
}

/// TODO/enter.md T-1908 (PodUtils/podvm#402): `--hostname NAME`. The
/// handler of the native-static rung and the interposer of the loader
/// family answer `uname` and `gethostname` with the name of
/// `PODVM_HOSTNAME`; `/etc/hostname` of the tree holds it, `HOSTNAME`
/// names it, and `/etc/hosts` resolves it to 127.0.0.1, as docker does. An
/// entry with neither refuses by name: it has no UTS namespace.
pub(super) fn hostname_env(
    verb: &str,
    o: &mut Opts,
    rootfs: &std::path::Path,
    userland: Option<&crate::lifecycle::Userland>,
    findings: &podvm_probe::Findings,
    env: &mut Vec<String>,
) -> Result<(), i32> {
    env.retain(|e| !e.starts_with("PODVM_HOSTNAME="));
    crate::userns::bind_devices(rootfs); // TODO/probe.md T-1909
    crate::userns::owners::apply(rootfs); // TODO/probe.md T-1911
                                          // OQ-01 of the outside pass (T-1970): pod-enter mounts a tmpfs on
                                          // `/tmp` after podvm's binds, so a volume at or under `/tmp` goes to
                                          // pod-enter's own binds, which come after it.
    let (at_tmp, rest) = super::binds::split_tmp(&o.volumes);
    if userland.is_none() {
        // TODO/enter.md T-1920: the volumes as binds on the namespace rung.
        if let Err(e) =
            crate::userns::bind_volumes(rootfs, &rest, &o.ro_volumes, o.volume_base.as_deref())
        {
            eprintln!("podvm {verb}: {e}");
            return Err(podvm_image::error::EXIT_RUNTIME_ERROR);
        }
    }
    super::binds::push_env(
        &o.volumes,
        &o.ro_volumes,
        userland.is_none(),
        o.volume_base.as_deref(),
        env,
    ); // T-1920
    if userland.is_none() && crate::userns::active() {
        super::binds::push_foreground(&at_tmp, &o.ro_volumes, o.volume_base.as_deref(), env);
    }
    super::binds::push_volume_base(userland, o.volume_base.as_deref(), env); // T-1972
    let Some(name) = o.hostname.clone() else {
        return Ok(());
    };
    let held = userland.is_some_and(|u| {
        u.dispatch_exe.is_some() || u.family == podvm_enter::userland::Family::Loader
    });
    // TODO/probe.md T-1909: the run owns a UTS namespace.
    if crate::userns::owns_uts()
        && let Err(e) = crate::userns::set_hostname(&name)
    {
        eprintln!("podvm {verb}: sethostname in the UTS namespace of the run: {e}");
        return Err(podvm_image::error::EXIT_RUNTIME_ERROR);
    }
    // TODO/enter.md T-1926 (PodUtils/podrunner#196): `run -d` and `create`
    // of the namespace rung enter through pod-enter, whose plan opens a UTS
    // namespace and sets the name before the exec (`Plan::hostname`, the
    // launcher of `podvm-supervise`); where pod-enter falls back to the
    // chroot sequence, it refuses a plan with a name by itself.
    let by_enter = (o.detach || verb == "create")
        && userland.is_none()
        && podvm_enter::entered_rung(podvm_probe::select::Selection::choose(findings).rung)
            == podvm_probe::select::Rung::Namespace;
    let held = held || crate::userns::owns_uts() || by_enter;
    if !held {
        eprintln!(
            "podvm {verb}: --hostname needs the handler of the native-static rung or the \
             interposer of the loader family, and this entry has neither and no UTS \
             namespace: the payload would see the name of the host (TODO/enter.md T-1908)"
        );
        return Err(podvm_image::error::EXIT_RUNTIME_ERROR);
    }
    if let Err(e) = write_hostname(rootfs, &name) {
        eprintln!("podvm {verb}: /etc/hostname of the tree: {e}");
        return Err(podvm_image::error::EXIT_RUNTIME_ERROR);
    }
    if !o
        .env
        .iter()
        .any(|e| e == "HOSTNAME" || e.starts_with("HOSTNAME="))
    {
        env.retain(|e| !e.starts_with("HOSTNAME="));
        env.push(format!("HOSTNAME={name}"));
    }
    env.push(format!("PODVM_HOSTNAME={name}"));
    o.ask.add_hosts.push(format!("{name}:127.0.0.1"));
    Ok(())
}

/// Write `/etc/hostname` of the tree. A link there is removed first, so
/// that the write cannot leave the tree.
fn write_hostname(rootfs: &std::path::Path, name: &str) -> std::io::Result<()> {
    let etc = rootfs.join("etc");
    if !std::fs::symlink_metadata(&etc).is_ok_and(|m| m.is_dir()) {
        return Err(std::io::Error::other("etc is no directory of the tree"));
    }
    let f = etc.join("hostname");
    match std::fs::remove_file(&f) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&f)?;
    std::io::Write::write_all(&mut out, format!("{name}\n").as_bytes())
}

/// The directory of the `--tmpfs` directories of this process, under the
/// store. A foreground run removes it at its end. TODO/cli.md T-1655.
pub(crate) fn tmpfs_base(store: &podvm_image::Store) -> std::path::PathBuf {
    store
        .root()
        .join("tmpfs")
        .join(std::process::id().to_string())
}

/// Make an empty writable directory for each `--tmpfs` and add it to the
/// volumes of the run. TODO/cli.md T-1655. It is a directory of the store,
/// not a mount: its size and its mount options are not enforced, and the
/// row says so. A detached run refuses it by name, because nothing would
/// remove the directory at its end.
pub(super) fn add_tmpfs(verb: &str, store: &podvm_image::Store, o: &mut Opts) -> Result<(), i32> {
    if o.tmpfs.is_empty() {
        return Ok(());
    }
    if o.detach || verb == "create" {
        eprintln!(
            "podvm {verb}: --tmpfs is refused for a detached container: podvm makes it \
             a directory of the run and removes it when a foreground run ends \
             (TODO/cli.md T-1655)"
        );
        return Err(EXIT_FLAG_ERROR);
    }
    let base = tmpfs_base(store);
    for (i, guest) in o.tmpfs.clone().iter().enumerate() {
        let dir = base.join(i.to_string());
        let _ = std::fs::remove_dir_all(&dir);
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!("podvm {verb}: --tmpfs {guest}: {}: {e}", dir.display());
            return Err(podvm_image::error::EXIT_RUNTIME_ERROR);
        }
        o.volumes.push(podvm_enter::userland::Volume {
            guest: guest.clone(),
            host: dir.display().to_string(),
        });
    }
    Ok(())
}

/// Make sure the store holds the image, honouring `--pull`.
pub(super) fn acquire(
    verb: &str,
    store: &podvm_image::Store,
    image: &str,
    platform: &Platform,
    policy: &Policy,
    pull: &str,
) -> std::result::Result<podvm_image::Record, i32> {
    let found = store.find_for(image, Some(platform)).unwrap_or_default();
    let others = found.other_platforms.clone();
    let have = found.one();
    match (pull, have) {
        ("never", Some(r)) => trust_record(verb, store, image, r),
        ("never", None) => {
            // ⛔ Two different sentences for two different situations. "held,
            // for another platform" sends the caller to --platform; "not held"
            // sends them to a pull. Collapsing them into "no such image" is the
            // message a caller cannot act on.
            if others.is_empty() {
                eprintln!(
                    "podvm {verb}: {image} is not in the store and --pull never was \
                     given, so podvm will not fetch it"
                );
            } else {
                eprintln!(
                    "podvm {verb}: the store holds {image} for {}, and {platform} was \
                     asked for. --pull never was given, so podvm will not fetch \
                     the one that was asked for",
                    others.join(", ")
                );
            }
            Err(podvm_image::error::EXIT_RUNTIME_ERROR)
        }
        ("missing", Some(r)) => trust_record(verb, store, image, r),
        _ => {
            // ⛔ The transcript goes to STDERR here, unlike `podvm pull` where
            // it is the output. The payload owns stdout.
            let mut err = std::io::stderr().lock();
            match podvm_image::pull::pull(store, image, platform, policy, &mut err) {
                Ok(done) => Ok(done.record),
                Err(e) => {
                    let _ = writeln!(err, "podvm {verb}: {e}");
                    Err(e.exit_code())
                }
            }
        }
    }
}

/// Ensure the record is extracted, holding nothing: the caller holds the
/// image lock across the check and the extract (T-0204), because the
/// rootfs below must not be deleted between them. Shared by `run`'s
/// prepare and `cp`'s image addressing (TODO/cli.md T-1323): one path
/// for "the extracted rootfs in the store".
pub(crate) fn extract_now(
    verb: &str,
    store: &podvm_image::Store,
    record: &podvm_image::Record,
    loud: bool,
) -> std::result::Result<(), i32> {
    // ⛔ Every caller reads `is_extracted` before it calls this, and two of
    // them that read it together both extract the same tree. Measured
    // 2026-11-14: eight `podvm create` at once of a never-extracted image,
    // `v0.2.0-beta.16` created 12 of 48 containers over six rounds. The lock is
    // taken here, and the question is asked again under it, so the second
    // caller finds the tree of the first and does nothing.
    let _extracting = store.hold_extract(record).map_err(|e| {
        eprintln!("podvm {verb}: {e}");
        e.exit_code()
    })?;
    if podvm_extract::is_extracted(store, &record.manifest_digest) {
        return Ok(());
    }
    let d = podvm_image::digest::Digest::parse(&record.manifest_digest).map_err(|e| {
        eprintln!("podvm {verb}: {e}");
        podvm_image::Error::from(e).exit_code()
    })?;
    let bytes = store.read_blob(&d).map_err(|e| {
        eprintln!("podvm {verb}: {e}");
        e.exit_code()
    })?;
    let manifest: podvm_image::oci::Manifest = serde_json::from_slice(&bytes).map_err(|e| {
        eprintln!("podvm {verb}: the manifest does not parse: {e}");
        podvm_image::error::EXIT_RUNTIME_ERROR
    })?;
    // TODO/gate.md T-1852 (`img.load-run-quiet`): the line of each layer
    // is progress, which a quiet run does not print; `--verbose` prints it.
    // A failure and the warnings below print on each run.
    let mut layers: Vec<u8> = Vec::new();
    let done = podvm_extract::extract(store, &manifest, &record.manifest_digest, &mut layers);
    let mut err = std::io::stderr().lock();
    if loud || done.is_err() {
        let _ = err.write_all(&layers);
    }
    let done = done.map_err(|e| {
        let _ = writeln!(err, "podvm {verb}: {e}");
        podvm_image::error::EXIT_RUNTIME_ERROR
    })?;
    // TODO/probe.md T-1909: in the user namespace of a run, an id that the
    // map does not hold stays the caller's, as outside it; the sidecar
    // keeps the intended ids, and a quiet run says nothing.
    if !crate::userns::active() {
        crate::diagnose::report_dropped(&mut err, &done, &podvm_probe::identity::read(), false);
    }
    crate::diagnose::report_skipped(&mut err, &done); // TODO/image.md T-1883
    Ok(())
}

/// The image's config blob.
///
/// ⚠ `pub(crate)` because `exec` reads the same blob for the same reason, and a
/// second copy of it is the copy that drifts: `docs/conventions/code.md`.
pub(crate) fn config_of(
    verb: &str,
    store: &podvm_image::Store,
    record: &podvm_image::Record,
) -> std::result::Result<podvm_image::oci::Config, i32> {
    let d = podvm_image::digest::Digest::parse(&record.config_digest).map_err(|e| {
        eprintln!("podvm {verb}: {e}");
        podvm_image::Error::from(e).exit_code()
    })?;
    let bytes = store.read_blob(&d).map_err(|e| {
        eprintln!("podvm {verb}: {e}");
        e.exit_code()
    })?;
    serde_json::from_slice(&bytes).map_err(|e| {
        eprintln!("podvm {verb}: the image config does not parse: {e}");
        podvm_image::error::EXIT_RUNTIME_ERROR
    })
}
