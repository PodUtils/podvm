//! The prepared image of a run ([`Prepared`]) and the JSON of `inspect` of a container.

use super::*;

pub(super) fn container_json(s: &podvm_image::Store, c: &Container) -> String {
    let fields = container_fields(s, c);
    let mut o = serde_json::Map::new();
    for (k, v) in &fields {
        // TODO/cli.md T-2001 OP-06: `State` is docker's object, with typed
        // values; `--format '{{.State}}'` stays the word.
        if *k == "State" || k.starts_with("State.") {
            continue;
        }
        // ⚠ The nested pair is nested here too, so a caller reading the
        // document and one reading `--format` do not find two shapes.
        if let Some((group, rest)) = k
            .strip_prefix("Exec.")
            .map(|r| ("Exec", r))
            .or_else(|| k.strip_prefix("End.").map(|r| ("End", r)))
        {
            let e = o
                .entry(group.to_string())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if let Some(m) = e.as_object_mut() {
                m.insert(rest.to_string(), serde_json::Value::String(v.clone()));
            }
            continue;
        }
        o.insert(k.to_string(), serde_json::Value::String(v.clone()));
    }
    o.insert("State".to_string(), state_json(c));
    serde_json::Value::Array(vec![serde_json::Value::Object(o)]).to_string()
}

/// docker's `State` of a container: a null where podvm has no value (no
/// exit code, no start), never a zero for an unknown code.
pub(super) fn state_json(c: &Container) -> serde_json::Value {
    serde_json::json!({
        "Status": c.state.word(),
        "Running": matches!(c.state, State::Running),
        "Dead": matches!(c.state, State::Dead),
        "ExitCode": c.exit_code,
        "Pid": c.pid.unwrap_or(0),
        "StartedAt": c.started_at,
        "FinishedAt": c.finished_at,
    })
}

/// The image `run -d` and `create` need, and everything derived from it.
pub struct Prepared {
    pub record: podvm_image::Record,
    pub image: String,
    pub rootfs: String,
    pub argv: Vec<String>,
    pub env: Vec<String>,
    pub working_dir: String,
    pub name: Option<String>,
    pub rung: String,
    pub detach: bool,
    pub rm: bool,
    /// `-t`: a foreground `run` gives the payload a pty (TODO/cli.md T-1760).
    pub tty: bool,
    /// `-i`: with `-t`, stdin goes to the pty (PodUtils/podvm#545).
    pub interactive: bool,
    /// `--timeout` in milliseconds: the wall bound of the run, or `None`.
    /// TODO/cli.md T-1655.
    pub wall_ms: Option<u64>,
    /// `--ulimit` and `--security-opt no-new-privileges`, for the child.
    pub hardening: podvm_supervise::table::Hardening,
    /// ⭐ T-0710: the host memo file this entry hands the payload, beside the
    /// container record once the container exists. For `prepare` itself this is
    /// an ephemeral file under staging; `create` and `run -d` move it to
    /// `containers/<id>/ownership.memo`, foreground `run` deletes it on exit.
    /// The environment already carries `PODVM_MEMO_FD` for it.
    pub memo_host_path: std::path::PathBuf,
    /// ⭐ TODO/packaging.md T-1003. The forced launch rung `prepare` admitted,
    /// driven by foreground `run` and by nothing else. None is the default
    /// chroot-by-path entry.
    pub ladder: Option<podvm_enter::ladder::Mode>,
    /// The probe rows the entry was decided under. `run` feeds them to the
    /// ladder; `create` and `run -d` carry them unused, because neither
    /// drives a rung.
    pub findings: podvm_probe::Findings,
    /// ⭐ T-0804 rule 3: `inspect` reports the TRUE mode per container, and a
    /// shimmed `/dev/null` is part of that mode. One line per fixup that
    /// changed a byte or failed, carried into the container record.
    pub completion: Vec<String>,
    pub completion_degraded: usize,
    /// ⭐ TODO/enter.md T-1317. The no-chroot entry foreground `run` takes
    /// where chroot is denied. None is the chroot sequence, as before.
    pub userland: Option<Userland>,
    /// `--podvm-confine`, guest paths (TODO/enter.md T-1658).
    pub confine: Option<podvm_enter::confine::Spec>,
}
