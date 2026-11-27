//! TODO/gate.md T-1853: each long flag of the parity table takes the form
//! `--flag=value`: a valued flag as `--flag value`, a boolean as docker's
//! `--flag=true`. The rows come from the binary itself (`system info
//! --format '{{json .Parity}}'`), so a new row is checked with no change
//! here. A row whose parser has no arm for the form fails with its name.

use std::process::{Command, Stdio};

fn podvm(args: &[&str], store: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_podvm"))
        .args(args)
        // `--report=1` and `--cidfile=1` write in the working directory.
        .current_dir(store)
        .env("PODVM_STORE", store)
        .env("HOME", store)
        .env("PODVM_SPARE_TREE", "0")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// Mirrors `parity::BOOLEAN_LONGS`; a boolean gets `=true`.
const BOOLEANS: &[&str] = &[
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
    "--podvm-vm",
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

#[test]
fn each_long_flag_takes_the_equals_form() {
    let store = std::env::temp_dir().join(format!("podvm-t1853-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    std::fs::create_dir_all(&store).unwrap();
    let out = podvm(&["system", "info", "--format", "{{json .Parity}}"], &store);
    assert!(out.status.success());
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    let mut cases: Vec<(Vec<String>, String)> = Vec::new();
    for r in &rows {
        let (Some(verb), Some(flag)) = (r["verb"].as_str(), r["flag"].as_str()) else {
            continue;
        };
        if r["status"] == "None" {
            continue;
        }
        for long in flag.split(',').map(str::trim) {
            if !long.starts_with("--") || long == "--help" {
                continue;
            }
            let value = if BOOLEANS.contains(&long) {
                "true"
            } else {
                "1"
            };
            let arg = format!("{long}={value}");
            let mut argv: Vec<String> = verb.split(' ').map(str::to_string).collect();
            argv.extend([arg.clone(), "localhost:1/t1853:1".into(), "true".into()]);
            cases.push((argv, arg));
        }
    }
    let checked = cases.len();
    // Eight threads, so the walk of the whole table stays short.
    let missing: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = cases
            .chunks(checked.div_ceil(8).max(1))
            .map(|chunk| {
                let store = &store;
                s.spawn(move || {
                    let mut bad = Vec::new();
                    for (argv, arg) in chunk {
                        let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
                        let o = podvm(&refs, store);
                        let err = String::from_utf8_lossy(&o.stderr);
                        // A verb with no positional refuses the filler image
                        // by name; only a refusal of the flag itself counts.
                        if err.contains("has no arm")
                            || err.contains(&format!("unknown option \"{arg}"))
                        {
                            let first = err.lines().next().unwrap_or("").to_string();
                            bad.push(format!("{}: {first}", refs.join(" ")));
                        }
                    }
                    bad
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    });
    let _ = std::fs::remove_dir_all(&store);
    assert!(checked > 100, "only {checked} long flags");
    assert!(
        missing.is_empty(),
        "{} of {checked}:\n{}",
        missing.len(),
        missing.join("\n")
    );
}

/// A boolean refuses a value that is not `true` or `false`, naming the
/// flag; `=false` turns it off.
#[test]
fn a_boolean_takes_true_and_false_only() {
    let store = std::env::temp_dir().join(format!("podvm-t1853b-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store);
    std::fs::create_dir_all(&store).unwrap();
    let o = podvm(&["ps", "--all=1"], &store);
    let err = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(125), "{err}");
    assert!(err.contains("--all takes no value"), "{err}");
    let o = podvm(&["ps", "--all=false"], &store);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let _ = std::fs::remove_dir_all(&store);
}
