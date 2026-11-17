//! TODO/gate.md T-1946: the contract of the CLI, one table-driven walk of
//! each verb of the parity table (`system info --format '{{json .Parity}}'`).
//!
//! The parser, the help and the parity table are three texts that people
//! write by hand; the external QA of `0.2.0-beta.31` found 17 faults where
//! they disagreed, and two panics (`docs/methodology/quality.md`). For each
//! verb that podvm serves:
//!
//! 1. `VERB --help` exits 0;
//! 2. each flag row of the verb is named in that help;
//! 3. an unknown flag, and an argument that is not UTF-8, end with a code of
//!    the published table (0, 1, 125, 126, 127), never a panic, and with a
//!    line on stderr where the code is not 0.
//!
//! A new row, a new verb or a new flag is checked with no change here.

use std::os::unix::ffi::OsStrExt;
use std::process::{Command, Stdio};

/// The exit codes of podvm's own answers (`podvm man`, EXIT STATUS).
const CODES: [i32; 5] = [0, 1, 125, 126, 127];

fn podvm<S: AsRef<std::ffi::OsStr>>(args: &[S], store: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_podvm"))
        .args(args)
        .current_dir(store)
        .env("PODVM_STORE", store.join("store"))
        .env("HOME", store)
        .env("PODVM_SPARE_TREE", "0")
        .env_remove("RUST_BACKTRACE")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

/// Rows that are not verbs: the names that podvm answers to through
/// argv[0] (`docker`, `podman`, T-0803). `podvm docker` is no verb.
const NOT_VERBS: [&str; 2] = ["docker", "podman"];

/// Top-level names that only redirect to `podvm system VERB` (T-1832,
/// PodUtils/podvm#483): `podvm --help` names them under `system`.
const REDIRECTS: [&str; 5] = ["abi", "df", "info", "install-names", "prune"];

/// The verbs that podvm serves, each with the flags of its rows.
fn table(store: &std::path::Path) -> Vec<(String, Vec<String>)> {
    let out = podvm(&["system", "info", "--format", "{{json .Parity}}"], store);
    assert!(out.status.success());
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
    let mut verbs: Vec<(String, Vec<String>)> = Vec::new();
    for r in &rows {
        let verb = r["verb"].as_str().unwrap_or_default().to_string();
        if NOT_VERBS.contains(&verb.as_str()) {
            continue;
        }
        match r["flag"].as_str() {
            // A verb that podvm refuses by name has no help to check.
            None if r["status"] == "None" => continue,
            None => {
                if !verbs.iter().any(|(v, _)| *v == verb) {
                    verbs.push((verb, Vec::new()));
                }
            }
            Some(f) if r["status"] != "None" => match verbs.iter_mut().find(|(v, _)| *v == verb) {
                Some((_, fs)) => fs.push(f.to_string()),
                None => verbs.push((verb, vec![f.to_string()])),
            },
            Some(_) => {}
        }
    }
    verbs
}

/// The spellings of one row (`-f, --force` gives `-f` and `--force`), each
/// with no value.
fn spellings(row: &str) -> Vec<String> {
    row.split(',')
        .map(|s| {
            let s = s.trim();
            let s = s.split(' ').next().unwrap_or(s);
            s.split('=').next().unwrap_or(s).to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

#[test]
fn each_verb_keeps_its_contract() {
    let dir = std::env::temp_dir().join(format!("podvm-t1946-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let verbs = table(&dir);
    assert!(verbs.len() > 30, "only {} verbs", verbs.len());
    let mut bad: Vec<String> = Vec::new();
    // PodUtils/podvm#520: the top help names each one-word verb.
    let top = podvm(&["--help"], &dir);
    let top = String::from_utf8_lossy(&top.stdout).to_string();
    for (verb, _) in &verbs {
        if !verb.contains(' ')
            && !REDIRECTS.contains(&verb.as_str())
            && !top.contains(&format!("  {verb} "))
        {
            bad.push(format!("podvm --help: no line of the verb {verb}"));
        }
    }
    for (verb, rows) in &verbs {
        let argv: Vec<&str> = verb.split(' ').collect();
        let mut help_argv = argv.clone();
        help_argv.push("--help");
        let h = podvm(&help_argv, &dir);
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&h.stdout),
            String::from_utf8_lossy(&h.stderr)
        );
        if h.status.code() != Some(0) {
            bad.push(format!("{verb} --help: exit {:?}", h.status.code()));
        }
        for row in rows {
            let names = spellings(row);
            if names.iter().any(|n| n == "-h" || n == "--help") {
                continue;
            }
            if !names.iter().any(|n| text.contains(n.as_str())) {
                bad.push(format!("{verb} --help: no word of the row {row:?}"));
            }
        }
        let hostile: [&std::ffi::OsStr; 2] = [
            std::ffi::OsStr::new("--podvm-no-such-flag"),
            std::ffi::OsStr::from_bytes(b"\xff"),
        ];
        for h in hostile {
            let mut a: Vec<&std::ffi::OsStr> = argv.iter().map(std::ffi::OsStr::new).collect();
            a.push(h);
            let o = podvm(&a, &dir);
            let code = o.status.code();
            let shown = h.to_string_lossy();
            match code {
                // The panic hook of T-1947 turns a panic into exit 125
                // with this line: the line is the sign of a bug here.
                Some(_) if String::from_utf8_lossy(&o.stderr).contains("internal error") => {
                    bad.push(format!("{verb} {shown}: a panic of podvm"))
                }
                Some(c) if CODES.contains(&c) => {
                    if c != 0 && o.stderr.is_empty() {
                        bad.push(format!("{verb} {shown}: exit {c} with no line on stderr"));
                    }
                }
                _ => bad.push(format!(
                    "{verb} {shown}: exit {code:?} (a panic or a signal)"
                )),
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        bad.is_empty(),
        "{} breaks of the contract:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

#[test]
fn a_row_gives_each_of_its_spellings() {
    assert_eq!(spellings("-f, --force"), ["-f", "--force"]);
    assert_eq!(spellings("--tls-verify=B"), ["--tls-verify"]);
    assert_eq!(spellings("--network, --net"), ["--network", "--net"]);
    assert_eq!(spellings("-e, --env K=V"), ["-e", "--env"]);
}
