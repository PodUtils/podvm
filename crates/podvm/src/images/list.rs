use super::*;

struct Options {
    quiet: bool,
    digests: bool,
    no_trunc: bool,
    format: Option<String>,
    filter: Option<String>,
    filters: Vec<crate::parity::Filter>,
}

/// `podvm images`.
pub fn images(verb: &str, args: &[String]) -> i32 {
    // ⭐ TODO/cli.md T-1330: bundled shorts expand before admission.
    let expanded: Vec<String> = match crate::parity::expand(verb, args) {
        Ok(a) => a,
        Err(member) => return crate::parity::refuse_member(verb, &member, IMAGES_USAGE),
    };
    let args: &[String] = &expanded;
    if let Some(c) = crate::parity::admit_all(verb, args, IMAGES_USAGE) {
        return c;
    }
    let mut o = Options {
        quiet: false,
        digests: false,
        no_trunc: false,
        format: None,
        filter: None,
        filters: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{IMAGES_USAGE}");
                return 0;
            }
            "-a" | "--all" => {}
            "-q" | "--quiet" => o.quiet = true,
            "--digests" => o.digests = true,
            "--no-trunc" => o.no_trunc = true,
            "-f" | "--filter" => match it.next() {
                Some(f) => match crate::parity::parse_filter(f) {
                    Ok(p) => o.filters.push(p),
                    Err(bad) => {
                        eprintln!("podvm images: {bad}");
                        return EXIT_FLAG_ERROR;
                    }
                },
                None => {
                    eprintln!("podvm images: --filter needs a key=value");
                    return EXIT_FLAG_ERROR;
                }
            },
            other if other.starts_with("--filter=") => {
                match crate::parity::parse_filter(&other["--filter=".len()..]) {
                    Ok(p) => o.filters.push(p),
                    Err(bad) => {
                        eprintln!("podvm images: {bad}");
                        return EXIT_FLAG_ERROR;
                    }
                }
            }
            "--format" => match it.next() {
                Some(t) => o.format = Some(t.clone()),
                None => {
                    eprintln!("podvm images: --format needs a template");
                    return EXIT_FLAG_ERROR;
                }
            },
            other if other.starts_with("--format=") => {
                o.format = Some(other["--format=".len()..].to_string())
            }
            other if other.starts_with('-') => {
                if let Err(c) = crate::parity::admit(verb, other, IMAGES_USAGE) {
                    return c;
                }
                return crate::parity::no_arm(verb, other);
            }
            other if o.filter.is_none() => o.filter = Some(other.to_string()),
            other => {
                eprintln!("podvm images: {other:?}: images takes at most one image");
                return EXIT_CLI_ERROR;
            }
        }
    }
    if o.quiet && o.format.is_none() {
        o.format = Some("{{.ID}}".to_string());
    }
    // ⛔ Validated BEFORE the store is opened, and therefore before the loop
    // over records. An empty store means zero iterations, and a template
    // checked only inside the loop is never checked at all: measured on
    // 2026-09-08, `--format '{{.Nope}}'` against an empty store printed
    // nothing and exited 0, so a caller's typo read as an empty result set.
    if let Some(t) = &o.format
        && let Err(bad) = format::check(t, IMAGE_FIELDS)
    {
        eprintln!("podvm images: {bad}");
        return EXIT_CLI_ERROR;
    }

    let store = match podvm_image::open_store() {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let records = match &o.filter {
        // TODO/image.md T-1840: a repository with no tag lists each tag.
        Some(f) => {
            match super::table_more::repository_filter(&store, f).unwrap_or_else(|| store.find(f)) {
                Ok(r) => r,
                Err(e) => return fail(e),
            }
        }
        None => match store.list() {
            Ok(r) => r,
            Err(e) => return fail(e),
        },
    };
    // ⭐ TODO/cli.md T-1331. Caller-side predicate over the listed
    // records: `name=` matches the repository or tag substring, `label=`
    // the labels of the image's OCI config (T-1695), every filter must match.
    let records: Vec<_> = records
        .into_iter()
        .filter(|r| {
            o.filters.iter().all(|f| match f.key.as_str() {
                "name" => {
                    r.repository.contains(&f.value)
                        || r.tag.as_deref().is_some_and(|t| t.contains(&f.value))
                }
                "label" => label_matches(&labels_of(&store, r), &f.value),
                _ => false,
            })
        })
        .collect();

    let mut out = std::io::stdout().lock();
    // ⭐ TODO/image.md T-1419. A record whose blobs are gone is marked
    // here, on stderr so the table and template contracts are untouched:
    // presence is a stat per blob, not a hash, because `images` lists and
    // `verify` proves. `run` refuses such a record (see `trust_record`);
    // `--pull always` re-fetches it.
    for r in &records {
        let missing = missing_blobs(&store, r);
        if !missing.is_empty() {
            eprintln!(
                "podvm images: {}: {} of its blob(s) are missing from the \
                 store at {}: {}. `podvm verify {}` reports it; re-fetch \
                 with --pull always (TODO/image.md T-1419)",
                r.name(),
                missing.len(),
                store.root().display(),
                missing.join(", "),
                r.name(),
            );
        }
    }
    if let Some(template) = &o.format {
        for r in &records {
            match format::render(template, &image_fields(r, &store, o.no_trunc)) {
                Ok(line) => {
                    if let Err(e) = writeln!(out, "{line}") {
                        return crate::names::stdout::failed(&e); // T-1816
                    }
                }
                Err(bad) => {
                    eprintln!("podvm images: {bad}");
                    return EXIT_CLI_ERROR;
                }
            }
        }
        return 0;
    }

    let mut header: Vec<&str> = vec!["REPOSITORY", "TAG"];
    if o.digests {
        header.push("DIGEST");
    }
    let platform = super::table_more::needs_platform(&records); // T-1840
    if platform {
        header.push("PLATFORM");
    }
    header.extend(["IMAGE ID", "CREATED", "SIZE (STORED)"]);
    let mut rows: Vec<Vec<String>> = vec![header.iter().map(|s| s.to_string()).collect()];
    for r in &records {
        let f = image_fields(r, &store, o.no_trunc);
        let get = |k: &str| pick(&f, k);
        let mut row = vec![get("Repository"), get("Tag")];
        if o.digests {
            row.push(get("Digest"));
        }
        if platform {
            row.push(r.platform.clone());
        }
        row.extend([get("ID"), get("CreatedSince"), get("Size")]);
        rows.push(row);
    }
    // T-1861; PodUtils/podvm#476: the rows of Windows take the filter too.
    rows.extend(
        crate::windows::pull::image_rows(o.digests)
            .into_iter()
            .filter(|w| {
                o.filters
                    .iter()
                    .all(|f| f.key == "name" && w.iter().take(2).any(|c| c.contains(&f.value)))
            }),
    );
    // TODO/cli.md T-1816: a lost table is 125, not 0.
    match write!(out, "{}", table(&rows)).and_then(|()| out.flush()) {
        Ok(()) => 0,
        Err(e) => crate::names::stdout::failed(&e),
    }
}

/// `podvm rmi` and `podvm image rm`.
pub fn rmi(verb: &str, args: &[String]) -> i32 {
    // ⭐ TODO/cli.md T-1330: bundled shorts expand before admission.
    let expanded: Vec<String> = match crate::parity::expand(verb, args) {
        Ok(a) => a,
        Err(member) => return crate::parity::refuse_member(verb, &member, RMI_USAGE),
    };
    let args: &[String] = &expanded;
    if let Some(c) = crate::parity::admit_all(verb, args, RMI_USAGE) {
        return c;
    }
    let mut wanted: Vec<&str> = Vec::new();
    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{RMI_USAGE}");
                return 0;
            }
            // docker's `-f` forces removal of a tagged image. podvm already
            // removes every name the reference gives; what it refuses is an
            // image IN USE, and no flag overrides that, because the deletion
            // would be under a running payload.
            "-f" | "--force" => {}
            other if other.starts_with('-') => {
                if let Err(c) = crate::parity::admit(verb, other, RMI_USAGE) {
                    return c;
                }
                return crate::parity::no_arm(verb, other);
            }
            other => wanted.push(other),
        }
    }
    if wanted.is_empty() {
        eprint!("{RMI_USAGE}");
        return EXIT_CLI_ERROR;
    }
    let store = match podvm_image::open_store() {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let mut code = 0;
    for want in wanted {
        if let Some(name) = want.strip_prefix("windows:") {
            code = code.max(crate::windows::pull::rmi_named(verb, name)); // T-1861
            continue;
        }
        // ⭐ TODO/image.md T-1322. Records before holds: a created
        // container takes no hold, so only its record sees the reference,
        // and docker's rule keys on any container, running or stopped.
        // Every record the name resolves to is checked, and
        // `store.remove` below stays the last word on running payloads.
        // Unresolvable here is `remove`'s own error to report below,
        // once, rather than two refusals for one name.
        if let Ok(records) = store.find(want) {
            let mut failed = false;
            let mut referrers: Vec<String> = Vec::new();
            // PodUtils/podvm#548: a manifest that another record still names
            // stays, so a removal of this name is an untag, as docker's.
            let others = store.list().unwrap_or_default();
            for record in &records {
                let named_elsewhere = others.iter().any(|o| {
                    o.manifest_digest == record.manifest_digest
                        && !records
                            .iter()
                            .any(|r| r.repository == o.repository && r.tag == o.tag)
                });
                if named_elsewhere {
                    continue;
                }
                match podvm_supervise::referencing(&store, &record.manifest_digest) {
                    Ok(found) => referrers.extend(found.iter().map(|c| c.name.clone())),
                    Err(e) => {
                        eprintln!("podvm rmi: {e}");
                        code = EXIT_RUNTIME_ERROR;
                        failed = true;
                    }
                }
            }
            if failed {
                continue;
            }
            if !referrers.is_empty() {
                referrers.sort();
                referrers.dedup();
                eprintln!(
                    "podvm rmi: {want} is referenced by container {} and was not removed",
                    referrers.join(", ")
                );
                code = EXIT_CLI_ERROR; // TODO/cli.md T-2062: docker's 1
                continue;
            }
        }
        let digests: Vec<String> = store
            .find(want)
            .map(|v| v.iter().map(|r| r.manifest_digest.clone()).collect())
            .unwrap_or_default();
        match store.remove(want) {
            Ok(done) => {
                super::drop_extractions(&store, &digests);
                for name in &done.untagged {
                    println!("Untagged: {name}");
                }
                for d in &done.deleted {
                    println!("Deleted: {d}");
                }
            }
            Err(e) => {
                eprintln!("podvm rmi: {e}");
                code = e.exit_code();
            }
        }
    }
    code
}

/// `podvm tag`.
pub fn tag(verb: &str, args: &[String]) -> i32 {
    // ⭐ TODO/cli.md T-1330: bundled shorts expand before admission.
    let expanded: Vec<String> = match crate::parity::expand(verb, args) {
        Ok(a) => a,
        Err(member) => return crate::parity::refuse_member(verb, &member, TAG_USAGE),
    };
    let args: &[String] = &expanded;
    if let Some(c) = crate::parity::admit_all(verb, args, TAG_USAGE) {
        return c;
    }
    let mut positional: Vec<&str> = Vec::new();
    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{TAG_USAGE}");
                return 0;
            }
            other if other.starts_with('-') => {
                if let Err(c) = crate::parity::admit(verb, other, TAG_USAGE) {
                    return c;
                }
                return crate::parity::no_arm(verb, other);
            }
            other => positional.push(other),
        }
    }
    if positional.len() != 2 {
        eprint!("{TAG_USAGE}");
        return EXIT_CLI_ERROR;
    }
    let store = match podvm_image::open_store() {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    match store.tag(positional[0], positional[1]) {
        Ok(_) => 0,
        Err(e) => fail(e),
    }
}

/// `podvm login`.
pub fn login(verb: &str, args: &[String]) -> i32 {
    // ⭐ TODO/cli.md T-1330: bundled shorts expand before admission.
    let expanded: Vec<String> = match crate::parity::expand(verb, args) {
        Ok(a) => a,
        Err(member) => return crate::parity::refuse_member(verb, &member, LOGIN_USAGE),
    };
    let args: &[String] = &expanded;
    if let Some(c) = crate::parity::admit_all(verb, args, LOGIN_USAGE) {
        return c;
    }
    let parsed = match parse_login_args(verb, args) {
        Ok(p) => p,
        Err(c) => {
            eprint!("{LOGIN_USAGE}");
            return c;
        }
    };
    if parsed.help {
        print!("{LOGIN_USAGE}");
        return 0;
    }
    let server = parsed
        .server
        .as_deref()
        .unwrap_or_else(|| crate::images::default_login_server());
    let password = match read_password(verb, std::io::stdin().is_terminal(), &mut std::io::stdin())
    {
        Ok(p) => p,
        Err(c) => return c,
    };
    match credentials::store_login(server, &parsed.username, &password) {
        Ok(()) => {
            // #559: docker's line on stdout; stderr says that no registry checked it.
            eprintln!("podvm login: stored for {server}; the registry checks it at the next pull");
            println!("Login Succeeded");
            0
        }
        Err(e) => {
            eprintln!("podvm login: {e}");
            EXIT_RUNTIME_ERROR
        }
    }
}

/// The password off stdin, trimmed of trailing newlines.
///
/// ⭐ TODO/image.md T-0209: a terminal stdin would hang the read, so the
/// refusal comes first and names the pipe. podvm never prompts: the help
/// documents stdin-only passwords, and a caller with no pipe is told
/// rather than hung. Pure over its inputs so tests own both arms: the
/// refusal precedes any read (a reader that panics proves it), and a pipe
/// reads to the trimmed password.
pub(super) fn read_password(
    verb: &str,
    stdin_is_terminal: bool,
    stdin: &mut dyn std::io::Read,
) -> Result<String, i32> {
    if stdin_is_terminal {
        eprintln!(
            "podvm {verb}: stdin is a terminal; pipe the password with --password-stdin \
             (podvm never prompts for one)"
        );
        return Err(EXIT_CLI_ERROR);
    }
    let mut password = String::new();
    if let Err(e) = stdin.read_to_string(&mut password) {
        eprintln!("podvm login: the password on stdin does not read: {e}");
        return Err(EXIT_CLI_ERROR);
    }
    while password.ends_with('\n') || password.ends_with('\r') {
        password.pop();
    }
    if password.is_empty() {
        eprintln!("podvm login: the password on stdin is empty"); // #500
        return Err(EXIT_CLI_ERROR);
    }
    Ok(password)
}

/// The labels of an image: the `config.Labels` object of its OCI config
/// blob, as `KEY=VALUE` (TODO/cli.md T-1695). None where the blob is gone
/// or holds none.
pub(super) fn labels_of(store: &podvm_image::Store, r: &podvm_image::Record) -> Vec<String> {
    let Ok(d) = podvm_image::digest::Digest::parse(&r.config_digest) else {
        return Vec::new();
    };
    let Ok(bytes) = store.read_blob(&d) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Vec::new();
    };
    v.get("config")
        .and_then(|c| c.get("Labels"))
        .and_then(|l| l.as_object())
        .map(|m| {
            m.iter()
                .map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or_default()))
                .collect()
        })
        .unwrap_or_default()
}

/// docker's rule: `KEY` matches a label with that key, `KEY=VALUE` the
/// label exactly.
pub(super) fn label_matches(labels: &[String], want: &str) -> bool {
    labels.iter().any(|l| match want.split_once('=') {
        Some(_) => l == want,
        None => l.split('=').next() == Some(want),
    })
}

#[cfg(test)]
mod label_tests {
    use super::label_matches;

    #[test]
    fn an_image_label_matches_by_key_or_by_key_and_value() {
        let l = vec!["org.opencontainers.image.version=3.20".to_string()];
        assert!(label_matches(&l, "org.opencontainers.image.version"));
        assert!(label_matches(&l, "org.opencontainers.image.version=3.20"));
        assert!(!label_matches(&l, "org.opencontainers.image.version=3.21"));
        assert!(!label_matches(&[], "a"));
    }
}
