//! `TOOL.md` section 6.1: the probe set, disposable children, mode selection.
//! The work is in [`TODO/probe.md`](../../../TODO/probe.md), T-0101 to T-0110.
//!
//! `pod-hostprobe` (PodUtils/podcrates) runs the probes: the raw system
//! calls, the disposable children, the probe table, the selection of the
//! rung, the mount, machine and supervise legs, the identity and the write
//! probe. This crate gives podvm's settings to it, and keeps podvm's report,
//! exit codes, JSON writer and non-goals.
//!
//! ⭐ **Four rules decide whether the probes are right rather than merely
//! finished**, and `pod-hostprobe` keeps each one:
//!
//! 1. every probe runs in a disposable child ([`child`], T-0101);
//! 2. the errno is recorded, never a boolean ([`sys::Errno`], T-0101);
//! 3. "could not run" is a third state and never reads as "denied"
//!    ([`verdict::Verdict::Skip`], T-0109);
//! 4. the controls travel with the bogus-argument discriminator, so a probe
//!    that has stopped discriminating says so ([`select::Selection`], T-0102).
#![forbid(unsafe_op_in_unsafe_fn)]

pub mod exit;
pub mod inside;
pub mod isolation; // TODO/cli.md T-2053
pub mod json;
pub mod nongoals;
pub mod report;
pub mod select;

pub use pod_hostprobe::{
    binfmt, cgroup, child, identity, interp, machine, mounts, probes, supervise, sys, verdict,
    writable, Findings, Settings,
};

/// The private argument a probe child is re-executed with.
///
/// ⚠ It is not a public verb. `podvm probe --help` names it as internal, and
/// the CLI dispatches it before anything else so a child cannot fall into the
/// ordinary argument surface.
pub const CHILD_FLAG: &str = "--probe-child";

/// podvm's values for `pod-hostprobe`: the child flag, the `argv[0]` of a
/// probe child, and the variable that the space and exec messages name.
pub fn settings() -> Settings {
    Settings {
        child_flag: CHILD_FLAG.to_string(),
        child_name: "podvm".to_string(),
        directory_setting: Some("$PODVM_STORE".to_string()),
        // podcrates `v0.61.0` (podcrates@T-751): the rows that send a packet
        // out stay off; podvm's probe sends nothing past the host.
        network: None,
    }
}

/// The directories of the write probe: the fixed list, then `$PODVM_STORE`
/// and each of [`writable::VARIABLES`] that is set, each with the variable
/// that named it, then the working directory.
pub fn candidates() -> Vec<(String, String)> {
    candidates_from(|var| std::env::var_os(var))
}

fn candidates_from(get: impl Fn(&str) -> Option<std::ffi::OsString>) -> Vec<(String, String)> {
    let named: Vec<(String, String)> = std::iter::once("PODVM_STORE")
        .chain(writable::VARIABLES.iter().copied())
        .filter_map(|var| get(var).map(|v| (v.to_string_lossy().to_string(), format!("${var}"))))
        .collect();
    writable::candidates(&named)
}

/// Is an emulator in the way, from the variables of this process.
pub fn interpreter() -> interp::Interpreter {
    interp::detect(&|var| std::env::var_os(var).is_some())
}

/// The cache key of `id` with the emulator check of this process.
///
/// ⛔ In a run that podvm gave a mount namespace of its own (the user
/// namespace of `run` and `probe`, a join of `exec`), the `mnt_ns`
/// component is the namespace that the run came from: the new one is
/// fresh at each invocation, so a key that read it never matched the next
/// invocation (TODO/enter.md T-1953, PodUtils/podvm#558).
pub fn confinement_key(id: &identity::Identity) -> identity::ConfinementKey {
    confinement_key_from(id, ENTRY_MOUNT_NS.get())
}

/// [`confinement_key`] with `outer`, the mount namespace that a run came
/// from, in place of the live one. `Some(None)` is a namespace that could
/// not be read: the key then has a hole and never matches.
pub fn confinement_key_from(
    id: &identity::Identity,
    outer: Option<&Option<String>>,
) -> identity::ConfinementKey {
    let mut key = identity::confinement_key(id, &interpreter());
    if let Some(outer) = outer {
        key.mnt_ns = outer.clone();
    }
    key
}

/// The mount namespace of the entry, once podvm made one of its own
/// ([`own_mount_namespace`]).
static ENTRY_MOUNT_NS: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// `mnt:[INODE]`, the mount namespace of this process, or `None` where
/// the kernel does not say.
pub fn mount_namespace() -> Option<String> {
    let link = std::fs::read_link(identity::MNT_NS_PATH).ok()?;
    link.into_os_string().into_string().ok()
}

/// Record that this process runs in a mount namespace that podvm made at
/// its start: `outer` is the one that podvm read before.
pub fn own_mount_namespace(outer: Option<String>) {
    let _ = ENTRY_MOUNT_NS.set(outer);
}

/// The entry that this process measures in: the answer of a run in a
/// mount namespace of its own is not the answer of a plain process of the
/// same user, so each has its own cache file (`podvm-image`).
pub fn entry_variant() -> &'static str {
    if ENTRY_MOUNT_NS.get().is_some() {
        "userns"
    } else {
        "plain"
    }
}

/// Run the whole set with podvm's settings. One disposable child per probe,
/// in a fixed order.
pub fn run() -> Findings {
    pod_hostprobe::run(&settings(), &candidates())
}

/// What a probe child does: run exactly one probe, in this process, and report
/// it on stdout and in the exit status.
///
/// ⛔ Both, and they must agree. [`child::Spawner`] refuses a child whose line
/// and exit status disagree, because a child that contradicts itself has
/// established no verdict. `TODO/probe.md` T-0109 rule 1.
pub fn run_child(name: &str) -> i32 {
    if probes::find(name).is_none() {
        eprintln!("podvm: no probe named {name:?}");
        return 2;
    }
    pod_hostprobe::run_child(name, &mut std::io::stdout().lock())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TODO/enter.md T-1953 (PodUtils/podvm#558): the key of a run in a
    /// mount namespace of its own names the namespace that it came from,
    /// so two invocations in two fresh namespaces have one key. A
    /// namespace that was not read is a hole, which never matches.
    #[test]
    fn a_fresh_mount_namespace_does_not_change_the_key() {
        let id = identity::read();
        let live = confinement_key_from(&id, None);
        assert_eq!(live.mnt_ns, mount_namespace(), "no entry: the live one");
        let from = Some("mnt:[4026531841]".to_string());
        let a = confinement_key_from(&id, Some(&from));
        let b = confinement_key_from(&id, Some(&from));
        assert_eq!(a.mnt_ns, from);
        assert!(a.differences(&b).is_empty());
        // The other components are the live ones, so a change of the
        // confinement itself still changes the key.
        let moved = identity::ConfinementKey {
            cap_eff: Some("0".into()),
            ..a.clone()
        };
        assert_eq!(a.differences(&moved), vec!["cap_eff"]);
        let hole = confinement_key_from(&id, Some(&None));
        assert!(!hole.is_complete());
        assert!(hole.differences(&hole).contains(&"mnt_ns"));
    }

    #[test]
    fn a_probe_name_that_does_not_exist_exits_two() {
        assert_eq!(run_child("no such probe"), 2);
    }

    /// TODO/probe.md T-1921: the leg in a user namespace is not the
    /// caller's `chroot` (podcrates@T-758 in `v0.61.0`; podvm held a
    /// shadow of the function before).
    #[test]
    fn chroot_usable_reads_the_callers_own_row_only() {
        use verdict::Outcome;
        let f = |rows: Vec<(&'static str, Outcome)>| Findings {
            rows,
            identity: identity::Identity::default(),
            writable: Vec::new(),
            self_exe: String::new(),
        };
        let leg = probes::USERNS_CHROOT;
        let host = probes::CHROOT;
        assert!(
            probes::find(host).is_some(),
            "the crate has the row {host:?}"
        );
        let denied = || Outcome::denied(sys::Errno(1));
        assert!(!probes::chroot_usable(&f(vec![
            (leg, Outcome::ok()),
            (host, denied())
        ])));
        assert!(!probes::chroot_usable(&f(vec![(leg, Outcome::ok())])));
        assert!(probes::chroot_usable(&f(vec![
            (leg, denied()),
            (host, Outcome::ok())
        ])));
        assert!(!probes::chroot_usable(&f(vec![])));
    }

    #[test]
    fn the_settings_are_podvms() {
        let s = settings();
        assert_eq!(s.child_flag, "--probe-child");
        assert_eq!(s.child_name, "podvm");
        assert_eq!(s.directory_setting.as_deref(), Some("$PODVM_STORE"));
    }

    #[test]
    fn the_store_is_the_first_named_candidate() {
        let env = |var: &str| match var {
            "PODVM_STORE" => Some("/podvm-store".into()),
            "TMPDIR" => Some("/podvm-tmp".into()),
            _ => None,
        };
        let got = candidates_from(env);
        let store = got
            .iter()
            .position(|c| *c == ("/podvm-store".into(), "$PODVM_STORE".into()));
        let tmp = got
            .iter()
            .position(|c| *c == ("/podvm-tmp".into(), "$TMPDIR".into()));
        assert!(store.is_some() && store < tmp, "{got:?}");
        assert_eq!(got[0].0, "/tmp", "the fixed list comes first: {got:?}");
    }
}
