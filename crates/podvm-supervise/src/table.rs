//! The container table: running state, in the launcher, persisted to the store.
//!
//! [`TODO/supervise.md`](../../../TODO/supervise.md) T-0604 and T-0601.
//!
//! ⭐ **The table is `pod_supervise::table` (PodUtils/podcrates).** This
//! module gives podvm's record fields ([`Detail`]), the paths of a container
//! under the store, the ownership memo of `podvm-interpose`, and the table
//! calls with a [`Store`] in place of the crate's settings.
//!
//! ⛔ **Running state is launcher state, and it is never inferred from the
//! filesystem.** `TOOL.md` section 6.6 and T-0601. A launcher that was killed
//! leaves the table saying `running` until a reconcile takes its lock:
//! T-0604's Decision, in `pod_supervise::table::reconcile`.

use std::path::PathBuf;

use podvm_image::Store;
use serde::{Deserialize, Serialize};

pub use pod_supervise::table::{State, CONTAINERS_DIR, TABLE_FILE, TABLE_VERSION};

/// A podvm container record: the crate's fields and podvm's [`Detail`],
/// one JSON object in `containers.json`.
pub type Container = pod_supervise::table::Container<Detail>;
pub type Table = pod_supervise::table::Table<Detail>;

/// What podvm records beside the crate's fields.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Detail {
    /// The reference as the caller wrote it.
    pub image: String,
    pub manifest_digest: String,
    pub rootfs: String,
    /// The rung the launcher selected when it started this container.
    pub rung: String,
    /// ⭐ TODO/enter.md T-1411. The exact argv the launcher execs on a
    /// detached userland entry: the image's loader, the payload by host
    /// path, then the payload's own arguments (the applet name riding
    /// beside a resolved busybox file where one was inserted). Empty
    /// everywhere else, including chroot records and the memfd family,
    /// which has no detached driver: the launcher enters through
    /// `spawn_selected` there. The libraries, the `PODVM_MAPS` table
    /// and `PODVM_GUEST_EXE` ride in `env`, decided beside this argv at
    /// `create`. `serde(default)` so a table written before this parses.
    #[serde(default)]
    pub userland_exec: Vec<String>,
    /// ⭐ What the completion layer did to this container's rootfs, one line
    /// per fixup that changed a byte or failed.
    /// [`TODO/cli.md`](../../../TODO/cli.md) T-0804 rule 3: `inspect` reports
    /// the true mode PER CONTAINER, and a `/dev/null` that is a regular file is
    /// part of that mode. ⚠ `serde(default)` so a table written before M5 still
    /// parses; an empty list there means "not recorded", which is why
    /// [`Detail::completion_note`] says so rather than printing nothing.
    #[serde(default)]
    pub completion: Vec<String>,
    #[serde(default)]
    pub completion_degraded: usize,
    /// TODO/cli.md T-1655: the `--ulimit` specs and the no-new-privileges
    /// bit of the run, which the launcher sets in the child before the exec.
    #[serde(default)]
    pub hardening: Hardening,
}

/// The limits that a run asked for, kept in its record so that the
/// launcher of a detached run sets them as a foreground run does.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Hardening {
    /// Each `--ulimit NAME=SOFT[:HARD]` as the caller wrote it, checked at
    /// the parse (`podvm_enter::ulimit`).
    #[serde(default)]
    pub ulimits: Vec<String>,
    /// `--security-opt no-new-privileges`.
    #[serde(default)]
    pub no_new_privs: bool,
    /// The SPEC of `--podvm-confine` (TODO/enter.md T-1658): the launcher of
    /// `run -d` and each `exec` apply the same layer.
    #[serde(default)]
    pub confine: Option<String>,
    /// Each `--label KEY=VALUE` of the run (TODO/cli.md T-1695): `ps
    /// --filter label=` and `inspect` read them.
    #[serde(default)]
    pub labels: Vec<String>,
    /// The absolute path of `--report` of a detached run (TODO/cli.md
    /// T-1825): the launcher writes the end of the payload into it.
    #[serde(default)]
    pub report: Option<String>,
}

impl Hardening {
    /// Whether the labels match a `--filter label=` value: `KEY` matches a
    /// label with that key, `KEY=VALUE` the label exactly (docker's rule).
    pub fn matches_label(&self, want: &str) -> bool {
        self.labels.iter().any(|l| match want.split_once('=') {
            Some(_) => l == want,
            None => l.split('=').next() == Some(want),
        })
    }

    /// The confine layer of the record with host paths, for the loader
    /// family: each guest path through `maps` (TODO/enter.md T-1658).
    pub fn confine_host_spec(
        &self,
        maps: &podvm_enter::userland::Maps,
        emulator: Option<&str>,
    ) -> Result<Option<podvm_enter::confine::Spec>, String> {
        let Some(s) = &self.confine else {
            return Ok(None);
        };
        let ask = podvm_enter::confine::parse(s)
            .map_err(|e| format!("the confine layer of the record ({s:?}): {e}"))?;
        // PodUtils/podrunner#304 (TODO/enter.md T-1982): the launcher of
        // `run -d` takes the same rules as a foreground run; without podvm's
        // own binary the exec of the syscall tier was denied (EACCES, 125).
        userland_host_spec(&ask, maps, emulator).map(Some)
    }

    /// The confine layer of the record for the tree at `rootfs`, or None.
    /// A SPEC that no longer parses is an error: the layer is never dropped.
    pub fn confine_spec(&self, rootfs: &str) -> Result<Option<podvm_enter::confine::Spec>, String> {
        let Some(s) = &self.confine else {
            return Ok(None);
        };
        let ask = podvm_enter::confine::parse(s)
            .map_err(|e| format!("the confine layer of the record ({s:?}): {e}"))?;
        Ok(Some(podvm_enter::confine::spec_in(
            &ask,
            std::path::Path::new(rootfs),
        )))
    }

    /// The limits of the specs. A spec that no longer parses (a record of
    /// another podvm) is left out: the parse at `run` refused each bad one.
    pub fn limits(&self) -> Vec<podvm_enter::Limit> {
        self.ulimits
            .iter()
            .filter_map(|s| podvm_enter::ulimit(s).ok())
            .collect()
    }
}

impl Detail {
    /// ⛔ A dash where the record predates M5, never an empty list read as
    /// "nothing was done": the two are different facts and only one of them is
    /// a claim about the rootfs.
    pub fn completion_note(&self) -> String {
        if self.completion.is_empty() {
            "-".to_string()
        } else {
            self.completion.join("; ")
        }
    }
}

/// Where one container's own files live: its lock, its log, its control
/// socket and its memo. ⚠ Under the store, so the same containment gate
/// covers them.
pub fn dir(store: &Store, id: &str) -> PathBuf {
    pod_supervise::table::dir(&crate::settings(store), id)
}

pub fn lock_path(store: &Store, id: &str) -> PathBuf {
    pod_supervise::table::lock_path(&crate::settings(store), id)
}

pub fn log_path(store: &Store, id: &str) -> PathBuf {
    pod_supervise::table::log_path(&crate::settings(store), id)
}

pub fn control_path(store: &Store, id: &str) -> PathBuf {
    pod_supervise::table::control_path(&crate::settings(store), id)
}

/// Where one container's ownership memo lives: beside its record, on the host.
///
/// ⛔ T-0710's ruling. The payload cannot reach it; the interposer is handed a
/// descriptor to it at spawn, re-handed on every `exec` re-entry. Nothing on
/// the host reads it to decide anything: it is the payload's own view of
/// ownership, not evidence.
pub fn memo_path(store: &Store, id: &str) -> PathBuf {
    dir(store, id).join("ownership.memo")
}

/// The variable carrying the memo descriptor number into the payload.
///
/// One fact in two homes with `podvm-interpose`'s `MEMO_FD_VAR`: that crate
/// cannot depend on this one, so the two spellings are asserted to agree by
/// driving the behaviour end to end in
/// `experiments/105-interpose-ownership.sh`.
pub const MEMO_FD_VAR: &str = "PODVM_MEMO_FD";

/// The descriptor number the payload sees the memo on.
///
/// High enough to miss 0, 1, 2 and the log sinks the launcher passes, low
/// enough to stay under any conservative `FD_SETSIZE`. The host's own number
/// for the file is unrelated: `podvm-enter`'s `Fds.pass` dups it to this one
/// in the child before the `chroot`, and `dup3` with flags 0 clears
/// `CLOEXEC`, so it survives the `execve` the readiness pipe does not.
pub const MEMO_CHILD_FD: i64 = 17;

/// The `PODVM_MEMO_FD` entry for a payload environment, once the host has the
/// file open.
pub fn memo_fd_env() -> String {
    format!("{MEMO_FD_VAR}={MEMO_CHILD_FD}")
}

/// The memo descriptor the environment hands the payload, or `None`.
///
/// Decimal digits only, within `c_int` range; the interposer parses the same
/// grammar, so anything else is no descriptor rather than a guess at one.
pub fn memo_fd_of(env: &[String]) -> Option<i64> {
    let v = env
        .iter()
        .rev()
        .find_map(|e| e.strip_prefix(&format!("{MEMO_FD_VAR}=")))?;
    if v.is_empty() || v.len() > 10 || !v.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n: i64 = v.parse().ok()?;
    (n >= 0 && n <= i32::MAX as i64).then_some(n)
}

mod memo;
pub use memo::*;

/// Read the table, with no lock. ⛔ Safe because the table is REPLACED by a
/// rename, exactly as the image index is: invariant I2 of
/// `crates/podvm-image/src/store.rs`.
pub fn read(store: &Store) -> crate::Result<Table> {
    Ok(pod_supervise::table::read(&crate::settings(store))?)
}

/// Read the table, hand it to `f`, and write back what `f` leaves.
///
/// ⛔ **Under the store's own lock, invariant I1.** One writer at a time over
/// one table, and the same lock the image index takes, so a `prune` and a
/// container write cannot interleave either.
pub fn update<T>(
    store: &Store,
    f: impl FnOnce(&mut Table) -> crate::Result<T>,
) -> crate::Result<T> {
    pod_supervise::table::update(&crate::settings(store), f)
}

/// ⛔ T-0604. Mark as `Dead` anything the table calls running whose launcher is
/// gone, and say when it was noticed.
pub fn reconcile(store: &Store) -> crate::Result<Table> {
    Ok(pod_supervise::table::reconcile(&crate::settings(store))?)
}

/// Resolve a name, a full id or an unambiguous id prefix.
///
/// ⛔ Ambiguity is an error rather than a choice.
pub fn find(store: &Store, table: &Table, want: &str) -> crate::Result<Container> {
    Ok(pod_supervise::table::find(
        &crate::settings(store),
        table,
        want,
    )?)
}

#[cfg(test)]
mod tests;

/// The confine layer of a family of the userland rung, in host paths: the
/// paths of `ask` through the table `maps`, and what the payload's process
/// runs beside the image (TODO/enter.md T-1819, T-1892, T-1902, T-1982).
///
/// - podvm's own binary, read and execute: the native-static rung and the
///   syscall tier exec podvm itself, and a static child of the loader
///   family enters through it (T-1724);
/// - the host emulator of the emulate tier (`MACHINE:HOST`), the same;
/// - the files that musl opens with its own `syscall` instruction
///   (`/etc/resolv.conf`, `/etc/hosts`, `/etc/services`), read, where the
///   spec lets the payload read the guest path (PodUtils/podvm#396).
///
/// A foreground run (`lifecycle::confine::decide`) and the launcher of
/// `run -d` both take it, so the two cannot differ.
pub fn userland_host_spec(
    ask: &podvm_enter::confine::Ask,
    maps: &podvm_enter::userland::Maps,
    emulator: Option<&str>,
) -> Result<podvm_enter::confine::Spec, String> {
    use podvm_enter::confine::Access;
    let rx = Access::READ | Access::EXECUTE;
    let mut spec = podvm_enter::confine::host_spec(ask, maps)?;
    if let Ok(me) = std::env::current_exe() {
        spec = spec.allow(me.to_string_lossy().to_string(), rx);
    }
    if let Some((_, host)) = emulator.and_then(|e| e.split_once(':')) {
        spec = spec.allow(host.to_string(), rx);
    }
    for host in raw_reads(ask) {
        spec = spec.allow(host, Access::READ);
    }
    Ok(spec)
}

/// TODO/enter.md T-1985: the variable of the write table of the confine
/// layer, which the interposer and the handler of the syscall tier read.
pub const WRITE_TABLE_VAR: &str = "PODVM_WRITE_HOST";

/// TODO/enter.md T-1985: drop each write table of the caller from `env`,
/// and add the table of `spec` (host paths, a family of the userland rung)
/// where there is one. Landlock governs no change of mode, owner or times,
/// so the interposer and the handler refuse each one on a file outside
/// each path with write. A path that the table cannot spell (a `:` or a
/// NUL) is a refusal by name: a run never goes on without its table.
pub fn set_write_table(
    env: &mut Vec<String>,
    spec: Option<&podvm_enter::confine::Spec>,
) -> Result<(), String> {
    use podvm_enter::confine::{Access, Spec};
    env.retain(|e| e.split_once('=').map(|(n, _)| n) != Some(WRITE_TABLE_VAR));
    let Some(spec) = spec else {
        return Ok(());
    };
    // The exec table of pod-confine spells the paths of the rules with
    // write the same way (each path once, resolved, `:` separated), so they
    // go through it as rules with execute: no second copy of its code.
    let writes = spec
        .paths
        .iter()
        .filter(|r| r.access.contains(Access::WRITE))
        .fold(Spec::new(), |s, r| s.allow(r.path.clone(), Access::EXECUTE));
    let table = writes
        .exec_table()
        .resolved()
        .encode()
        .map_err(|e| format!("the write table of the confine layer: {e}"))?;
    env.push(format!("{WRITE_TABLE_VAR}={table}"));
    Ok(())
}

/// PodUtils/podvm#396 (TODO/enter.md T-1902): the files that musl opens
/// past each symbol of the interposer, so the kernel resolves them on the
/// host. Where the spec lets the payload read the guest path, the host
/// file (its link resolved) gets read too; without it the resolver of musl
/// found no name server, and each name failed with EAI_AGAIN.
fn raw_reads(ask: &podvm_enter::confine::Ask) -> Vec<String> {
    const RAW: [&str; 3] = ["/etc/resolv.conf", "/etc/hosts", "/etc/services"];
    RAW.iter()
        .filter(|g| guest_readable(ask, g))
        .filter_map(|g| std::fs::canonicalize(g).ok())
        .filter(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

/// True where a rule of the spec covers the guest path `g` (each kind of
/// rule gives read).
pub fn guest_readable(ask: &podvm_enter::confine::Ask, g: &str) -> bool {
    ask.read.iter().chain(&ask.write).chain(&ask.exec).any(|p| {
        let p = p.trim_end_matches('/');
        p.is_empty() || g == p || g.strip_prefix(p).is_some_and(|r| r.starts_with('/'))
    })
}
