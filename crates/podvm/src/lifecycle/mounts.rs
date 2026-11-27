//! docker's `.Mounts`, `.HostConfig.Binds` and `.HostConfig.Tmpfs` of a
//! container (TODO/cli.md T-2108, OR-B42-08), from the one list of mounts
//! of its record (`Hardening::mounts`).
//!
//! docker lists `-v` in `.Mounts` and `.HostConfig.Binds`, and `--tmpfs` in
//! `.HostConfig.Tmpfs` only: its `.Mounts` holds a tmpfs of `--mount
//! type=tmpfs`, which podvm does not take. A record of an older podvm has
//! no list: its mounts come from `PODVM_BINDS` and `PODVM_TMPFS` of its env,
//! which hold the mounts of the namespace rung only.

use podvm_supervise::table::{Container, Mount, MountKind};
use serde_json::{json, Map, Value};

/// The mounts of `c`: the list of its record, or the fallback of its env.
pub(super) fn of(c: &Container) -> Vec<Mount> {
    if let Some(list) = &c.hardening.mounts {
        return list.clone();
    }
    let bind = |source: String, destination: String, options: &str, rw| Mount {
        kind: MountKind::Bind,
        source,
        destination,
        options: options.to_string(),
        rw,
    };
    let mut out: Vec<Mount> = podvm_supervise::launcher::binds_of(&c.env)
        .into_iter()
        .map(|b| {
            let ro = if b.read_only { "ro" } else { "" };
            bind(b.host, b.guest, ro, !b.read_only)
        })
        .collect();
    out.extend(
        podvm_supervise::launcher::overlays_of(&c.env)
            .into_iter()
            .map(|v| bind(v.host, v.guest, "overlay", true)),
    );
    // A damaged `PODVM_TMPFS` names no mount here: the launcher refuses
    // to start such a record, and `inspect` still prints the rest.
    let tmpfs = podvm_supervise::launcher::tmpfs::of(&c.env).unwrap_or_default();
    out.extend(tmpfs.into_iter().map(|t| Mount {
        kind: MountKind::Tmpfs,
        source: String::new(),
        destination: t.guest,
        options: format!("size={}", t.bytes),
        rw: true,
    }));
    out
}

/// docker's `.Mounts`: one object for each `-v`, and `[]` where there is
/// none. An overlay volume (T-2101) is a bind of GUEST with the mode
/// `overlay`: the payload writes to GUEST, never to HOST.
pub(super) fn mounts_json(list: &[Mount]) -> Value {
    Value::Array(
        list.iter()
            .filter(|m| m.kind == MountKind::Bind)
            .map(|m| {
                json!({
                    "Type": "bind",
                    "Source": m.source,
                    "Destination": m.destination,
                    "Mode": m.options,
                    "RW": m.rw,
                    "Propagation": "rprivate",
                })
            })
            .collect(),
    )
}

/// docker's `.HostConfig.Binds`: `HOST:GUEST[:OPTIONS]` for each `-v`, and
/// null where there is none.
pub(super) fn binds_json(list: &[Mount]) -> Value {
    let binds: Vec<String> = list
        .iter()
        .filter(|m| m.kind == MountKind::Bind)
        .map(|m| match m.options.as_str() {
            "" => format!("{}:{}", m.source, m.destination),
            o => format!("{}:{}:{o}", m.source, m.destination),
        })
        .collect();
    if binds.is_empty() {
        return Value::Null;
    }
    json!(binds)
}

/// docker's `.HostConfig.Tmpfs`: the options of each `--tmpfs` by its path,
/// and null where there is none.
pub(super) fn tmpfs_json(list: &[Mount]) -> Value {
    let m: Map<String, Value> = list
        .iter()
        .filter(|m| m.kind == MountKind::Tmpfs)
        .map(|m| (m.destination.clone(), Value::String(m.options.clone())))
        .collect();
    if m.is_empty() {
        return Value::Null;
    }
    Value::Object(m)
}

/// docker's `.HostConfig` of the fields that podvm keeps.
pub(super) fn host_config(list: &[Mount]) -> Value {
    json!({ "Binds": binds_json(list), "Tmpfs": tmpfs_json(list) })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TODO/cli.md T-2108: one rule for each record, new or old: the list of
    /// the record wins, and a record with no list reads `PODVM_BINDS` and
    /// `PODVM_TMPFS`; each gives docker's three values, and nothing gives
    /// `[]` for `.Mounts` and null for the two of `HostConfig`.
    #[test]
    fn each_record_gives_dockers_mounts() {
        let m = |kind, source: &str, destination: &str, options: &str, rw| Mount {
            kind,
            source: source.to_string(),
            destination: destination.to_string(),
            options: options.to_string(),
            rw,
        };
        let env = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let bind = |s: &str, d: &str, mode: &str, rw: bool| {
            json!({"Type": "bind", "Source": s, "Destination": d, "Mode": mode, "RW": rw,
                   "Propagation": "rprivate"})
        };
        // (the list of the record, the env, .Mounts, .HostConfig.Binds, .HostConfig.Tmpfs)
        type Row = (Option<Vec<Mount>>, Vec<String>, Value, Value, Value);
        let rows: Vec<Row> = vec![
            (Some(vec![]), env(&[]), json!([]), Value::Null, Value::Null),
            (None, env(&["A=1"]), json!([]), Value::Null, Value::Null),
            (
                Some(vec![
                    m(MountKind::Bind, "/h", "/d", "", true),
                    m(MountKind::Bind, "/r", "/ro", "ro,z", false),
                    m(MountKind::Tmpfs, "", "/w", "size=64m", true),
                    m(MountKind::Other, "x", "/o", "", true),
                ]),
                // The list wins over the env.
                env(&["PODVM_BINDS=/x:/y"]),
                json!([bind("/h", "/d", "", true), bind("/r", "/ro", "ro,z", false)]),
                json!(["/h:/d", "/r:/ro:ro,z"]),
                json!({"/w": "size=64m"}),
            ),
            (
                None,
                env(&[
                    "PODVM_BINDS=/d:/h,/ro:/r:ro,/lay:/low:overlay",
                    "PODVM_TMPFS=/w:size=4096",
                ]),
                json!([
                    bind("/h", "/d", "", true),
                    bind("/r", "/ro", "ro", false),
                    bind("/low", "/lay", "overlay", true),
                ]),
                json!(["/h:/d", "/r:/ro:ro", "/low:/lay:overlay"]),
                json!({"/w": "size=4096"}),
            ),
            // A damaged tmpfs of an old record names no mount.
            (
                None,
                env(&["PODVM_TMPFS=/w:size=lots"]),
                json!([]),
                Value::Null,
                Value::Null,
            ),
        ];
        for (list, env, mounts, binds, tmpfs) in rows {
            let mut c = podvm_supervise::table::Container {
                id: "a".repeat(64),
                name: "n".into(),
                argv: Vec::new(),
                env: env.clone(),
                working_dir: "/".into(),
                created_at: "now".into(),
                started_at: None,
                finished_at: None,
                state: podvm_supervise::table::State::Created,
                pid: None,
                launcher_pid: None,
                exit_code: None,
                noticed: None,
                wall_ms: None,
                end: None,
                rm: false,
                of: Default::default(),
            };
            c.of.hardening.mounts = list;
            let got = of(&c);
            assert_eq!(mounts_json(&got), mounts, "{env:?}");
            assert_eq!(binds_json(&got), binds, "{env:?}");
            assert_eq!(tmpfs_json(&got), tmpfs, "{env:?}");
            assert_eq!(
                host_config(&got),
                json!({"Binds": binds, "Tmpfs": tmpfs}),
                "{env:?}"
            );
        }
    }
}
