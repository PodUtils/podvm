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
/// TODO/enter.md T-2101: `:overlay` for an overlay volume, which the
/// launcher mounts itself (`launcher::overlays_of`).
pub(super) fn push_env(
    volumes: &[podvm_enter::userland::Volume],
    (ro, overlay): (&[String], &[String]),
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
            let mode = match (ro.contains(&v.guest), overlay.contains(&v.guest)) {
                (_, true) => ":overlay",
                (true, false) => ":ro",
                (false, false) => "",
            };
            format!("{}:{}{mode}", v.guest, v.host)
        })
        .collect();
    env.push(format!("{VAR}={}", pairs.join(",")));
    if let Some(base) = base {
        env.push(format!("{BASE_VAR}={base}"));
    }
}

/// PodUtils/podrunner#277: a bind holds a descriptor of a source, and so
/// does the handler of the syscall tier, which opens each volume once.
/// The interposer of the loader family opens each volume under the base
/// again at each call (TODO/enter.md T-1972); at the start, each volume
/// under the base must open under it as a directory with no symbolic link.
pub(super) fn check_base_off_binds(
    verb: &str,
    o: &super::Opts,
    userland: Option<&crate::lifecycle::Userland>,
) -> Result<(), i32> {
    let (Some(base), Some(u)) = (o.volume_base.as_deref(), userland) else {
        return Ok(());
    };
    if u.dispatch_exe.is_some() {
        return Ok(());
    }
    for v in &o.volumes {
        if let Err(why) = crate::userns::check_volume_under_base(base, &v.host) {
            eprintln!("podvm {verb}: -v {}:{}: {why}", v.host, v.guest);
            return Err(podvm_image::error::EXIT_RUNTIME_ERROR);
        }
    }
    Ok(())
}

/// The base of `--podvm-volume-base` for the userland rung: the handler of
/// the syscall tier opens each volume under it once, and the interposer of
/// the loader family at each call (`PODVM_VOLUME_BASE`, T-1972).
pub(super) fn push_volume_base(
    userland: Option<&crate::lifecycle::Userland>,
    base: Option<&str>,
    env: &mut Vec<String>,
) {
    env.retain(|e| !e.starts_with("PODVM_VOLUME_BASE="));
    if let (Some(b), true) = (base, userland.is_some()) {
        env.push(format!("PODVM_VOLUME_BASE={b}"));
    }
}

/// The volumes at or under `/tmp`, and the others: in the user namespace of
/// a foreground run, the first go to pod-enter (OQ-01, T-1970). With the
/// init of the image (`init_image`), also each one at or under `/dev` and
/// `/run`, which the supervisor binds after the mounts of the init there
/// (TODO/enter.md T-2017: a `--device` on its `/dev`).
pub(super) fn split_tmp(
    volumes: &[podvm_enter::userland::Volume],
    init_image: bool,
) -> (
    Vec<podvm_enter::userland::Volume>,
    Vec<podvm_enter::userland::Volume>,
) {
    let at_tmp = |g: &str| {
        if init_image {
            crate::initns::mounts::late(g)
        } else {
            g == "/tmp" || g.starts_with("/tmp/")
        }
    };
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

/// True where the caller gives `/tmp` (`--tmpfs /tmp` or `-v H:/tmp`): the
/// syscall tier then adds no `/tmp` of its own (OR-05 of the outside pass
/// of 2026-11-18, TODO/cli.md T-1970).
pub(super) fn tmp_is_given(o: &super::Opts) -> bool {
    o.tmpfs
        .iter()
        .chain(o.volumes.iter().map(|v| &v.guest))
        .any(|t| t == "/tmp")
}

/// TODO/enter.md T-1920: the volumes of a run in podvm's own user namespace
/// as binds on the namespace rung, less those at or under `/tmp`, which
/// pod-enter binds after its tmpfs (OQ-01, T-1970).
/// ⛔ OR-B38-06 of the seventh outside pass (TODO/cli.md T-2072): only after
/// the completion, `/etc/hostname` and each other write of podvm to the
/// tree. Before, the binds came first, and `-v DIR:/etc` or `-v DIR:/dev`
/// got podvm's fixups in the caller's directory (`passwd`, `hosts`, the CA
/// bundle; a `/dev/zero` of 1 MiB). podvm writes no file of its own through
/// a volume: the payload sees the directory as the caller left it. Docker
/// binds its own `hosts`, `hostname` and `resolv.conf` over a volume of
/// `/etc`; podvm has no mount on each rung, and does not.
/// The banner takes the line of [`covered`] first.
pub(super) fn bind(
    verb: &str,
    o: &super::Opts,
    rootfs: &std::path::Path,
    userland: Option<&crate::lifecycle::Userland>,
    report: &podvm_complete::Report,
    banner: &mut String,
) -> Result<(), i32> {
    banner.push_str(&covered(report, &o.volumes));
    if userland.is_some() {
        return Ok(());
    }
    banner.push_str(&super::overlay::banner(o, rootfs));
    let (_, rest) = split_tmp(&o.volumes, o.init_image);
    // TODO/enter.md T-2101: pid 2 mounts each overlay volume after the
    // binds; no other volume shares a path with one (`overlay::refuse_tier`).
    let (layers, rest) = super::overlay::split(&rest, &o.overlay_volumes);
    let base = o.volume_base.as_deref();
    let fail = |e: String| {
        eprintln!("podvm {verb}: {e}");
        podvm_image::error::EXIT_RUNTIME_ERROR
    };
    crate::userns::bind_volumes(rootfs, &rest, &o.ro_volumes, base).map_err(fail)?;
    if crate::userns::active() {
        podvm_enter::overlay::mount_all(rootfs, &layers, base).map_err(fail)?;
    }
    Ok(())
}

/// TODO/cli.md T-2072: one line of the banner for each volume that covers a
/// file of the completion: the payload sees the directory of the caller
/// there, not the fixup, and podvm wrote nothing into it. Empty where no
/// volume covers a fixup.
pub(super) fn covered(
    report: &podvm_complete::Report,
    volumes: &[podvm_enter::userland::Volume],
) -> String {
    use podvm_complete::Action;
    let mut out = String::new();
    for v in volumes {
        let g = v.guest.trim_matches('/');
        let under = |p: &str| g.is_empty() || p == g || p.starts_with(&format!("{g}/"));
        let paths: Vec<&str> = report
            .fixups
            .iter()
            .filter(|f| matches!(f.action, Action::Created | Action::Rewrote))
            .map(|f| f.path.as_str())
            .filter(|p| !p.is_empty() && *p != "-" && under(p))
            .collect();
        if paths.is_empty() {
            continue;
        }
        out.push_str(&format!(
            "podvm: -v {}:{} covers {} of the tree: the payload sees the directory of \
             the volume there, and podvm writes no file into it (TODO/cli.md T-2072)\n",
            v.host,
            v.guest,
            paths.join(", ")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-2072: one rule for each fixup target and each volume: a volume
    /// names each fixup at or under its guest path, and no other.
    #[test]
    fn a_volume_names_each_fixup_that_it_covers() {
        use podvm_complete::{Action, Fixup};
        let fixup = |path: &str, action| Fixup {
            entry: "-",
            id: "t",
            path: path.to_string(),
            action,
            detail: String::new(),
            degraded: false,
        };
        let report = podvm_complete::Report {
            fixups: vec![
                fixup("etc/passwd", Action::Created),
                fixup("etc/hosts", Action::Rewrote),
                fixup("etc/ssl/certs/ca-certificates.crt", Action::Created),
                fixup("dev/zero", Action::Created),
                fixup("dev/pts", Action::Created),
                fixup("etc/group", Action::Unchanged),
                fixup("etcetera", Action::Created),
                fixup("", Action::Created),
            ],
            ..Default::default()
        };
        let vol = |g: &str| podvm_enter::userland::Volume {
            guest: g.to_string(),
            host: "/h".to_string(),
        };
        let cases: &[(&str, &[&str])] = &[
            (
                "/etc",
                &[
                    "etc/passwd",
                    "etc/hosts",
                    "etc/ssl/certs/ca-certificates.crt",
                ],
            ),
            (
                "/etc/",
                &[
                    "etc/passwd",
                    "etc/hosts",
                    "etc/ssl/certs/ca-certificates.crt",
                ],
            ),
            ("/etc/ssl", &["etc/ssl/certs/ca-certificates.crt"]),
            ("/dev", &["dev/zero", "dev/pts"]),
            ("/data", &[]),
        ];
        for (guest, want) in cases {
            let line = covered(&report, &[vol(guest)]);
            if want.is_empty() {
                assert_eq!(line, "", "{guest}");
                continue;
            }
            assert!(line.contains(&want.join(", ")), "{guest}: {line}");
            assert!(
                !line.contains("etc/group") && !line.contains("etcetera"),
                "{line}"
            );
            assert_eq!(line.lines().count(), 1, "{line}");
        }
    }
}
