//! TODO/machine.md T-1846: `ps --podvm-tier=incus` and the rows of the
//! instances of podvm in `podvm ps`; TODO/cli.md T-2087 (OR-B40-18): the
//! state of an instance in docker's words.
//!
//! incusd says `Stopped` for an instance that never ran and for one whose
//! command ended. docker says `Created` for the first. incusd 6.0 gives
//! `last_used_at` `1970-01-01T00:00:00Z` to an instance that never started:
//! a start sets it, and a push of a file does not (measured 2026-11-26).

use std::io::Write;

use pod_incus::{Client, Instance};
use podvm_image::error::EXIT_RUNTIME_ERROR;

use super::verbs::{block_on, connect, ours, IMAGE_KEY};

/// Whether the instance `i` is stopped and never started. An empty or
/// another time is not known: the instance is not called `Created`.
pub fn never_started(i: &Instance) -> bool {
    i.status == "Stopped"
        && ["1970-01-01T00:00:00", "0001-01-01T00:00:00"]
            .iter()
            .any(|zero| i.last_used_at.starts_with(zero))
}

/// The status of `i` in the words of incus, with `Created` for an
/// instance that never started.
pub fn status(i: &Instance) -> &str {
    if never_started(i) {
        "Created"
    } else {
        &i.status
    }
}

/// The bound of the answer of incusd for [`hint`]: a hint must not hold a
/// refusal for long.
const HINT_ANSWER: std::time::Duration = std::time::Duration::from_secs(2);

/// TODO/cli.md T-2087 (OR-B40-04): after "no such container" of the
/// other tiers, the line that names an instance of podvm in incus of the
/// name `name`, where an incusd answers at the socket within [`HINT_ANSWER`].
pub fn hint(verb: &str, name: &str) {
    if !super::run::socket_path().exists() {
        return;
    }
    let ours = block_on(verb, async {
        match super::verbs::connect_within(HINT_ANSWER).await {
            Ok((c, _)) => i32::from(matches!(c.instance(name).await, Ok(i) if ours(&i))),
            Err(_) => 0,
        }
    }) == 1;
    if ours {
        eprintln!(
            "podvm {verb}: {name} is an instance of podvm in incus: give --podvm-tier incus \
             (or tier = incus in the configuration of the store) (TODO/cli.md T-2087)"
        );
    }
}

/// TODO/machine.md T-1846: the rows of `podvm ps` for the instances of
/// podvm, in the columns of the containers: each one with `all`, else the
/// running ones. TODO/cli.md T-2108 (OR-B42-08): the newest first, with
/// the age in CREATED and the time of the state in STATUS, as the rows of
/// the containers have them.
pub(crate) async fn rows(c: &Client, all: bool) -> Result<Vec<Vec<String>>, String> {
    let mut list: Vec<&Instance> = Vec::new();
    let got = c.instances().await.map_err(|e| e.to_string())?;
    list.extend(
        got.iter()
            .filter(|i| ours(i) && (all || i.status == "Running")),
    );
    list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(list
        .into_iter()
        .map(|i| {
            let status = if i.status == "Running" {
                match podvm_image::clock::span(&i.last_used_at) {
                    Some(s) if !never_started(i) => format!("Up {s}"),
                    _ => "Up".to_string(),
                }
            } else if never_started(i) {
                "Created".to_string()
            } else {
                format!("Exited ({})", i.status)
            };
            vec![
                i.name.clone(),
                i.config
                    .get(IMAGE_KEY)
                    .cloned()
                    .unwrap_or_else(|| "-".into()),
                "(incus instance)".to_string(),
                podvm_image::clock::since(&i.created_at).unwrap_or_else(|| "-".into()),
                status,
                String::new(),
                i.name.clone(),
            ]
        })
        .collect())
}

/// The rows of `podvm ps` for the instances of podvm where the
/// configuration selects the incus tier and its incusd answers; none else.
pub fn ps_rows(all: bool) -> Vec<Vec<String>> {
    if !matches!(
        super::config::configured(),
        Ok(super::config::Configured::Incus { socket: true })
    ) {
        return Vec::new();
    }
    let out = std::sync::Mutex::new(Vec::new());
    block_on("ps", async {
        match connect().await {
            Ok((c, _)) => match rows(&c, all).await {
                Ok(r) => {
                    *out.lock().unwrap_or_else(|e| e.into_inner()) = r;
                    0
                }
                Err(e) => {
                    eprintln!("podvm ps: incus tier: {e}");
                    1
                }
            },
            Err(e) => {
                eprintln!("podvm ps: incus tier: {e}");
                1
            }
        }
    });
    out.into_inner().unwrap_or_default()
}

pub(super) async fn ps(c: &Client, all: bool) -> i32 {
    let list = match c.instances().await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("podvm ps: incus tier: {e}");
            return EXIT_RUNTIME_ERROR;
        }
    };
    match write_ps(&list, all) {
        Ok(()) => 0,
        Err(e) => crate::names::stdout::failed(&e),
    }
}

/// The table of `ps --podvm-tier=incus` to stdout: a full stdout is an
/// error (T-1816).
fn write_ps(list: &[Instance], all: bool) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    writeln!(out, "{:<24} {:<10} IMAGE", "NAMES", "STATUS")?;
    for i in list.iter().filter(|i| ours(i)) {
        if all || i.status == "Running" {
            let image = i.config.get(IMAGE_KEY).map(String::as_str).unwrap_or("-");
            writeln!(out, "{:<24} {:<10} {image}", i.name, status(i))?;
        }
    }
    out.flush()
}
