//! TODO/enter.md T-2101 (PodUtils/podrunner#368): `-v HOST:GUEST:overlay`.
//!
//! The namespace rung holds it: an overlayfs mount in a mount namespace
//! that podvm owns, with HOST as the read-only lower layer and the upper
//! layer in the tree of the container (`podvm_enter::overlay`). pid 2 of a
//! foreground run mounts it (`binds::bind`); the launcher of `run -d`,
//! `create` and `start` mounts it from `PODVM_BINDS` (`GUEST:HOST:overlay`).
//! Each other rung and tier refuses it by name at 125, and never falls back
//! to a writable bind or to a path rewrite: no write may reach HOST.

use super::*;
use crate::tier::Tier;

/// The rungs that hold an overlay volume, for `system info`.
pub const RUNGS: &[&str] = &["namespace"];

fn refuse(verb: &str, o: &Opts, why: &str) -> i32 {
    let g = o.overlay_volumes.first().map_or("", String::as_str);
    let host = o
        .volumes
        .iter()
        .find(|v| v.guest == g)
        .map_or("", |v| v.host.as_str());
    eprintln!("podvm {verb}: -v {host}:{g}:overlay: {why} (TODO/enter.md T-2101)");
    EXIT_FLAG_ERROR
}

/// The refusal of a tier or a mode that holds no overlay, by name, before
/// anything is fetched; and the guest paths that a layer cannot take.
pub(super) fn refuse_tier(verb: &str, tier: Tier, o: &Opts) -> Result<(), i32> {
    if o.overlay_volumes.is_empty() {
        return Ok(());
    }
    let all: Vec<String> = o.volumes.iter().map(|v| v.guest.clone()).collect();
    if let Err(why) = podvm_enter::overlay::check_guests(&o.overlay_volumes, &all) {
        return Err(refuse(verb, o, &why));
    }
    let tier_word = match tier {
        Tier::Ladder => None,
        Tier::Machine => Some("--podvm-tier=machine shares a host directory over 9p"),
        Tier::Incus => Some("--podvm-tier=incus gives a disk device of the instance"),
        Tier::Runsc => Some("--podvm-tier=runsc binds a volume in its sandbox"),
        Tier::Emulate | Tier::Syscall => Some(
            "the native-static rung of this tier opens a volume as a directory of its \
             handler, which has no copy-up layer",
        ),
    };
    if let Some(what) = tier_word {
        return Err(refuse(
            verb,
            o,
            &format!("{what}; only the namespace rung holds an overlay"),
        ));
    }
    if o.init_image {
        return Err(refuse(
            verb,
            o,
            "--podvm-init=image binds the volumes in the supervisor of the init, which \
             mounts no overlay yet; run with podvm's init",
        ));
    }
    Ok(())
}

/// The refusal of a run whose entry cannot hold an overlay: each family of
/// the userland rung (a path table cannot hold a copy-up layer, a whiteout
/// or a merged listing), and a run with no mount namespace of podvm's own
/// on the namespace rung.
pub(super) fn refuse_unserved(
    verb: &str,
    o: &Opts,
    userland: Option<&crate::lifecycle::Userland>,
) -> Result<(), i32> {
    if o.overlay_volumes.is_empty() {
        return Ok(());
    }
    if let Some(u) = userland {
        let rung = if u.dispatch_exe.is_some() {
            "the native-static rung opens a volume as a directory of its handler"
        } else if u.family == podvm_enter::userland::Family::Loader {
            "the userland rung serves a volume through the path table of its interposer"
        } else {
            "the memfd family of the userland rung has no path table"
        };
        return Err(refuse(
            verb,
            o,
            &format!(
                "{rung}, which cannot hold a copy-up layer (a copy at the first write, a \
                 whiteout, a merged listing); only the namespace rung holds an overlay, \
                 and this host or this run does not take it"
            ),
        ));
    }
    if !(crate::userns::active() || super::binds::planned()) {
        return Err(refuse(
            verb,
            o,
            "this run has no mount namespace of podvm's own on the namespace rung (a \
             foreground run of uid 0, or PODVM_USERNS=0), so podvm cannot mount the layer",
        ));
    }
    Ok(())
}

/// The overlay volumes of `volumes`, in their order, and the others.
pub(super) fn split(
    volumes: &[podvm_enter::userland::Volume],
    overlays: &[String],
) -> (
    Vec<podvm_enter::userland::Volume>,
    Vec<podvm_enter::userland::Volume>,
) {
    volumes
        .iter()
        .cloned()
        .partition(|v| overlays.contains(&v.guest))
}

/// One line of the banner for each overlay volume.
pub(super) fn banner(o: &Opts, rootfs: &std::path::Path) -> String {
    let (layers, _) = split(&o.volumes, &o.overlay_volumes);
    let mut out = String::new();
    for (n, v) in layers.iter().enumerate() {
        let dir = podvm_enter::overlay::layer_dir(rootfs, n).unwrap_or_default();
        out.push_str(&format!(
            "podvm: -v {}:{}:overlay is an overlayfs mount: {} is the read-only lower \
             layer, and each write goes to {}, which the removal of the container \
             removes (TODO/enter.md T-2101)\n",
            v.host,
            v.guest,
            v.host,
            dir.display()
        ));
    }
    out
}

/// The guest path of a `:ro` or an `:overlay` volume in its list of `o`.
pub(super) fn note_mode(mode: crate::lifecycle::volume::Mode, guest: &str, o: &mut Opts) {
    use crate::lifecycle::volume::Mode;
    match mode {
        Mode::Ro => o.ro_volumes.push(guest.to_string()),
        Mode::Overlay => o.overlay_volumes.push(guest.to_string()),
        Mode::Rw => {}
    }
}
