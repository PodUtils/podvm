//! `podvm man`: the manual, generated from the binary's own data.
//!
//! [`TODO/cli.md`](../../../TODO/cli.md) T-1332: help text copied into a
//! document drifts from the binary the day a flag is added, so the manual is
//! rendered here instead: the command list and the one-line summaries come
//! from the parity table, and every reference section carries the exact bytes
//! that verb's `--help` prints, captured by re-executing this binary. A new
//! verb appears by construction once it has a table row; a changed usage
//! string appears without any edit here.
//!
//! ⛔ No hand-kept verb list. [`implemented`] is the table's own implemented
//! verbs minus the argv0 aliases, and [`help_argv`] is pinned against
//! [`parity::rows_of`] by unit test, so the two cannot fork.

use std::io::{IsTerminal, Write};

mod model;

use crate::parity::{self, Row, Status};

pub const MAN_USAGE: &str = "\
usage: podvm man [--no-pager | --json | --roff | --llms] [verb]

  man            render the manual from the binary's own usage strings
                 and the parity table
  man <verb>     render only that verb's section
  --no-pager     print to stdout even on a terminal
  --json         the manual as one JSON object (the model of pod-manual)
  --roff         the manual as one man(7) page
  --llms         the whole manual as one llms.txt
";

/// The verbs that get a command-reference section: every implemented table
/// verb except the argv0 aliases, which are names for this binary rather
/// than commands it takes.
fn implemented() -> Vec<&'static Row> {
    parity::TABLE
        .iter()
        .filter(|r| {
            r.flag.is_none()
                && !matches!(r.status, Status::None)
                && !crate::names::ALIASES.contains(&r.verb)
        })
        .collect()
}

/// The argv that prints a verb's usage: bare for a top-level verb, through
/// the group for a subverb whose rows live under its own name (T-0808).
fn help_argv(verb: &str) -> Vec<&str> {
    match verb {
        "prune" => vec!["image", "prune"],
        "install-names" | "abi" | "df" => vec!["system", verb],
        "remote ssh" => vec!["remote", "ssh"],
        "machine ssh" => vec!["machine", "ssh"],
        _ => vec![verb],
    }
}

/// TODO/cli.md T-1833: `podvm prune --help` (and `install-names`, `abi`,
/// `df`) prints the usage of the group verb that serves the name, exit 0.
/// None for each other name or argv.
pub(crate) fn alias_help(verb: &str, rest: &[String]) -> Option<std::process::ExitCode> {
    let help = matches!(rest, [h] if h == "-h" || h == "--help");
    if !help || help_argv(verb) == [verb] || verb.contains(' ') {
        return None;
    }
    print!("{}", help_of(verb)?);
    Some(std::process::ExitCode::SUCCESS)
}

/// Capture what `podvm <argv...> --help` prints on stdout: the same bytes
/// the caller sees, so the manual cannot drift from them. Empty (a refusal,
/// which goes to stderr) is `None`.
fn help_of(verb: &str) -> Option<String> {
    let mut cmd = help_argv(verb);
    cmd.push("--help");
    let exe = std::env::current_exe().ok()?;
    let out = std::process::Command::new(exe).args(&cmd).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

/// The command-list summary: the note's first sentence. The notes are
/// written as sentences, so this is a read rather than a parse; the unit
/// test below holds it on fixed strings.
fn summary(note: &str) -> &str {
    match note.find(". ") {
        Some(i) => note[..=i].trim(),
        None => note.trim().trim_end_matches('.'),
    }
}

fn render_section(out: &mut String, row: &Row, help: &dyn Fn(&str) -> Option<String>) {
    // TODO/cli.md T-1832: the head names the argv that serves the verb
    // (`podvm image prune`), not a top-level name that refuses.
    let head = help_argv(row.verb).join(" ");
    out.push_str(&format!("=== podvm {head} [{}]\n", row.status.word()));
    out.push_str(row.note.trim());
    out.push('\n');
    match help(row.verb) {
        Some(text) => {
            if !text.ends_with('\n') {
                out.push_str(&text);
                out.push('\n');
            } else {
                out.push_str(&text);
            }
        }
        None => {
            out.push_str(&format!(
                "No usage to show: `podvm {}` is not a command this binary takes.\n",
                row.verb
            ));
        }
    }
    out.push('\n');
}

/// The whole manual, or one verb's section under the same header.
fn render(only: Option<&str>, help: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let verbs = implemented();
    if let Some(want) = only {
        let row = parity::TABLE
            .iter()
            .find(|r| r.flag.is_none() && r.verb == want)?;
        let mut out = String::new();
        header(&mut out);
        render_section(&mut out, row, &help);
        return Some(out);
    }
    let mut out = String::new();
    header(&mut out);
    out.push_str("COMMANDS\n");
    // PodUtils/podvm#483: a name that only redirects lists as the verb that
    // serves it (`system df`, `image prune`), as the reference below does.
    let width = verbs
        .iter()
        .map(|r| help_argv(r.verb).join(" ").len())
        .max()
        .unwrap_or(0);
    for row in &verbs {
        out.push_str(&format!(
            "  {:<width$} {} [{}]\n",
            help_argv(row.verb).join(" "),
            summary(row.note),
            row.status.word(),
            width = width
        ));
    }
    out.push('\n');
    global(&mut out);
    out.push_str("COMMAND REFERENCE\n");
    for row in &verbs {
        render_section(&mut out, row, &help);
    }
    Some(out)
}

fn header(out: &mut String) {
    out.push_str(&format!("podvm {} manual\n\n", env!("CARGO_PKG_VERSION")));
}

fn global(out: &mut String) {
    out.push_str(&format!(
        "GLOBAL\n\
         Every verb prints its own usage with -h or --help and exits 0.\n\
         Installed as docker or podman, this binary takes the same \
         verbs under either of those names.\n\
         A refusal from a verb's flag parser exits {}; a refusal after the \
         flags parsed exits {}; a payload's exit code is its own, 128+N \
         for a signal. The full table is data: podvm system info \
         --format '{{{{json .ExitCodes}}}}'.\n\
         podvm never prompts: a missing input is a refusal naming what is \
         missing.\n\
         Answers go to stdout; progress and diagnostics go to stderr.\n\n",
        podvm_probe::exit::EXIT_FLAG_ERROR,
        podvm_probe::exit::EXIT_CLI_ERROR,
    ));
}

fn run(args: &[String], help: &dyn Fn(&str) -> Option<String>) -> i32 {
    // ⭐ TODO/cli.md T-1330: bundled shorts expand before admission.
    let args: Vec<String> = match crate::parity::expand("man", args) {
        Ok(a) => a,
        Err(member) => {
            return crate::parity::refuse_member("man", &member, MAN_USAGE);
        }
    };
    let mut no_pager = false;
    let (mut json, mut roff, mut llms) = (false, false, false);
    let mut want: Option<&str> = None;
    for a in &args {
        match a.as_str() {
            "--no-pager" => no_pager = true,
            "--json" => json = true,
            "--roff" => roff = true,
            "--llms" => llms = true,
            "-h" | "--help" => {
                print!("{MAN_USAGE}");
                return 0;
            }
            s if s.starts_with('-') => {
                if let Err(c) = crate::parity::admit("man", s, MAN_USAGE) {
                    return c;
                }
                return crate::parity::no_arm("man", s);
            }
            s => {
                if want.is_some() {
                    eprint!("{MAN_USAGE}");
                    return podvm_image::error::EXIT_FLAG_ERROR;
                }
                want = Some(s);
            }
        }
    }
    if let Some(name) = want
        && parity::TABLE
            .iter()
            .all(|r| r.flag.is_some() || r.verb != name)
    {
        let mut err = std::io::stderr().lock();
        let _ = writeln!(
            err,
            "podvm man: {name}: podvm has no such command, and neither does \
                 the parity table (TOOL.md section 6.8)"
        );
        return podvm_image::error::EXIT_CLI_ERROR;
    }
    // PodUtils/podvm#478: one form of the manual at a time, and `--llms` is
    // the whole manual, with no page.
    if llms && (json || roff || want.is_some()) {
        eprintln!(
            "podvm man: --llms is the whole manual, with no other form and no page; \
             fix: give one of --json, --roff, --llms, and a page with --json or --roff"
        );
        return podvm_image::error::EXIT_CLI_ERROR;
    }
    // TODO/cli.md T-1917: the whole manual as llms.txt, with no other page.
    if llms {
        print!("{}", pod_manual::llms::full(&model::manual(help), &[]));
        return 0;
    }
    // TODO/cli.md T-1917: the writers of pod-manual over the same sources.
    if json || roff {
        let m = model::manual(help);
        let section = want.map(|w| help_argv(w).join(" "));
        let req = pod_manual::Request {
            section,
            no_pager: true,
            roff,
            json,
        };
        return match pod_manual::body(&m, &req) {
            Ok(t) => {
                print!("{t}");
                0
            }
            Err(e) => {
                eprintln!("podvm man: {e}");
                podvm_image::error::EXIT_CLI_ERROR
            }
        };
    }
    let text = match render(want, &help) {
        Some(t) => t,
        None => return podvm_image::error::EXIT_CLI_ERROR,
    };
    // ⭐ The pager is a display choice, never the answer: `--no-pager` and a
    // non-terminal stdout print the same bytes, and a pager that cannot
    // start falls back to stdout rather than failing the manual.
    let pager: Option<String> = if no_pager || !std::io::stdout().is_terminal() {
        None
    } else {
        Some(
            std::env::var("PAGER")
                .ok()
                .filter(|p| !p.trim().is_empty())
                .unwrap_or_else(|| "less".to_string()),
        )
    };
    match pager {
        None => {
            print!("{text}");
            0
        }
        Some(p) => {
            let mut child = match std::process::Command::new(&p)
                .stdin(std::process::Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("podvm man: {p} could not start ({e}), printing without a pager");
                    print!("{text}");
                    return 0;
                }
            };
            let wrote = match child.stdin.take() {
                Some(mut stdin) => stdin.write_all(text.as_bytes()).is_ok(),
                None => false,
            };
            if !wrote {
                eprintln!("podvm man: {p} took no input, printing without a pager");
                print!("{text}");
                return 0;
            }
            let _ = child.wait();
            0
        }
    }
}

pub fn man(args: &[String]) -> i32 {
    run(args, &help_of)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stub(verb: &str) -> Option<String> {
        Some(format!("usage: podvm {verb} [options]\n"))
    }

    #[test]
    fn every_implemented_verb_renders_a_section() {
        let text = render(None, &stub).expect("the full manual renders");
        let verbs = implemented();
        assert!(!verbs.is_empty(), "the manual lists no verbs");
        for row in &verbs {
            assert!(
                text.contains(&format!(
                    "=== podvm {} [{}]",
                    help_argv(row.verb).join(" "),
                    row.status.word()
                )),
                "no section for {}",
                row.verb
            );
            assert!(
                text.contains(&format!("usage: podvm {} [options]", row.verb)),
                "no help bytes for {}",
                row.verb
            );
        }
        let sections = text.matches("=== podvm ").count();
        assert_eq!(sections, verbs.len(), "{text}");
    }

    #[test]
    fn aliases_render_no_reference_section() {
        let text = render(None, &stub).expect("the full manual renders");
        for alias in crate::names::ALIASES {
            assert!(
                !text.contains(&format!("=== podvm {alias} ")),
                "{alias} is a name for the binary, not a command section"
            );
        }
    }

    #[test]
    fn no_unimplemented_verb_gets_a_reference_section() {
        let verbs = implemented();
        assert!(
            verbs.iter().all(|r| !matches!(r.status, Status::None)),
            "a refused verb would render as a command"
        );
        assert!(verbs.iter().all(|r| r.flag.is_none()));
    }

    #[test]
    fn one_verb_renders_only_its_section() {
        let text = render(Some("ps"), &stub).expect("ps renders");
        assert!(text.contains("=== podvm ps "), "{text}");
        assert!(!text.contains("=== podvm run "), "{text}");
        assert!(render(Some("frobnicate"), &stub).is_none());
    }

    #[test]
    fn group_help_paths_match_rows_of() {
        for (single, multi) in [
            ("prune", "image prune"),
            ("install-names", "system install-names"),
            ("abi", "system abi"),
            ("df", "system df"),
        ] {
            assert_eq!(parity::rows_of(multi), single, "{multi}");
            assert_eq!(help_argv(single).join(" "), multi, "{single}");
        }
        // Space-verbs with their own rows: the manual reaches them through
        // the group, so a section renders instead of "not a command".
        for (multi, argv) in [("remote ssh", "remote ssh"), ("machine ssh", "machine ssh")] {
            assert_eq!(help_argv(multi).join(" "), argv, "{multi}");
        }
    }

    #[test]
    fn summary_is_the_first_sentence() {
        assert_eq!(
            summary("enters a chroot, never a namespace. The rest."),
            "enters a chroot, never a namespace."
        );
        assert_eq!(summary("one artefact, no split"), "one artefact, no split");
    }

    #[test]
    fn the_manual_names_its_sources() {
        let text = render(None, &stub).expect("the full manual renders");
        assert!(text.starts_with(&format!("podvm {} manual", env!("CARGO_PKG_VERSION"))));
        assert!(text.contains("GLOBAL"));
        assert!(text.contains("COMMAND REFERENCE"));
    }

    #[test]
    fn the_whole_manual_is_plain_ascii() {
        // TODO/cli.md T-1336: usage strings, parity notes and the manual
        // carry no marker glyphs on any path; a binary whose manual needs
        // a codepoint above U+007F forces every downstream parser to
        // handle bytes that carry no meaning.
        let text = render(None, &stub).expect("the full manual renders");
        let bad = text.bytes().filter(|b| *b > 0x7F).count();
        assert_eq!(bad, 0, "the manual carries {bad} non-ASCII bytes");
    }

    /// TODO/cli.md T-1832: the six texts say what the binary does.
    #[test]
    fn doc_text_matches_the_binary() {
        let top = crate::USAGE;
        assert!(top.contains("login") && top.contains("logout"), "{top}");
        assert!(crate::exec::EXEC_USAGE.contains("T-1750"));
        assert!(crate::images::PULL_USAGE.contains("daemon option of the same name"));
        assert!(crate::images::EXTRACT_USAGE.contains("platform of\n    this host"));
        let notes: Vec<&str> = crate::parity::TABLE
            .iter()
            .filter(|r| r.flag == Some("--insecure-registry"))
            .map(|r| r.note)
            .collect();
        assert!(
            notes.iter().all(|n| n.contains("daemon option")),
            "{notes:?}"
        );
        let limits = include_str!("../../../docs/limits.md");
        assert!(!limits.contains("A dynamic program and a detached"));
        assert_eq!(help_argv("prune").join(" "), "image prune");
    }

    /// TODO/cli.md T-1917: `--llms` writes the whole model as llms.txt.
    #[test]
    fn llms_writes_the_whole_model() {
        let m = model::manual(&stub);
        let t = pod_manual::llms::full(&m, &[]);
        assert!(t.contains("podvm"), "{t}");
        assert!(t.contains("run"), "{t}");
        assert!(MAN_USAGE.contains("--llms"));
        assert!(crate::parity::TABLE
            .iter()
            .any(|r| r.verb == "man" && r.flag == Some("--llms")));
    }
}
