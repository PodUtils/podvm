//! The entry of a detached payload: what podvm's launcher does between its
//! lock and its pidfd.
//!
//! [`TODO/supervise.md`](../../../TODO/supervise.md) T-0601, T-0602, T-0603,
//! T-0605.
//!
//! ⭐ **The launcher is `pod_supervise::launcher` (PodUtils/podcrates).** It
//! double-forks, takes the container's lock, binds the control socket, puts
//! its own stdio on `/dev/null`, opens the payload's stdio on the log, holds
//! the pidfd, answers `SIG` and `WAIT`, and writes `running` and `exited`.
//! [`enter`] is podvm's part, run in that launcher before the root changes:
//! the ownership memo, the devices, the image hold, the plan, and the rung.
//!
//! ⛔ **Nothing here spawns a thread.** `PR_SET_PDEATHSIG` fires on the
//! creating THREAD's exit, and this runs on the path that clones, chroots and
//! execs (`nothing_on_the_spawn_path_can_spawn_a_thread`).

use std::os::fd::AsRawFd;

use pod_supervise::launcher::{Launch, Payload};
use podvm_enter::{Fds, Plan, RootDir};
use podvm_image::Store;

use crate::table::{self, Container};

/// What the launcher keeps open while the payload runs, and drops after it.
struct Kept {
    // TODO/cli.md T-1825: first, so that it drops first, with the lock of
    // the image still held.
    _report: Option<Finish>,
    _memo: std::fs::File,
    _held: podvm_image::store::Lock,
    _root: RootDir,
    _child: podvm_enter::Child,
    // TODO/enter.md T-2002: after the end of each process of the run.
    _init: Option<Box<dyn std::any::Any>>,
    // TODO/cli.md T-2074: last, after each descriptor of the tree closed.
    _discard: Option<Discard>,
}

/// TODO/cli.md T-2074 (OR-B38-04): `--rm` of `run -d`. `pod_supervise`
/// removes the record and the directory of the container before it drops
/// `Payload::keep`; the drop puts the tree of the container aside, as
/// `podvm rm` does (`crate::remove`). A record that stays (the removal
/// failed) keeps its tree.
struct Discard {
    store: std::path::PathBuf,
    id: String,
    rootfs: String,
}

impl Drop for Discard {
    fn drop(&mut self) {
        let Ok(s) = Store::open(&self.store) else {
            return;
        };
        if crate::get(&s, &self.id).is_err_and(|e| e.is_callers()) {
            let _ = podvm_extract::tree::discard(&s, &self.rootfs);
        }
    }
}

/// TODO/cli.md T-1825: `pod_supervise` drops `Payload::keep` after it
/// wrote the end into the record, so the drop writes that end into the
/// `--report` of the detached run.
struct Finish {
    store: std::path::PathBuf,
    id: String,
    report: std::path::PathBuf,
}

impl Drop for Finish {
    fn drop(&mut self) {
        let Ok(s) = Store::open(&self.store) else {
            return;
        };
        if let Ok(c) = crate::get(&s, &self.id) {
            let _ = crate::report::finish(&self.report, &c);
        }
    }
}

/// TODO/enter.md T-2002: what the entry of a container with the init of
/// its image (`--podvm-init=image` on the namespace rung) gets.
pub struct InitJob<'a> {
    pub container: &'a Container,
    /// The descriptors of the payload, as [`Launch::pass`] gives them.
    pub pass: &'a [(i64, i64)],
    /// The file of the state of the init, beside the record.
    pub state: std::path::PathBuf,
}

/// What the entry of [`InitJob`] started.
pub struct InitStarted {
    /// The pid of the supervisor of the init, a child of the launcher.
    pub pid: i64,
    /// What the launcher drops after the end of the run (the cgroups).
    pub keep: Box<dyn std::any::Any>,
}

/// The entry of [`InitJob`]: the binary sets it at its start. It starts the
/// supervisor of the init as a child of the launcher, and waits until the
/// boot of the init is complete; an `Err` is the line of the failure, with
/// no process left.
pub static INIT_ENTRY: std::sync::OnceLock<InitEntry> = std::sync::OnceLock::new();

/// The type of [`INIT_ENTRY`].
pub type InitEntry = fn(&InitJob<'_>) -> Result<InitStarted, String>;

/// The mark of a container whose PID 1 is the init of its image.
pub const INIT_MARK: &str = "PODVM_INIT=image";
/// The name of the state file of such a container, in its directory.
pub const INIT_STATE: &str = "init.state";

/// True where the record asks for the init of the image on the namespace
/// rung: the launcher then starts no payload of pod-enter.
pub fn init_of_image(container: &Container) -> bool {
    container.rung == "namespace" && container.env.iter().any(|e| e == INIT_MARK)
}

/// podvm's entry for `pod_supervise::start`: it runs in the launcher, and
/// its `Err` text reaches the caller as the start's failure.
pub fn enter<'a>(
    store: &'a Store,
    record: &'a podvm_image::Record,
) -> impl FnOnce(&Container, Launch<'_>) -> Result<Payload, String> + 'a {
    move |container, launch| enter_in_launcher(store, record, container, launch)
}

fn enter_in_launcher(
    store: &Store,
    record: &podvm_image::Record,
    container: &Container,
    launch: Launch<'_>,
) -> Result<Payload, String> {
    let Launch { mut pass, log } = launch;
    // ⭐ T-0710: the host memo, beside this container's record. Opened here,
    // before the root changes, and dup'd to the constant child number the
    // environment names. An entry that cannot be handed one refuses rather
    // than starting with no record.
    let memo_path = table::memo_path(store, &container.id);
    let memo = table::open_memo(&memo_path)
        .map_err(|e| format!("the ownership memo could not be opened: {e}"))?;
    pass.push((table::MEMO_CHILD_FD, memo.as_raw_fd() as i64));

    // ⭐ TODO/enter.md T-0501. The container's device spec re-opens HERE,
    // in the process that enters: a descriptor cannot cross the client
    // that ran `create`, so the record carries the spec and each entry
    // opens it before its own root changes. A host path gone since
    // `create` refuses the start naming it.
    let mut env = container.env.clone();
    let opened = podvm_enter::device::specs_of(&env)
        .and_then(|s| podvm_enter::device::open_all(&s))
        .map_err(|e| format!("starting with --device: {e}"))?;
    pass.extend(opened.pass.iter().copied());
    podvm_enter::device::push_serve(&mut env, &opened);

    let root = RootDir::open(&container.rootfs).map_err(|e| e.to_string())?;
    // The stored environment already carries the constant descriptor number
    // from `create`; containers made before T-0710 do not, so ensure it here
    // rather than refusing a record that predates the rule.
    // TODO/interpose.md T-1928: and the identity of the file opened here,
    // which is the container's memo, not the one that `create` named.
    table::hand_memo_file(&mut env, &memo, &memo_path)?;
    // TODO/enter.md T-1920: the volumes that pod-enter binds on the
    // namespace rung ride in `PODVM_BINDS`; the payload does not see it.
    let binds = binds_of(&env);
    env.retain(|e| !e.starts_with("PODVM_BINDS=") && !e.starts_with("PODVM_BINDS_BASE="));
    if container.userland_exec.is_empty() {
        table::drop_host_paths(&mut env); // T-2054: an entry with a root
    }
    let mut plan = Plan {
        argv: container.argv.clone(),
        env: env.clone(),
        working_dir: container.working_dir.clone(),
        fds: Fds { pass },
        // ⚠ Empty: the banner was printed by the process the caller was
        // watching, before it forked the launcher. Printing it again here
        // would put it in the container's LOG.
        banner: String::new(),
        path_dirs: Plan::path_from(&env),
        names: podvm_enter::NAMES,
        limits: container.of.hardening.limits(),
        no_new_privs: container.of.hardening.no_new_privs,
        // podcrates v0.22.0 (COORD-046, T-205): the payload gets SIGKILL
        // when this launcher dies, so it never outlives a killed
        // supervisor. The launcher forks from its one thread.
        die_with_parent: true,
        // TODO/enter.md T-1658: the confine layer of `run -d`.
        confine: container.of.hardening.confine_spec(&container.rootfs)?,
        // TODO/enter.md T-1908: `--hostname` names a UTS namespace of the
        // namespace rung (podcrates v0.52.0, `Plan::hostname`); each other
        // entry refuses a plan with a name and holds it by its env.
        hostname: (container.rung == "namespace")
            .then(|| {
                env.iter()
                    .find_map(|e| e.strip_prefix("PODVM_HOSTNAME="))
                    .map(str::to_string)
            })
            .flatten(),
        // TODO/enter.md T-1920: pod-enter binds them on the namespace
        // rung (podcrates `v0.59.0`); the loader family keeps its table.
        volumes: if container.userland_exec.is_empty() {
            binds
        } else {
            Vec::new()
        },
        // TODO/enter.md T-1927: the ids of `--user` or the image's `USER`
        // inside the user namespace of the namespace rung.
        userns_ids: (container.rung == "namespace")
            .then(|| table::userns_ids_of(&env))
            .flatten(),
        // TODO/enter.md T-1930 (PodUtils/podrunner#200): a PID namespace
        // with its own `/proc` on the namespace rung (podcrates `v0.65.0`;
        // its init closes the descriptors of the caller since `v0.66.0`,
        // COORD-167), so that a payload sees and signals only its own
        // processes.
        pid_namespace: container.rung == "namespace" && container.userland_exec.is_empty(),
    };

    // ⭐ TODO/image.md T-0204 and T-0211. The image lock is taken here and
    // handed to the payload immediately before the fork that leads to its
    // exec, so a concurrent `rmi` or `prune` cannot delete the rootfs a
    // running container is executing out of. ⛔ In the LAUNCHER and not in
    // the caller: the caller exits as soon as the container is up.
    let held = store
        .hold(record)
        .map_err(|e| format!("holding the image: {e}"))?;
    // TODO/enter.md T-2054 (OQ-08): with a PID namespace no process of the
    // payload outlives its init, and this launcher holds the lock until the
    // init ends, so the payload gets no descriptor of the store.
    if !plan.pid_namespace {
        held.hand_to_payload()
            .map_err(|e| format!("handing the image lock to the payload: {e}"))?;
    }

    // ⭐ TODO/enter.md T-1411: where the record carries the loader drive
    // `create` decided, the launcher execs it with no chroot through
    // `spawn_userland`; else T-1339: the rung the probe selects, decided
    // where the payload is entered. The fallback line joins the container
    // log: the launcher's stderr is /dev/null.
    let mut init = None;
    let child = if init_of_image(container) {
        // TODO/enter.md T-2002: the init of the image is PID 1 of a PID
        // namespace, and the command of the record joins it after the boot.
        let entry = INIT_ENTRY
            .get()
            .ok_or("this program has no entry for the init of an image (TODO/enter.md T-2002)")?;
        let started = entry(&InitJob {
            container,
            pass: &plan.fds.pass,
            state: table::dir(store, &container.id).join(INIT_STATE),
        })?;
        init = Some(started.keep);
        podvm_enter::Child::of(started.pid)
    } else if container.userland_exec.is_empty() {
        let findings = podvm_probe::run();
        let selection = podvm_probe::select::Selection::choose(&findings);
        // TODO/enter.md T-2054 (OQ-08): the init of the PID namespace is a
        // fork of this launcher, and the payload reads its `/proc/1`: it
        // takes a title and no environment, and the launcher its own back.
        let hidden = plan
            .pid_namespace
            .then(|| podvm_enter::title::hide("podvm-init"));
        if let Some(Err(e)) = &hidden {
            // The log is the stderr of the payload: a lost line stops no run.
            writeln!(
                log,
                "podvm: the payload can read the command line and the environment of \
                 podvm in /proc/1: {e} (TODO/enter.md T-2054)"
            )
            .unwrap_or(());
        }
        let spawned = podvm_enter::spawn_selected(&root, &plan, selection.rung, log);
        if let Some(Ok(saved)) = &hidden {
            let _ = podvm_enter::title::restore(saved);
        }
        spawned.map_err(|e| e.to_string())?
    } else {
        // ⭐ TODO/enter.md T-1653: the volumes of the record join the root
        // pair, so a working directory in a volume resolves in the volume.
        let volumes = podvm_enter::userland::volumes_of(&container.env);
        let maps =
            podvm_enter::userland::Maps::new(&podvm_enter::NAMES, &container.rootfs, &volumes)
                .map_err(|why| format!("starting with -v: {why}"))?;
        // TODO/enter.md T-1658: no chroot here, so the layer takes host
        // paths, each guest path through the table.
        // The emulator of the emulate tier, as the drive names it.
        let emu = container
            .userland_exec
            .iter()
            .find_map(|a| a.strip_prefix("--podvm-dispatch-emu="));
        // TODO/enter.md T-1985: `pod-enter` writes the write table of the
        // spec (`NAMES.write_table`, podcrates `v0.80.0`).
        plan.confine = container.of.hardening.confine_host_spec(&maps, emu)?;
        // TODO/enter.md T-2013: `exec=` for the first program.
        podvm_enter::check_exec_guest(&plan, &maps, &plan.argv)
            .map_err(|e| format!("{e} (TODO/enter.md T-2013)"))?;
        podvm_enter::spawn_userland(
            &root,
            &plan,
            &maps,
            container.userland_exec.clone(),
            podvm_probe::select::Rung::Userland.word(),
            None,
            log,
        )
        .map_err(|e| e.to_string())?
    };
    let pid = child.pid;
    Ok(Payload {
        pid,
        keep: Box::new(Kept {
            _report: container.of.hardening.report.as_ref().map(|r| Finish {
                store: store.root().to_path_buf(),
                id: container.id.clone(),
                report: r.into(),
            }),
            _memo: memo,
            _held: held,
            _root: root,
            _child: child,
            _init: init,
            _discard: container.rm.then(|| Discard {
                store: store.root().to_path_buf(),
                id: container.id.clone(),
                rootfs: container.rootfs.clone(),
            }),
        }),
    })
}

#[cfg(test)]
mod tests;

/// Is `host` the directory `base` or a path under it?
fn under(host: &str, base: &str) -> bool {
    let base = base.trim_end_matches('/');
    host == base || host.strip_prefix(base).is_some_and(|r| r.starts_with('/'))
}

/// The volumes of `PODVM_BINDS` (`GUEST:HOST[:ro]`, comma separated), as
/// pod-enter binds them (TODO/enter.md T-1920).
pub fn binds_of(env: &[String]) -> Vec<podvm_enter::VolumeBind> {
    let Some(v) = env.iter().find_map(|e| e.strip_prefix("PODVM_BINDS=")) else {
        return Vec::new();
    };
    // PodUtils/podrunner#277: a source under the base opens under it with
    // no symbolic link, and pod-enter binds that descriptor.
    let base = env.iter().find_map(|e| e.strip_prefix("PODVM_BINDS_BASE="));
    v.split(',')
        .filter_map(|p| {
            let (guest, rest) = p.split_once(':')?;
            let (host, read_only) = match rest.strip_suffix(":ro") {
                Some(h) => (h, true),
                None => (rest, false),
            };
            Some(podvm_enter::VolumeBind {
                host: host.to_string(),
                guest: guest.to_string(),
                read_only,
                host_base: base.filter(|b| under(host, b)).map(str::to_string),
            })
        })
        .collect()
}
