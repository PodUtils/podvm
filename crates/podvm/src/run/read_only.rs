//! TODO/cli.md T-2108 (OR-B42-02): `--read-only` keeps the namespace rung.
//!
//! Before, `--read-only` always asked for the read-only layer of the
//! interposer, which only the loader family of the userland rung has: a
//! run of uid 1000 left the namespace rung for the userland rung (the PID
//! table of the host, no `--tmpfs` with a size), and a raw system call
//! wrote the host's `/tmp`. Now, where the namespace rung holds and podvm
//! owns (or opens) the mount namespace of the entry, the root is a
//! read-only bind of the tree (`podvm_enter::read_only`), and the record
//! carries `PODVM_RO_ROOT=1` for the launcher and for `exec`. Each other
//! run keeps the read-only layer of the interposer, as before.

use std::sync::atomic::{AtomicBool, Ordering};

static MOUNTED: AtomicBool = AtomicBool::new(false);

/// True where the root of this run is a read-only mount ([`layer`]).
pub fn mounted() -> bool {
    MOUNTED.load(Ordering::Relaxed)
}

/// True where podvm makes the root read-only by a mount: the probe selects
/// the namespace rung, no launch rung is forced, the init is podvm's, and a
/// process of podvm owns the mount namespace of the entry: the launcher of
/// a detached run, pid 2 of podvm's user namespace, or a caller of uid 0,
/// which opens one ([`podvm_enter::read_only::own_mounts`]).
pub(super) fn by_mount(
    detached: bool,
    forced: bool,
    init_image: bool,
    uid0: bool,
    findings: &podvm_probe::Findings,
) -> bool {
    let ns = podvm_probe::select::Selection::choose(findings).rung
        == podvm_probe::select::Rung::Namespace;
    ns && !forced && !init_image && (detached || crate::userns::active() || uid0)
}

/// True where `--read-only` of `o` needs the read-only layer of the
/// interposer (the loader family); notes where a mount serves it instead.
pub(super) fn layer(
    o: &super::Opts,
    detached: bool,
    forced: bool,
    findings: &podvm_probe::Findings,
) -> bool {
    let uid0 = crate::userns::host_root();
    let mount = o.read_only && by_mount(detached, forced, o.init_image, uid0, findings);
    MOUNTED.store(mount, Ordering::Relaxed);
    o.read_only && !mount
}

/// The read-only root of a foreground run, where its record asks for it:
/// a mount namespace of its own for a caller outside podvm's user
/// namespace, then the seal, after each write of podvm to the tree and
/// before the open of the root of the entry.
pub(super) fn seal_foreground(p: &crate::lifecycle::Prepared) -> Result<(), String> {
    if p.userland.is_some() || !podvm_enter::read_only::wanted(&p.env) {
        return Ok(());
    }
    if !crate::userns::active() {
        podvm_enter::read_only::own_mounts()?;
    }
    let volumes = podvm_supervise::launcher::binds_of(&p.env);
    let tmpfs = podvm_supervise::launcher::tmpfs::of(&p.env)?;
    podvm_enter::read_only::seal(std::path::Path::new(&p.rootfs), &volumes, &tmpfs)
}
