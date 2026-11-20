//! TODO/cli.md T-1917: the manual of podvm as the model of `pod-manual`
//! (podcrates@T-700, `v0.54.0`), filled from podvm's own sources and from
//! no second text: the parity table gives the commands, their flags and
//! their status against docker; each verb's `--help` gives its usage;
//! `names::ALIASES` gives the names of the program by argv0.

use pod_manual::{Command, Flag, FlagKind, Manual, Parity, Status};

use crate::parity::{self, Row};

/// The status of a row as `pod-manual` says it.
pub fn status(s: parity::Status) -> Status {
    match s {
        parity::Status::Native => Status::Implemented,
        parity::Status::Degraded => Status::Degraded,
        parity::Status::Stub => Status::Stub,
        parity::Status::None => Status::Refused,
    }
}

fn parity_of(row: &Row) -> Parity {
    Parity {
        tool: "docker".to_string(),
        status: status(row.status),
        text: row.note.to_string(),
    }
}

/// The flags of one row: `-a, --all` is one flag with two spellings; a row
/// that names several long flags (`--stack, --nat`) gives one flag each.
pub fn flags_of(row: &Row) -> Vec<Flag> {
    let Some(spec) = row.flag else {
        return Vec::new();
    };
    let words: Vec<&str> = spec.split(", ").map(str::trim).collect();
    let short = words
        .iter()
        .find_map(|w| {
            w.strip_prefix('-')
                .filter(|s| !s.starts_with('-') && s.chars().count() == 1)
        })
        .and_then(|s| s.chars().next());
    let longs: Vec<&str> = words.iter().filter_map(|w| w.strip_prefix("--")).collect();
    let kind = if row.status == parity::Status::None {
        FlagKind::Refused {
            instead: row.note.to_string(),
        }
    } else {
        FlagKind::Wired
    };
    let one = |long: Option<&str>, short: Option<char>| Flag {
        short,
        long: long.map(str::to_string),
        help: super::summary(row.note).to_string(),
        kind: kind.clone(),
        parity: Some(parity_of(row)),
        ..Flag::default()
    };
    match longs.as_slice() {
        [] => short.map(|c| vec![one(None, Some(c))]).unwrap_or_default(),
        [l] => vec![one(Some(l), short)],
        many => many.iter().map(|l| one(Some(l), None)).collect(),
    }
}

/// The note after its first sentence (the summary), where it has more.
fn rest_of(note: &str) -> Option<String> {
    note.find(". ")
        .map(|i| note[i + 2..].trim().to_string())
        .filter(|r| !r.is_empty())
}

/// The model: each verb that the table implements, with its flags, its
/// The arguments of the first usage line of `help` (and of each line that
/// goes on with `podvm` under it), with no `usage: podvm PATH`, and the
/// rest of the help, where it has any.
pub fn split_usage(help: &str, path: &[String]) -> (String, Option<String>) {
    let head = format!("podvm {}", path.join(" "));
    let mut usage = Vec::new();
    let mut lines = help.lines().peekable();
    while let Some(l) = lines.peek() {
        let t = l.trim_start();
        let t = t.strip_prefix("usage:").map_or(t, str::trim_start);
        if !(usage.is_empty() && l.trim_start().starts_with("usage:")) && !t.starts_with("podvm ") {
            break;
        }
        let args = t.strip_prefix(head.as_str()).map_or(t, str::trim_start);
        usage.push(args.to_string());
        lines.next();
    }
    if usage.is_empty() {
        return (help.trim_end().to_string(), None);
    }
    let rest: Vec<&str> = lines.collect();
    let body = rest.join("\n").trim().to_string();
    (usage.join(" | "), (!body.is_empty()).then_some(body))
}

/// status and the bytes of its `--help` as its usage.
pub fn manual(help: &dyn Fn(&str) -> Option<String>) -> Manual {
    let mut commands = Vec::new();
    for row in super::implemented() {
        let path: Vec<String> = super::help_argv(row.verb)
            .iter()
            .flat_map(|s| s.split_whitespace())
            .map(str::to_string)
            .collect();
        let flags = parity::TABLE
            .iter()
            .filter(|r| r.verb == row.verb && r.flag.is_some())
            .flat_map(flags_of)
            .collect();
        // PodUtils/podvm#518: pod-manual writes "Usage: podvm PATH" before
        // the usage, so the field takes the arguments only, and the rest of
        // the help becomes a note.
        let (usage, body) = split_usage(&help(row.verb).unwrap_or_default(), &path);
        let mut notes: Vec<String> = rest_of(row.note).into_iter().collect();
        notes.extend(body);
        commands.push(Command {
            path,
            about: super::summary(row.note).to_string(),
            notes,
            usage,
            flags,
            parity: Some(parity_of(row)),
            ..Command::default()
        });
    }
    // TODO/cli.md T-1917: `SOURCE_DATE_EPOCH` of a reproducible build gives
    // the date of the page; with none, today.
    let now = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        });
    Manual {
        name: "podvm".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        date: pod_manual::model::date_of(now),
        about: "docker and podman commands on hosts that deny namespaces, mounts or chroot"
            .to_string(),
        names: crate::names::ALIASES
            .iter()
            .map(|s| s.to_string())
            .collect(),
        commands,
        facts: crate::environ::facts(),
        ..Manual::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_of_two_spellings_is_one_flag_and_a_row_of_longs_is_one_each() {
        let r = Row {
            verb: "x",
            flag: Some("-a, --all"),
            status: parity::Status::Stub,
            note: "Stub. More.",
        };
        let f = flags_of(&r);
        assert_eq!(f.len(), 1);
        assert_eq!((f[0].short, f[0].long.as_deref()), (Some('a'), Some("all")));
        assert_eq!(f[0].parity.as_ref().map(|p| p.status), Some(Status::Stub));
        let r = Row {
            verb: "x",
            flag: Some("--stack, --nat"),
            status: parity::Status::Native,
            note: "n",
        };
        assert_eq!(flags_of(&r).len(), 2);
        let r = Row {
            verb: "x",
            flag: Some("--no"),
            status: parity::Status::None,
            note: "use --yes",
        };
        assert!(matches!(flags_of(&r)[0].kind, FlagKind::Refused { .. }));
    }

    #[test]
    fn the_model_passes_the_structural_check_of_pod_manual() {
        let m = manual(&|v: &str| Some(format!("usage: podvm {v}\n")));
        pod_manual::check::assert_none(pod_manual::check::model(&m));
    }

    #[test]
    fn each_implemented_verb_is_a_command_of_the_model() {
        let m = manual(&|v: &str| Some(format!("usage: podvm {v} ARG\n\nbody\n")));
        assert_eq!(m.commands.len(), super::super::implemented().len());
        // PodUtils/podvm#518: pod-manual writes "Usage: podvm PATH" itself.
        assert!(m
            .commands
            .iter()
            .all(|c| !c.usage.starts_with("usage:") && c.usage.ends_with("ARG")));
        assert!(m
            .commands
            .iter()
            .all(|c| c.notes.iter().any(|n| n == "body")));
        assert!(m.names.contains(&"docker".to_string()));
    }

    #[test]
    fn a_usage_keeps_its_arguments_and_gives_the_rest_as_a_note() {
        let p = vec!["lab".to_string()];
        let (u, b) = split_usage(
            "usage: podvm lab profiles\n       podvm lab run P\n\nText.\n",
            &p,
        );
        assert_eq!(u, "profiles | run P");
        assert_eq!(b.as_deref(), Some("Text."));
        assert_eq!(
            split_usage("no usage line", &p),
            ("no usage line".to_string(), None)
        );
    }
}
