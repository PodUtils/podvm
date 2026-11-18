//! TODO/enter.md T-1920: the volumes of a detached run (`run -d`,
//! `create`) as binds of pod-enter on the namespace rung (podcrates
//! `v0.59.0`, `Plan::volumes`), not a path rewrite of the loader family.
//!
//! The record carries them in `PODVM_BINDS` (`GUEST:HOST[:ro]`, comma
//! separated); the launcher reads them into `Plan::volumes` and drops the
//! variable from the env of the payload.

use std::sync::atomic::{AtomicBool, Ordering};

/// The variable of the record.
pub const VAR: &str = "PODVM_BINDS";
/// The base of `--podvm-volume-base` (PodUtils/podrunner#277).
pub const BASE_VAR: &str = "PODVM_BINDS_BASE";

static PLANNED: AtomicBool = AtomicBool::new(false);

/// Does pod-enter bind the volumes of this run? A detached run or `create`
/// of a caller outside podvm's own user namespace, where the namespace rung
/// is reachable. A foreground run in that namespace binds them itself
/// (`userns::bind_volumes`).
pub(super) fn served(
    verb: &str,
    detach: bool,
    volumes: &[podvm_enter::userland::Volume],
    findings: &podvm_probe::Findings,
) -> bool {
    // A comma or a colon in a path cannot ride in `PODVM_BINDS`: such a
    // volume keeps the loader family.
    let fits = volumes
        .iter()
        .all(|v| !v.host.contains([',', ':']) && !v.guest.contains([',', ':']));
    let yes = !volumes.is_empty()
        && fits
        && (detach || verb == "create")
        && !crate::userns::active()
        && crate::ladder::ns_reachable(findings);
    PLANNED.store(yes, Ordering::Relaxed);
    yes
}

/// Is each volume of this run a bind: podvm's own in its user namespace
/// (`userns::bind_volumes`), or pod-enter's ([`served`])?
pub(super) fn bound(
    verb: &str,
    detach: bool,
    volumes: &[podvm_enter::userland::Volume],
    findings: &podvm_probe::Findings,
) -> bool {
    crate::userns::active() || served(verb, detach, volumes, findings)
}

/// True where [`served`] planned the binds of this run.
pub fn planned() -> bool {
    PLANNED.load(Ordering::Relaxed)
}

/// `PODVM_BINDS` of the volumes, in `env`, where pod-enter binds them on
/// the namespace rung of this run.
pub(super) fn push_env(
    volumes: &[podvm_enter::userland::Volume],
    ro: &[String],
    on_namespace: bool,
    base: Option<&str>,
    env: &mut Vec<String>,
) {
    env.retain(|e| !e.starts_with("PODVM_BINDS=") && !e.starts_with("PODVM_BINDS_BASE="));
    if !planned() || !on_namespace || volumes.is_empty() {
        return;
    }
    let pairs: Vec<String> = volumes
        .iter()
        .map(|v| {
            let ro = if ro.contains(&v.guest) { ":ro" } else { "" };
            format!("{}:{}{ro}", v.guest, v.host)
        })
        .collect();
    env.push(format!("{VAR}={}", pairs.join(",")));
    if let Some(base) = base {
        env.push(format!("{BASE_VAR}={base}"));
    }
}

/// PodUtils/podrunner#277: a bind holds a descriptor of a source, and so
/// does the handler of the syscall tier, which opens each volume once
/// (T-1972); the loader family of the userland rung maps each path at each
/// call and holds none.
pub(super) fn refuse_base_off_binds(
    verb: &str,
    base: bool,
    userland: Option<&crate::lifecycle::Userland>,
) -> Result<(), i32> {
    if base && userland.is_some_and(|u| u.dispatch_exe.is_none()) {
        eprintln!(
            "podvm {verb}: --podvm-volume-base needs a bind of each volume (the namespace \
             rung) or the syscall tier (--podvm-tier=syscall), and this run entered the \
             loader family of the userland rung, which maps each path at each call and \
             holds no descriptor of a source"
        );
        return Err(podvm_image::error::EXIT_RUNTIME_ERROR);
    }
    Ok(())
}

/// The base of `--podvm-volume-base` for the handler of the syscall tier,
/// which opens each volume under it (`PODVM_VOLUME_BASE`, T-1972).
pub(super) fn push_dispatch_base(
    userland: Option<&crate::lifecycle::Userland>,
    base: Option<&str>,
    env: &mut Vec<String>,
) {
    env.retain(|e| !e.starts_with("PODVM_VOLUME_BASE="));
    if let (Some(b), true) = (base, userland.is_some_and(|u| u.dispatch_exe.is_some())) {
        env.push(format!("PODVM_VOLUME_BASE={b}"));
    }
}

/// The volumes at or under `/tmp`, and the others: in the user namespace of
/// a foreground run, the first go to pod-enter (OQ-01, T-1970).
pub(super) fn split_tmp(
    volumes: &[podvm_enter::userland::Volume],
) -> (
    Vec<podvm_enter::userland::Volume>,
    Vec<podvm_enter::userland::Volume>,
) {
    let at_tmp = |g: &str| g == "/tmp" || g.starts_with("/tmp/");
    if !crate::userns::active() {
        return (Vec::new(), volumes.to_vec());
    }
    volumes.iter().cloned().partition(|v| at_tmp(&v.guest))
}

/// `PODVM_BINDS` of the volumes of a foreground run that pod-enter binds
/// after its tmpfs on `/tmp` (OQ-01, T-1970).
pub(super) fn push_foreground(
    at_tmp: &[podvm_enter::userland::Volume],
    ro: &[String],
    base: Option<&str>,
    env: &mut Vec<String>,
) {
    if at_tmp.is_empty() {
        return;
    }
    let pairs: Vec<String> = at_tmp
        .iter()
        .map(|v| {
            let ro = if ro.contains(&v.guest) { ":ro" } else { "" };
            format!("{}:{}{ro}", v.guest, v.host)
        })
        .collect();
    env.retain(|e| !e.starts_with("PODVM_BINDS=") && !e.starts_with("PODVM_BINDS_BASE="));
    env.push(format!("{VAR}={}", pairs.join(",")));
    if let Some(base) = base {
        env.push(format!("{BASE_VAR}={base}"));
    }
}
