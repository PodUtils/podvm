//! The types of the fields of `inspect` of a container (TODO/cli.md T-2060).
//!
//! `container_fields` gives each field as text. docker gives numbers, a
//! boolean, lists and maps, and a caller of `{{json .Config.Env}}` or
//! `{{.Config.Cmd}}` reads them as such. This is the one place that types
//! them: plain `inspect` prints [`document`], and `--format` renders the
//! same [`fields`], so the two cannot differ.

use super::*;
use serde_json::{json, Value};

pub const CONTAINER_INSPECT_FIELDS: &[&str] = &[
    "Id",
    "Name",
    "Image",
    "State",
    "State.Status",
    "State.Running",
    "State.ExitCode",
    "State.Pid",
    "State.StartedAt",
    "State.FinishedAt",
    "Status",
    "Pid",
    "LauncherPid",
    "ExitCode",
    "Created",
    "StartedAt",
    "FinishedAt",
    "RootfsPath",
    "LogPath",
    "Rung",
    "Tier",
    "Labels",
    "Complete.Fixups",
    "Complete.Degraded",
    "Command",
    "Config",
    "Config.Env",
    "Config.Cmd",
    "Config.Image",
    "Config.WorkingDir",
    "Config.Labels",
    "Exec.Mode",
    "Exec.Shares",
    "Contains",
    "Noticed",
    "End.Reason",
    "End.RunMs",
    "End.UserMs",
    "End.SystemMs",
    "End.MaxRssKiB",
    "End.Residual",
    "Interpose.Emulated.mknod",
    "Interpose.Emulated.mount",
    "Interpose.Emulated.unshare",
    "Interpose.Emulated.clone",
    "Interpose.Emulated.procfs",
];

/// The fields that are whole numbers, and null where the record has none.
const NUMBERS: &[&str] = &[
    "Pid",
    "LauncherPid",
    "ExitCode",
    "State.Pid",
    "State.ExitCode",
    "Complete.Degraded",
    "End.RunMs",
    "End.UserMs",
    "End.SystemMs",
    "End.MaxRssKiB",
    "Interpose.Emulated.mknod",
    "Interpose.Emulated.mount",
    "Interpose.Emulated.unshare",
    "Interpose.Emulated.clone",
    "Interpose.Emulated.procfs",
];

/// The fields that are a time, and null where the record has none.
const TIMES: &[&str] = &[
    "StartedAt",
    "FinishedAt",
    "State.StartedAt",
    "State.FinishedAt",
];

/// The labels of a run as docker's map: `KEY=VALUE` split at the first `=`.
fn labels(c: &Container) -> Value {
    let mut m = serde_json::Map::new();
    for l in &c.hardening.labels {
        let (k, v) = l.split_once('=').unwrap_or((l.as_str(), ""));
        m.insert(k.to_string(), Value::String(v.to_string()));
    }
    Value::Object(m)
}

/// The variables that the caller can see: the image's and the `-e` of the
/// run. The `PODVM_*` of the entry and its `LD_PRELOAD` are podvm's own, and
/// `exec` does not inherit them either (`exec::enter`).
fn visible_env(c: &Container) -> Vec<&String> {
    let own = |e: &&String| e.starts_with("PODVM_") || e.starts_with("LD_PRELOAD=");
    c.env.iter().filter(|e| !own(e)).collect()
}

/// docker's `Config` of a container.
fn config(c: &Container) -> Value {
    json!({
        "Env": visible_env(c),
        "Cmd": c.argv,
        "Image": c.image,
        "WorkingDir": c.working_dir,
        "Labels": labels(c),
    })
}

/// The fields of `c` with their types. A dash of the text form is a null.
pub(super) fn fields(text: &[(&'static str, String)], c: &Container) -> Vec<(&'static str, Value)> {
    text.iter()
        .map(|(k, v)| {
            let value = match *k {
                "State" => super::prepared::state_json(c),
                "State.Running" => Value::Bool(matches!(c.state, State::Running)),
                "Labels" | "Config.Labels" => labels(c),
                "Config" => config(c),
                "Config.Env" => json!(visible_env(c)),
                "Config.Cmd" => json!(c.argv),
                k if NUMBERS.contains(&k) => v.parse::<i64>().map_or(Value::Null, Value::from),
                k if TIMES.contains(&k) && v == "-" => Value::Null,
                _ => Value::String(v.clone()),
            };
            (*k, value)
        })
        .collect()
}

/// The text of `{{.X}}` where it is not the Go text of the value: `.State`
/// is the word, as docker's `ps` shows it.
pub(super) fn plain(c: &Container) -> Vec<(&'static str, String)> {
    vec![("State", c.state.word().to_string())]
}

/// The document of `inspect`: one object. A name of two parts is a key of an
/// object of the first (`Exec`, `End`), and `State` and `Config` are whole.
pub(super) fn document(fields: &[(&'static str, Value)]) -> Value {
    let mut o = serde_json::Map::new();
    for (k, v) in fields {
        if k.starts_with("State.") || k.starts_with("Config.") {
            continue;
        }
        if let Some((group, rest)) = k
            .strip_prefix("Exec.")
            .map(|r| ("Exec", r))
            .or_else(|| k.strip_prefix("End.").map(|r| ("End", r)))
        {
            let e = o
                .entry(group.to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if let Some(m) = e.as_object_mut() {
                m.insert(rest.to_string(), v.clone());
            }
            continue;
        }
        o.insert(k.to_string(), v.clone());
    }
    Value::Object(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TODO/cli.md T-2060 (OQ-06): each field of `inspect` of a container
    /// has docker's JSON type, in the document and in `--format`, which read
    /// the one list. One row for each field, one rule.
    #[test]
    fn the_fields_of_a_container_have_dockers_types() {
        let d = std::env::temp_dir().join(format!("podvm-typed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let s = podvm_image::Store::open(&d).unwrap();
        let mut c = Container {
            id: "a".repeat(64),
            name: "n".into(),
            argv: vec!["sleep".into(), "9".into()],
            env: vec![
                "A=1".into(),
                "PODVM_MEMO_FD=17".into(),
                "LD_PRELOAD=x".into(),
            ],
            working_dir: "/w".into(),
            created_at: "now".into(),
            started_at: Some("t0".into()),
            finished_at: None,
            state: State::Running,
            pid: Some(77),
            launcher_pid: Some(70),
            exit_code: None,
            noticed: None,
            wall_ms: None,
            end: None,
            rm: false,
            of: podvm_supervise::table::Detail {
                image: "img".into(),
                rung: "chroot".into(),
                ..Default::default()
            },
        };
        c.of.hardening.labels = vec!["team=ci".into(), "k=a=b".into()];
        let text = container_fields(&s, &c);
        let typed = fields(&text, &c);
        let doc = document(&typed);
        let labels = json!({"team": "ci", "k": "a=b"});
        // (field, docker's type as JSON, the text of `{{.X}}`)
        let table: Vec<(&str, Value, &str)> = vec![
            ("Pid", json!(77), "77"),
            ("State.Pid", json!(77), "77"),
            ("LauncherPid", json!(70), "70"),
            ("ExitCode", Value::Null, "-"),
            ("State.ExitCode", Value::Null, "-"),
            ("State.Running", json!(true), "true"),
            ("State.StartedAt", json!("t0"), "t0"),
            ("State.FinishedAt", Value::Null, "-"),
            ("Labels", labels.clone(), "map[k:a=b team:ci]"),
            ("Config.Labels", labels, "map[k:a=b team:ci]"),
            ("Config.Env", json!(["A=1"]), "[A=1]"),
            ("Config.Cmd", json!(["sleep", "9"]), "[sleep 9]"),
            ("Config.Image", json!("img"), "img"),
            ("Config.WorkingDir", json!("/w"), "/w"),
            ("Complete.Degraded", json!(0), "0"),
            ("End.RunMs", Value::Null, "-"),
            ("Name", json!("n"), "n"),
            ("State", json!("running"), "running"),
        ];
        let plain = plain(&c);
        for (field, json, text) in &table {
            let at = typed.iter().find(|(k, _)| k == field).unwrap().1.clone();
            // `State` is docker's object in the JSON and the word in the text.
            if *field == "State" {
                assert_eq!(at["Status"], "running");
                assert_eq!(at["Running"], true);
            } else {
                assert_eq!(&at, json, "{field}");
            }
            let t = |x: &str| crate::format::render_typed(x, &typed, &plain).unwrap();
            assert_eq!(t(&format!("{{{{.{field}}}}}")), *text, "{field}");
            if *field != "State" {
                assert_eq!(
                    t(&format!("{{{{json .{field}}}}}")),
                    json.to_string(),
                    "{field}"
                );
            }
        }
        // The document nests as docker does and holds the same values.
        assert_eq!(doc["Pid"], 77);
        assert_eq!(doc["Labels"]["team"], "ci");
        assert_eq!(doc["Config"]["Cmd"], json!(["sleep", "9"]));
        assert_eq!(doc["State"]["Running"], true);
        assert!(doc["State"]["ExitCode"].is_null());
        assert!(doc.get("State.Pid").is_none() && doc.get("Config.Env").is_none());
        let _ = std::fs::remove_dir_all(&d);
    }
}
