use super::*;

/// Refuse a cluster member no row names, in `admit`'s wording.
///
/// Expansion happens before admission, so the member (not the cluster)
/// is what has no row. The usage line matches `admit`'s: one refusal
/// shape for one surface.
pub fn refuse_member(verb: &str, member: &str, usage: &str) -> i32 {
    if let Some((name, _)) = member.split_once('=')
        && BOOLEAN_LONGS.contains(&name)
    {
        eprintln!(
            "podvm {verb}: {name} takes no value; it takes `{name}`, `{name}=true` \
             or `{name}=false`, and {member:?} is none of them (TODO/gate.md T-1853)"
        );
        short_usage(verb, usage);
        return podvm_image::error::EXIT_FLAG_ERROR;
    }
    eprintln!(
        "podvm {verb}: unknown option {member:?}. It has no row in the parity \
         table; `podvm system info` lists every flag this verb takes"
    );
    short_usage(verb, usage);
    podvm_image::error::EXIT_FLAG_ERROR
}

/// One `--filter key=value` predicate, for `ps` and `images`.
///
/// TODO/cli.md T-1331: caller-side, no new query language. `name=`
/// matches a substring; `label=` matches the labels of a container record
/// or of an image's OCI config (T-1695). Anything else is the caller's mistake and refuses
/// naming the two keys that exist.
pub struct Filter {
    pub key: String,
    pub value: String,
}

/// Split one `--filter` value into its key and value.
pub fn parse_filter(raw: &str) -> Result<Filter, String> {
    match raw.split_once('=') {
        Some((k, v)) if !k.is_empty() => {
            let key = k.trim().to_string();
            if key == "name" || key == "label" {
                Ok(Filter {
                    key,
                    value: v.to_string(),
                })
            } else {
                Err(format!(
                    "--filter takes name= and label=, not {raw:?}: podvm records \
                     carry no other key to filter on"
                ))
            }
        }
        _ => Err(format!(
            "--filter takes key=value, not {raw:?}: `name=<substring>` selects, \
             `label=<key>[=<value>]` matches the labels of a run"
        )),
    }
}

/// The verbs whose parser reads a value of `--strict` (`--strict=all`).
const STRICT_VALUE: [&str; 3] = ["run", "create", "exec"];

/// `-aq` becomes `-a -q`; a member that takes a value consumes the rest
/// (`-t5` becomes `-t 5`, `-eFOO=bar` becomes `-e FOO=bar`,
/// `-f{{.Id}}` becomes `-f {{.Id}}`); an unknown member refuses naming
/// the member. Only the first member decides the shape: past a
/// value-taking member the rest is opaque, past a value-less one every
/// member is another flag. Verbs that stop at the image (`run`, `exec`)
/// expand only before it: `run IMG -la` passes `-la` to the payload,
/// exactly as their parsers do past the image name. Every other verb
/// reads its whole argv, and clusters expand anywhere in it. Long flags,
/// lone shorts, a lone `-`, and anything not opening with an
/// alphanumeric short pass through for the parser (and `admit`) to judge
/// as before: a value that starts with `-` must still use the `=` form.
pub fn expand(verb: &str, args: &[String]) -> Result<Vec<String>, String> {
    let under = rows_of(verb);
    // A leading positional is the image (or the whole argv is podvm's):
    // a value follows only its flag, so the walk starts past nothing.
    let bounded = CLUSTER_BOUNDARY.contains(&under);
    let mut prev_dash = !bounded;
    let mut out = Vec::with_capacity(args.len());
    let mut i = 0;
    // TODO/cli.md T-1829: the word after a valued long flag is its value,
    // never a bundle of short flags (`logs --tail -1`).
    let mut value_next = false;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if std::mem::take(&mut value_next) {
            out.push(a.clone());
            prev_dash = false;
            continue;
        }
        let bytes = a.as_bytes();
        if bytes.len() > 2
            && bytes[0] == b'-'
            && bytes[1] != b'-'
            && bytes[1].is_ascii_alphanumeric()
        {
            let mut members = a[1..].chars();
            while let Some(c) = members.next() {
                let one = format!("-{c}");
                if CLUSTER_VALUES.contains(&(under, c)) {
                    out.push(one);
                    let rest = members.as_str();
                    if !rest.is_empty() {
                        out.push(rest.to_string());
                    }
                    break;
                }
                if !c.is_ascii_alphanumeric() || flag(under, &one).is_none() {
                    return Err(one);
                }
                out.push(one);
            }
            prev_dash = false;
            continue;
        }
        // TODO/gate.md T-1853: `--flag=value` of a flag with a row. A
        // boolean takes docker's `=true` and `=false`; a valued flag becomes
        // `--flag value`, the form each parser has an arm for.
        if let Some((name, val)) = a.strip_prefix("--").and_then(|_| a.split_once('='))
            && flag(under, name).is_some()
        {
            // PodUtils/podvm#492: the parsers of `pull`, `run` and `create`
            // read `--tls-verify=VALUE` themselves; split, `false` became
            // the image and TLS verification stayed on.
            if name == "--tls-verify" {
                out.push(a.clone());
                prev_dash = true;
            } else if BOOLEAN_LONGS.contains(&name) {
                match val {
                    "true" => out.push(name.to_string()),
                    "false" => {}
                    // `run --strict=all` (T-1415): the parsers of `run`,
                    // `create` and `exec` take it; each other verb refuses a
                    // value by name (PodUtils/podvm#479).
                    _ if name == "--strict" && STRICT_VALUE.contains(&verb) => out.push(a.clone()),
                    _ => return Err(a.clone()),
                }
                prev_dash = true;
            } else {
                out.push(name.to_string());
                out.push(val.to_string());
                prev_dash = false;
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            value_next = a.starts_with("--")
                && !a.contains('=')
                && !BOOLEAN_LONGS.contains(&a.as_str())
                && flag(under, a).is_some();
            out.push(a.clone());
            prev_dash = true;
            continue;
        }
        out.push(a.clone());
        if prev_dash {
            prev_dash = false;
        } else if bounded {
            // The image: the payload owns everything from here, clusters
            // included.
            out.extend(args[i..].iter().cloned());
            break;
        }
    }
    Ok(out)
}

/// TODO/gate.md T-1853: the long flags of the parity table that take no
/// value. `expand` gives them docker's `--flag=true` and `--flag=false`;
/// each other long flag with a row takes a value, and `--flag=value`
/// becomes `--flag value`. `tests/flag_equals.rs` walks the table.
pub const BOOLEAN_LONGS: &[&str] = &[
    "--all",
    "--all-tags",
    "--cached",
    "--detach",
    "--digests",
    "--follow",
    "--force",
    "--interactive",
    "--json",
    "--no-host-cas",
    "--no-pager",
    "--no-source-fixup",
    "--no-steps",
    "--no-trunc",
    "--password-stdin",
    "--podvm-engine",
    "--podvm-sealed",
    "--quiet",
    "--read-only",
    "--recursive",
    "--rm",
    "--rows",
    "--strict",
    "--tty",
    "--unsafe-host-paths",
    "--verbose",
    "--volumes",
];
