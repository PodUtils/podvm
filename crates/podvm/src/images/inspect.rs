use super::*;

/// The provenance half of `verify <image>`: how this record's bytes got
/// here, or the honest absence where no pull recorded one.
pub(super) fn print_provenance(store: &Store, record: &podvm_image::Record) {
    match store.read_provenance() {
        Ok(lines) => {
            let mut shown = 0;
            for p in lines
                .iter()
                .filter(|p| p.manifest_digest == record.manifest_digest)
            {
                println!("registry: {}", p.registry);
                println!("repository: {}", p.repository);
                println!("tag: {}", p.tag.as_deref().unwrap_or("(untagged)"));
                println!("manifest: {}", p.manifest_digest);
                println!("pulled-at: {}", p.pulled_at);
                println!("podvm-version: {}", p.podvm_version);
                shown += 1;
            }
            if shown == 0 {
                println!(
                    "no provenance line for {} (recorded before T-1321, imported, or loaded)",
                    record.manifest_digest
                );
            }
        }
        Err(e) => eprintln!("podvm verify: {e}"),
    }
}

pub fn inspect(verb: &str, args: &[String]) -> i32 {
    // ⭐ TODO/cli.md T-1330: bundled shorts expand before admission.
    let expanded: Vec<String> = match crate::parity::expand(verb, args) {
        Ok(a) => a,
        Err(member) => return crate::parity::refuse_member(verb, &member, INSPECT_USAGE),
    };
    let args: &[String] = &expanded;
    if let Some(c) = crate::parity::admit_all(verb, args, INSPECT_USAGE) {
        return c;
    }
    let mut template: Option<String> = None;
    let mut wanted: Vec<&str> = Vec::new();
    let mut verbose = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{INSPECT_USAGE}");
                return 0;
            }
            "-f" | "--format" => match it.next() {
                Some(t) => template = Some(t.clone()),
                None => {
                    eprintln!("podvm inspect: --format needs a template");
                    return EXIT_FLAG_ERROR;
                }
            },
            other if other.starts_with("--format=") => {
                template = Some(other["--format=".len()..].to_string())
            }
            // T-2104: the reason of a dash of an image (`.UnpackedBytes`);
            // T-2076 gives the same flag the reasons of an instance.
            "--verbose" => verbose = true,
            other if other.starts_with('-') => {
                if let Err(c) = crate::parity::admit(verb, other, INSPECT_USAGE) {
                    return c;
                }
                return crate::parity::no_arm(verb, other);
            }
            other => wanted.push(other),
        }
    }
    if wanted.is_empty() {
        eprint!("{INSPECT_USAGE}");
        return EXIT_CLI_ERROR;
    }
    // ⛔ Before the store is even opened. Every reference may fail to resolve,
    // and a template checked only inside the loop over what DID resolve is
    // never checked at all. It is also the caller's own input, so it is
    // reported before anything about the machine is.
    // ⚠ Checked against the IMAGE fields here and against the container's in
    // `lifecycle::inspect_container`, because the two kinds have different
    // fields and a union would accept a name neither of them has.
    // TODO/cli.md T-2087 (OR-B40-15): where the template fits neither kind,
    // the line names the part that fails for each kind.
    if let Some(t) = &template
        && let Err(image) = format::check(t, INSPECT_FIELDS)
        && let Err(container) = format::check(t, crate::lifecycle::CONTAINER_INSPECT_FIELDS)
    {
        match (&image, &container) {
            (format::Bad::UnknownField { got, .. }, format::Bad::UnknownField { got: g2, .. })
                if got == g2 =>
            {
                eprintln!("podvm inspect: no image and no container has the field {got:?}")
            }
            (format::Bad::UnknownField { .. }, format::Bad::UnknownField { .. }) => eprintln!(
                "podvm inspect: the template fits no image and no container: as an image, {}; \
                 as a container, {}",
                image.short(),
                container.short()
            ),
            _ => eprintln!("podvm inspect: {image}"),
        }
        return EXIT_CLI_ERROR;
    }
    let store = match podvm_image::open_store() {
        Ok(s) => s,
        Err(e) => return fail(e),
    };

    // PodUtils/podvm#514, #523: one document for each reference, in the
    // order given, and one JSON array for all of them (two arrays were two
    // documents that no JSON parser reads as one). A container first, then
    // an image, which is docker's order for an untyped inspect. ⚠ The two
    // have different fields, so a template is checked against the kind that
    // the reference resolved to.
    let mut docs: Vec<String> = Vec::new();
    let mut code = 0;
    for want in &wanted {
        if let Some(d) = crate::lifecycle::inspect_container(want, template.as_deref()) {
            if verbose {
                eprintln!("podvm inspect: {want}: --verbose has no effect on a container");
            }
            match d {
                Ok(d) => docs.push(d),
                Err(c) => code = c,
            }
            continue;
        }
        let r = match store.find_one(want) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("podvm inspect: {e}");
                code = e.exit_code();
                continue;
            }
        };
        // T-2104: the unpacked bytes, from the record or from its layers.
        let (r, why) = podvm_image::unpacked::complete(&store, r);
        if let (true, Some(why)) = (verbose, why) {
            eprintln!("podvm inspect: {want}: .UnpackedBytes is a dash: {why}");
        }
        match &template {
            Some(t) => match format::Template::parse(t).and_then(|t| {
                let data = format::Data::typed(&inspect_fields(&r, &store), &[]);
                t.render(&data.with_root(inspect_doc(&r, &store)))
            }) {
                Ok(line) => docs.push(line),
                Err(bad) => {
                    eprintln!("podvm inspect: {bad}");
                    code = EXIT_CLI_ERROR;
                }
            },
            None => docs.push(inspect_json(&r, &store)),
        }
    }
    if template.is_some() {
        for line in &docs {
            println!("{line}");
        }
    } else {
        println!("[{}]", docs.join(","));
    }
    code
}

// ------------------------------------------------------------------- fields

/// The field names `podvm images --format` answers to.
///
/// ⚠ These names exist so a template can be validated with NO record in hand,
/// which is what an empty store leaves the caller with. The test
/// `the_field_names_are_the_names_the_builder_produces` is what stops this list
/// and the builder below drifting: `docs/conventions/forbidden-patterns.md`
/// forbids a value in two places with no check that they agree.
pub const IMAGE_FIELDS: &[&str] = &[
    "Repository",
    "Tag",
    "ID",
    "Digest",
    "CreatedSince",
    "CreatedAt",
    "Size",
    "Platform",
    "Store",
];

/// The same, for `podvm inspect --format`.
pub const INSPECT_FIELDS: &[&str] = &[
    "Id",
    "Digest",
    "RepoTags",
    "RepoDigests",
    "Architecture",
    "Os",
    "Created",
    "Platform",
    "Size",
    // T-2104: the bytes of the unpacked root file system, null where unknown.
    "UnpackedBytes",
    "Store",
    "Layers",
    // ⭐ M2. `TODO/extract.md` T-0302's Prove reads the sidecar at
    // `<RootfsPath>/../.meta.jsonl`, so this field is what makes that command
    // writable at all.
    "RootfsPath",
    "Extracted",
    // ⛔ TODO/enter.md T-0505. What `podvm exec` against this image WOULD be,
    // and not a property of the image. It is constant because podvm has one
    // exec mechanism; when M4 gives a container its own record the same two
    // names sit on that record and answer from the container that was entered.
    "Exec.Mode",
    "Exec.Shares",
    // ⭐ TODO/cli.md T-1417. What the image declares, beside what podvm
    // applied: dotted names are single registered names (T-0505), so an
    // unregistered one is still refused rather than rendering half.
    "Config.Env",
    "Config.Cmd",
    "Config.Entrypoint",
    "Config.WorkingDir",
    "Config.User",
    "Config.Labels",
    // T-2060: the whole object, as `{{json .Config}}` reads it in docker.
    "Config",
];

/// `podvm exec`'s mode, in one word each, for `inspect` and for the banner.
///
/// ⛔ TODO/enter.md T-0505. `docker exec` enters the container's namespaces.
/// podvm has none to enter, so its `exec` re-runs the section 6.5 sequence
/// against the same rootfs: it shares the filesystem tree and NOTHING else, not
/// the process table, not `/proc`, not signals, not the original's environment.
/// A caller reads these rather than assuming.
pub const EXEC_MODE: &str = podvm_probe::isolation::FRESH;
pub const EXEC_SHARES: &str = podvm_probe::isolation::SHARES;

/// The IMAGE ID of a record: the first twelve hex digits of its config
/// digest, the id of `images`, `inspect` and `system df` (TODO/image.md
/// T-1814).
pub(crate) fn short_image_id(r: &Record) -> String {
    let id = &r.config_digest;
    id.strip_prefix("sha256:")
        .map(|h| h[..12.min(h.len())].to_string())
        .unwrap_or_else(|| id.clone())
}

pub(super) fn image_fields(
    r: &Record,
    store: &Store,
    no_trunc: bool,
) -> Vec<(&'static str, String)> {
    let id = r.config_digest.clone();
    let short_id = short_image_id(r);
    vec![
        ("Repository", r.display_repository()),
        ("Tag", r.tag.clone().unwrap_or_else(|| "<none>".into())),
        ("ID", if no_trunc { id } else { short_id }),
        ("Digest", r.digest.clone()),
        (
            "CreatedSince",
            r.created
                .as_deref()
                .and_then(clock::since)
                // ⛔ A dash where the value is unknown, never a fabricated one.
                .unwrap_or_else(|| "-".into()),
        ),
        ("CreatedAt", r.created.clone().unwrap_or_else(|| "-".into())),
        ("Size", space::mib(r.stored_bytes)),
        ("Platform", r.platform.clone()),
        ("Store", store.root().display().to_string()),
    ]
}

/// The fields of `--format`, each read from the document of `inspect`, so
/// that the two cannot differ (T-2060): a dotted name is a path into it.
pub(super) fn inspect_fields(r: &Record, store: &Store) -> Vec<(&'static str, serde_json::Value)> {
    let doc = inspect_doc(r, store);
    INSPECT_FIELDS
        .iter()
        .map(|name| {
            let at = name
                .split('.')
                .try_fold(&doc, |v, part| v.get(part))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            (*name, at)
        })
        .collect()
}

/// TODO/cli.md T-1417. The image's declared `Config`, read through the
/// stored config digest.
///
/// `None` where the blob is missing or does not parse: `inspect` is a
/// reader, and a broken store is `verify`'s to report (T-1321), so the
/// Config half renders null rather than failing the whole document.
pub(super) fn run_config_of(store: &Store, r: &Record) -> Option<podvm_image::oci::RunConfig> {
    let d = podvm_image::digest::Digest::parse(&r.config_digest).ok()?;
    let bytes = store.read_blob(&d).ok()?;
    let cfg: podvm_image::oci::Config = serde_json::from_slice(&bytes).ok()?;
    Some(cfg.config)
}

/// The Config half as a JSON document with docker's key names, null where
/// the image omits the field. Reported beside what podvm applied, never
/// merged with it.
pub(super) fn config_json(c: &Option<podvm_image::oci::RunConfig>) -> serde_json::Value {
    let null = serde_json::Value::Null;
    match c {
        None => serde_json::json!({
            "Env": null, "Cmd": null, "Entrypoint": null,
            "WorkingDir": null, "User": null,
        }),
        Some(c) => serde_json::json!({
            "Env": if c.env.is_empty() { null.clone() } else { serde_json::Value::from(c.env.clone()) },
            "Cmd": c.cmd.clone().map(serde_json::Value::from).unwrap_or(null.clone()),
            "Entrypoint": c.entrypoint.clone().map(serde_json::Value::from).unwrap_or(null.clone()),
            "WorkingDir": if c.working_dir.is_empty() { null.clone() } else { serde_json::Value::from(c.working_dir.clone()) },
            "User": if c.user.is_empty() { null } else { serde_json::Value::from(c.user.clone()) },
        }),
    }
}

pub(super) fn inspect_json(r: &Record, store: &Store) -> String {
    inspect_doc(r, store).to_string()
}

/// The one model of an image's fields: `inspect` prints it, and `--format`
/// reads its parts.
fn inspect_doc(r: &Record, store: &Store) -> serde_json::Value {
    let (rootfs, _) = podvm_extract::paths(store, &r.manifest_digest);
    // TODO/image.md T-1840: `.Config.Labels` beside the run config.
    let mut config = config_json(&run_config_of(store, r));
    config["Labels"] = super::table_more::labels_json(store, r);
    // ⚠ Built through serde_json rather than by concatenation, so a repository
    // or a tag carrying a quote cannot break the document.
    serde_json::json!({
        "Id": r.config_digest,
        "Digest": r.digest,
        "RepoTags": r.tag.as_ref().map(|_| vec![r.name()]).unwrap_or_default(),
        "RepoDigests": [format!("{}@{}", r.display_repository(), r.digest)],
        "Architecture": r.architecture,
        "Os": r.os,
        "Created": r.created,
        "Platform": r.platform,
        "Size": r.stored_bytes,
        "UnpackedBytes": r.unpacked_bytes,
        "Store": store.root().display().to_string(),
        "Layers": r.layers,
        "ManifestDigest": r.manifest_digest,
        "PulledAt": r.pulled_at,
        // ⭐ TODO/cli.md T-1417. What the image declares, with docker's key
        // names and null where it omits a field, beside what podvm applied.
        "Config": config,
        // ⛔ T-0505, and nested here because it is nested in --format too. A
        // caller that reads one and not the other must not find two shapes.
        "Exec": { "Mode": EXEC_MODE, "Shares": EXEC_SHARES },
        "RootfsPath": rootfs.display().to_string(),
        // ⚠ Whether the rootfs is THERE, which is a different question from
        // where it would be.
        "Extracted": podvm_extract::is_extracted(store, &r.manifest_digest),
    })
}

pub(super) fn pick(fields: &[(&str, String)], key: &str) -> String {
    fields
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

/// docker's two-space-padded columns.
pub(crate) fn table(rows: &[Vec<String>]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; columns];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            // TODO/supervise.md T-1858: columns of the terminal.
            widths[i] = widths[i].max(crate::width::str_width(cell));
        }
    }
    let mut out = String::new();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i + 1 == row.len() {
                out.push_str(cell);
            } else {
                out.push_str(cell);
                out.push_str(&" ".repeat(widths[i] - crate::width::str_width(cell) + 3));
            }
        }
        out.push('\n');
    }
    out
}

/// TODO/image.md T-1419. The record's blobs that have no file in the
/// store, by digest.
///
/// Presence only, never a hash: `images` marks blob-less records while
/// listing, and hashing every blob of every listed image on every listing
/// is `verify`'s job. An unparseable digest counts as missing, because no
/// file under that name can be the blob the record names.
pub(super) fn missing_blobs(store: &Store, r: &Record) -> Vec<String> {
    let mut out = Vec::new();
    for b in r.blobs() {
        match podvm_image::digest::Digest::parse(b) {
            Ok(d) if store.blob_path(&d).exists() => {}
            _ => out.push(b.to_string()),
        }
    }
    out
}

pub(super) fn unknown(verb: &str, flag: &str, usage: &str) -> i32 {
    eprintln!("podvm {verb}: unknown option {flag:?}");
    eprint!("{usage}");
    EXIT_FLAG_ERROR
}

pub(super) fn fail(e: Error) -> i32 {
    eprintln!("podvm: {e}");
    e.exit_code()
}
