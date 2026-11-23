//! TODO/cli.md T-1747 (PodUtils/podrunner#110, ask V-3 of podswarm):
//! `--memory`, `--cpus` and `--pids-limit` on the native tiers, by cgroup2
//! where the host delegates a subtree to the caller (`pod_hostprobe::cgroup`,
//! podcrates `v0.51.0`).
//!
//! podvm makes one leaf `podvm-ID` for the run under the delegated
//! directory, writes `memory.max`, `cpu.max` and `pids.max`, and moves
//! itself into it (`0` to `cgroup.procs`) before the payload or its
//! launcher is forked, so that each process of the run inherits it. At its
//! exit, podvm names an OOM kill of the leaf (`memory.events`), moves back
//! and removes the leaf; the leaf of a detached run stays until its
//! processes ended, and the next use removes each empty `podvm-*` leaf
//! (`rmdir` refuses a cgroup with a process).
//!
//! ⛔ No cgroup outside a delegated subtree, and no flag as a silent no-op:
//! each case that cannot hold the limit refuses by name (exit 125).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use podvm_image::error::EXIT_FLAG_ERROR;
use podvm_probe::cgroup::{self, Delegation};

/// The leaf of this run, the cgroup that podvm came from, and its pid.
static LEAF: Mutex<Option<(PathBuf, PathBuf, u32)>> = Mutex::new(None);
/// What [`limits`] found: the delegated directory, the cgroup of podvm,
/// and the files of the leaf, for [`apply`].
type Plan = (PathBuf, PathBuf, Vec<(&'static str, String)>);
static PLAN: Mutex<Option<Plan>> = Mutex::new(None);

unsafe extern "C" {
    fn atexit(f: extern "C" fn()) -> i32;
}

fn refuse(verb: &str, why: &str) -> i32 {
    eprintln!(
        "podvm {verb}: --memory, --cpus and --pids-limit need a delegated cgroup2 subtree: \
         {why}; the incus tier (--podvm-tier=incus) holds them as limits of the instance, \
         and --ulimit nproc= and --ulimit as= hold on each tier (TODO/cli.md T-1747)"
    );
    EXIT_FLAG_ERROR
}

/// The files of the leaf for the limits of `run` (flag, incus key, incus
/// value, as `parse::limits::incus_limit` gives them).
pub(crate) fn files(limits: &[(String, &'static str, String)]) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = limits
        .iter()
        .filter_map(|(_, key, value)| match *key {
            "limits.memory" => Some(("memory.max", value.trim_end_matches('B').to_string())),
            "limits.cpu" => value
                .parse::<u64>()
                .ok()
                .map(|n| ("cpu.max", format!("{} 100000", n * 100_000))),
            "limits.cpu.allowance" => value
                .trim_end_matches('%')
                .parse::<u64>()
                .ok()
                .map(|p| ("cpu.max", format!("{} 100000", p * 1000))),
            "limits.processes" => Some(("pids.max", value.clone())),
            _ => None,
        })
        .collect();
    // docker's default: `--memory` with no `--memory-swap` allows as much
    // swap as memory. With no swap limit, a payload over `memory.max` goes
    // to swap and is not killed (run 43754060132: 200 MB held under 32 MiB,
    // with a swap file of 3 GiB on the runner).
    if let Some((_, m)) = out.iter().find(|(f, _)| *f == "memory.max") {
        let m = m.clone();
        out.push(("memory.swap.max", m));
    }
    out
}

/// Make the controllers of `needed` reach a child of `dir`: they are in its
/// `cgroup.subtree_control`, or this process writes them there.
fn give_controllers(dir: &Path, needed: &[&str]) -> Result<(), String> {
    let control = dir.join("cgroup.subtree_control");
    let on = std::fs::read_to_string(&control).unwrap_or_default();
    let missing: Vec<String> = needed
        .iter()
        .filter(|n| !on.split_whitespace().any(|w| w == **n))
        .map(|n| format!("+{n}"))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    std::fs::write(&control, missing.join(" ")).map_err(|e| {
        format!(
            "{} refused {:?} ({e}): a cgroup that holds a process gives no controller to a \
             child (cgroup v2), and neither it nor its parent took them",
            control.display(),
            missing.join(" ")
        )
    })
}

/// Remove each empty `podvm-*` leaf under `dir`: a detached run that ended.
fn sweep(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        if e.file_name().to_string_lossy().starts_with("podvm-") {
            // EBUSY where a process is in it: the run goes on.
            let _ = std::fs::remove_dir(e.path());
        }
    }
}

/// Early in `prepare`, before anything is fetched: the guest tiers refuse
/// the limits (the machine tier takes `--podvm-mem` and `--podvm-cpus`);
/// a native tier checks the delegation and enables the controllers, or
/// refuses by name.
pub(super) fn limits(
    verb: &str,
    tier: crate::tier::Tier,
    limits: &[(String, &'static str, String)],
) -> Result<(), i32> {
    use crate::tier::Tier;
    if limits.is_empty() {
        return Ok(());
    }
    // TODO/machine.md T-1964: the size of a root disk is a device of an
    // incus instance; no other tier has one to size.
    if limits
        .iter()
        .any(|(_, k, _)| *k == super::parse::limits::ROOT_SIZE)
    {
        eprintln!(
            "podvm {verb}: --storage-opt size= sizes the root disk of an instance of the \
             incus tier (--podvm-tier=incus); this tier has no root disk to size \
             (TODO/machine.md T-1964, T-1968)"
        );
        return Err(EXIT_FLAG_ERROR);
    }
    // PodUtils/podrunner#296 ask 4 (TODO/machine.md T-1967): the init of a
    // machine guest holds `--pids-limit` in a cgroup of the guest.
    if tier == Tier::Machine {
        let rest: Vec<_> = limits
            .iter()
            .filter(|(_, k, _)| *k != "limits.processes")
            .cloned()
            .collect();
        return super::parse::limits::refuse_off_incus(verb, &rest);
    }
    if matches!(tier, Tier::Incus | Tier::Runsc) {
        return super::parse::limits::refuse_off_incus(verb, limits);
    }
    if verb == "create" {
        return Err(refuse(
            verb,
            "podvm makes the leaf when the payload starts, and `create` starts none; \
             `run` and `run -d` hold the limits",
        ));
    }
    let files = files(limits);
    let needed: Vec<&str> = files
        .iter()
        .map(|(f, _)| f.split('.').next().unwrap_or(f))
        .collect();
    let leg = cgroup::probe();
    // A directory that lacks only controllers that these flags do not
    // need serves too (systemd delegates memory and pids by default).
    let path = match &leg.verdict {
        Delegation::Delegated { path, .. } => path.clone(),
        Delegation::None(cgroup::Reason::MissingControllers { path, missing, .. })
            if !missing.iter().any(|m| needed.contains(&m.as_str())) =>
        {
            path.clone()
        }
        v => return Err(refuse(verb, &v.words())),
    };
    let own = leg.own.as_deref().unwrap_or("/");
    let home = Path::new(cgroup::ROOT).join(own.trim_start_matches('/'));
    // The leaf goes where the controllers reach a child: the delegated
    // directory, or its parent where the directory holds a process (the
    // scope of a shell under systemd): cgroup v2 gives no controller to
    // the child of a cgroup with a process.
    let mut last = String::new();
    let mut target = None;
    for dir in [Some(path.as_path()), path.parent()].into_iter().flatten() {
        if !dir.starts_with(cgroup::ROOT) || dir == Path::new(cgroup::ROOT) {
            break;
        }
        // ⛔ Not by the controllers alone: `pids` and `cpu` enable on a
        // cgroup that holds a process, and the move into its child then
        // fails with EOPNOTSUPP (run 43726504386, clause 2). podvm's own
        // cgroup holds a process (podvm); `cgroup.procs` names the others
        // that this PID namespace can see.
        if dir == home || holds_process(dir) {
            last = format!("{} holds a process", dir.display());
            continue;
        }
        match give_controllers(dir, &needed) {
            Ok(()) => {
                target = Some(dir.to_path_buf());
                break;
            }
            Err(e) => last = e,
        }
    }
    let Some(target) = target else {
        return Err(refuse(verb, &last));
    };
    sweep(&target);
    if let Ok(mut p) = PLAN.lock() {
        *p = Some((target, home, files));
    }
    Ok(())
}

/// Does the cgroup `dir` hold a process that this process can see?
fn holds_process(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("cgroup.procs")).is_ok_and(|s| !s.trim().is_empty())
}

/// Right before the payload (or its launcher) is forked: the leaf of the
/// run, its limits, and podvm in it. Nothing where [`limits`] planned none.
pub(super) fn apply(verb: &str) -> Result<(), i32> {
    let Some((path, home, files)) = PLAN.lock().ok().and_then(|mut p| p.take()) else {
        return Ok(());
    };
    let id = podvm_supervise::new_id();
    let leaf = path.join(format!("podvm-{}", id.get(..12).unwrap_or(&id)));
    let made = std::fs::create_dir(&leaf).and_then(|()| {
        for (file, value) in &files {
            // A kernel with no swap accounting has no memory.swap.max.
            if *file == "memory.swap.max" && !leaf.join(file).exists() {
                continue;
            }
            std::fs::write(leaf.join(file), value)?;
        }
        std::fs::write(leaf.join("cgroup.procs"), "0")
    });
    if let Err(e) = made {
        let _ = std::fs::remove_dir(&leaf);
        return Err(refuse(verb, &format!("{}: {e}", leaf.display())));
    }
    if let Ok(mut l) = LEAF.lock() {
        *l = Some((leaf, home, std::process::id()));
    }
    // SAFETY: a handler with no arguments, for the life of the process.
    unsafe { atexit(release) };
    Ok(())
}

/// Right after the payload is spawned: podvm leaves the leaf, so that its
/// own threads and memory do not count against the limits of the payload
/// (`pids.max` counts threads). The payload and each of its children stay.
pub(crate) fn leave() {
    let Some((_, home, pid)) = LEAF.lock().ok().and_then(|l| l.clone()) else {
        return;
    };
    if pid == std::process::id() {
        let _ = std::fs::write(home.join("cgroup.procs"), "0");
    }
}

/// TODO/supervise.md T-1969: the OOM kills of the leaf of this run, for
/// the run report; `None` where the run has no leaf.
pub fn oom_kills() -> Option<u64> {
    let leaf = LEAF
        .lock()
        .ok()
        .and_then(|l| l.as_ref().map(|l| l.0.clone()));
    match leaf {
        Some(leaf) => podvm_supervise::report::oom_kills_of(&leaf),
        None => GUEST_OOM.get().copied(),
    }
}

/// The OOM kills that a machine guest gave with its exit marker
/// (TODO/machine.md T-1969): the count of the guest's kernel.
static GUEST_OOM: std::sync::OnceLock<u64> = std::sync::OnceLock::new();

/// Keep the OOM kills of the guest of this run for its report.
pub fn set_guest_oom_kills(n: u64) {
    let _ = GUEST_OOM.set(n);
}

/// At podvm's exit: name an OOM kill, move back, and remove the leaf.
extern "C" fn release() {
    let Some((leaf, home, pid)) = LEAF.lock().ok().and_then(|l| l.clone()) else {
        return;
    };
    // A child of fork inherits the handler: only the process of `hold`.
    if pid != std::process::id() {
        return;
    }
    let events = std::fs::read_to_string(leaf.join("memory.events")).unwrap_or_default();
    if let Some(n) = podvm_supervise::report::oom_kills_in(&events).filter(|n| *n > 0) {
        eprintln!(
            "podvm: the run passed its --memory: the OOM killer of the kernel ended {n} \
             process(es) of it ({}: oom_kill {n}) (TODO/cli.md T-1747)",
            leaf.join("memory.events").display()
        );
    }
    let _ = std::fs::write(home.join("cgroup.procs"), "0");
    let _ = std::fs::remove_dir(&leaf);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limits_become_the_files_of_the_leaf() {
        let l = |f: &str, v: &str| {
            let (k, val) = crate::run::parse::limits::incus_limit("run", f, v).unwrap();
            (f.to_string(), k, val)
        };
        let got = files(&[
            l("--memory", "512m"),
            l("--cpus", "2"),
            l("--pids-limit", "64"),
        ]);
        assert_eq!(
            got,
            [
                ("memory.max", "536870912".to_string()),
                ("cpu.max", "200000 100000".to_string()),
                ("pids.max", "64".to_string()),
                ("memory.swap.max", "536870912".to_string()),
            ]
        );
        assert_eq!(
            files(&[l("--cpus", "1.5")]),
            [("cpu.max", "150000 100000".to_string())]
        );
    }
}
