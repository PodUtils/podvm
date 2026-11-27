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

/// TODO/cli.md T-2087 (OR-B40-09, OR-B42-04): an argument that is not
/// UTF-8. One rule for the family: podvm never panics on it, and refuses it
/// with exit 125 and one line that names its position and its class; an
/// argument that is UTF-8 (the empty one too) is not refused for it. The
/// payload class says why it cannot pass yet: the plan of pod-enter takes
/// UTF-8 only.
#[test]
fn an_argument_that_is_not_utf8_is_refused_by_its_position_and_class() {
    // The classes: the words of each line.
    const PAYLOAD: &str = "of the payload is not UTF-8: each tier hands";
    const PODVM: &str = "is not UTF-8: podvm takes its flags";
    const VERB: &str = "takes UTF-8 arguments only";
    const HOLDS: &str = "a character that podvm keeps for a byte";
    const PASSES: &str = "-";
    let dir = std::env::temp_dir().join(format!("podvm-t2087-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let i: &[u8] = b"mirror.gcr.io/library/alpine:3.20";
    #[rustfmt::skip]
    let rows: &[(&[&[u8]], &str, &str)] = &[
        // The command and its arguments, and an `-e` value.
        (&[b"run", b"--pull=never", i, b"echo", b"\xff\xfe"], PAYLOAD, r#"argument 5 ("\xff\xfe")"#),
        (&[b"run", i, b"printf", b"caf\xe9"], PAYLOAD, r#"argument 4 ("caf\xe9")"#),
        (&[b"run", b"-e", b"K=caf\xe9", i, b"true"], PAYLOAD, r#"argument 3 ("K=caf\xe9")"#),
        (&[b"run", b"--env=K=\xff", i, b"true"], PAYLOAD, r#"argument 2 ("K=\xff")"#),
        (&[b"create", i, b"\xe2\x28\xa1"], PAYLOAD, r#"argument 3 ("\xe2(\xa1")"#),
        (&[b"exec", b"-e", b"K=\xff", b"ctr", b"true"], PAYLOAD, r#"argument 3 ("K=\xff")"#),
        (&[b"exec", b"ctr", b"sh", b"-c", b"caf\xe9"], PAYLOAD, r#"argument 5 ("caf\xe9")"#),
        // A flag, its value or the image name: podvm's own.
        (&[b"run", b"--name", b"caf\xe9", i, b"true"], PODVM, r#"argument 3 ("caf\xe9")"#),
        (&[b"run", b"-w", b"/\xff", i, b"true"], PODVM, r#"argument 3 ("/\xff")"#),
        (&[b"run", b"--\xff", i], PODVM, r#"argument 2 ("--\xff")"#),
        (&[b"run", b"caf\xe9:1"], PODVM, r#"argument 2 ("caf\xe9:1")"#),
        (&[b"exec", b"--user", b"\xff", b"ctr", b"true"], PODVM, r#"argument 3 ("\xff")"#),
        (&[b"container", b"run", b"--label", b"a=\xff", i], PODVM, r#"argument 4 ("a=\xff")"#),
        (&[b"--store", b"/tmp/\xff", b"images"], PODVM, r#"argument 2 ("/tmp/\xff")"#),
        // A verb with no payload.
        (&[b"rm", b"caf\xe9"], VERB, r#"argument 2 ("caf\xe9") is not UTF-8: `rm`"#),
        (&[b"\xff"], VERB, r#"argument 1 ("\xff") is not UTF-8: `podvm`"#),
        // A UTF-8 argument with a character of the carried range.
        (&[b"run", i, "\u{10FFE9}".as_bytes()], HOLDS, r#"argument 3 ("\u{10ffe9}") holds U+10FFE9"#),
        // UTF-8, and empty: not refused for it.
        (&[b"run", b"--pull=never", b"nosuch.invalid/x:1", b"echo", b""], PASSES, ""),
    ];
    let mut bad: Vec<String> = Vec::new();
    for (args, class, want) in rows {
        let a: Vec<&std::ffi::OsStr> = args
            .iter()
            .map(|b| std::ffi::OsStr::from_bytes(b))
            .collect();
        let o = podvm(&a, &dir);
        let err = String::from_utf8_lossy(&o.stderr).to_string();
        let lines = err
            .lines()
            .filter(|l| l.contains("UTF-8") || l.contains("holds U+"))
            .count();
        let row = format!("{a:?}");
        let ok = if *class == PASSES {
            lines == 0
        } else {
            o.status.code() == Some(125) && lines == 1 && err.contains(want) && err.contains(class)
        };
        if !ok || err.contains("internal error") || err.contains("panicked") {
            bad.push(format!(
                "{row}: exit {:?}, want {class:?} {want:?}: {err}",
                o.status.code()
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
