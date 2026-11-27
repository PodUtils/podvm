use super::*;
use podvm_enter::ExitCode as _;

pub(super) use super::ns::serve_devices; // T-0501

/// Enter an existing rootfs. ⭐ ONE PATH for a container and for an image: the
/// only difference is where the rootfs came from, and duplicating the entry
/// would be a second implementation of the thing this verb exists to be.
pub(super) fn enter(
    target: &str,
    rootfs: &str,
    state: podvm_supervise::table::State,
    o: &Opts,
    store: &podvm_image::Store,
) -> i32 {
    use podvm_supervise::table::State;
    if state == State::Created {
        eprintln!("{}", super::ns::created_note(target));
    }
    // ⭐ T-0505's record, read once: the memo below, the device spec, the
    // container's own image name for the workdir refusal, and the image
    // record the entry decision reads the architecture from.
    let container = match podvm_supervise::get(store, target) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("podvm exec: {e}");
            return podvm_image::error::EXIT_RUNTIME_ERROR;
        }
    };
    // TODO/enter.md T-1910: a run in a user namespace of podvm's own.
    if state == State::Running && container.rung == "namespace" {
        crate::userns::rejoin(container.pid);
    }
    // TODO/cli.md T-1765: the environment of the container, as docker's
    // `exec` has it: the image's and the `-e` of `run`, then the `-e` of
    // `exec`. podvm's own variables of the run (`PODVM_*`, its
    // `LD_PRELOAD`) are made again for this entry, and not inherited. The
    // processes still share only the filesystem (T-0505).
    let inherited: Vec<String> = container
        .env
        .iter()
        // T-1908: the name of `--hostname` stays the container's.
        .filter(|e| {
            (e.starts_with("PODVM_HOSTNAME=") || !e.starts_with("PODVM_"))
                && !e.starts_with("LD_PRELOAD=")
        })
        .cloned()
        .collect();
    let mut env = podvm_enter::Plan::env_for(&podvm_enter::NAMES, &inherited, &o.env);
    // ⭐ T-0711: the requested identity, resolved against the re-entered
    // rootfs exactly as `run` resolves it against the extracted one.
    if let Err(code) = crate::lifecycle::apply_user("exec", rootfs, o.user.as_ref(), &mut env) {
        return code;
    }
    // ⭐ T-0710: the container's host memo, re-handed on this fresh re-entry.
    // A fresh chroot shares only the filesystem; without the same descriptor
    // a second process answers `stat` with the real uid and contradicts the
    // first, so an entry that cannot be handed one refuses.
    let memo_path = podvm_supervise::table::memo_path(store, &container.id);
    if let Err(e) = podvm_supervise::table::hand_memo(&mut env, &memo_path) {
        eprintln!("podvm exec: the ownership memo could not be created: {e}");
        return podvm_image::error::EXIT_RUNTIME_ERROR;
    } // T-1928: the descriptor and the identity of the memo.
      // ⭐ TODO/enter.md T-0501. The container's device spec is
      // configuration, not environment: the spec rides `PODVM_*`, which the
      // re-entry does not inherit, but the devices are the container's
      // (docker's `exec` sees them), so the spec is read from the record
      // and opened fresh here rather than inherited. A spec gone bad
      // since `create` refuses the exec naming it.
    let exec_devices = match podvm_enter::device::specs_of(&container.env) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("podvm exec: {e}");
            return e.exit_code();
        }
    };
    podvm_enter::device::push_spec(&mut env, &exec_devices);
    // ⭐ TODO/enter.md T-1653. The container's volumes are configuration
    // too: docker's `exec` sees the mounts of the container, so the pairs
    // come from the table in the record, and the table is built again. The
    // payload and the working directory resolve through it.
    let mut volumes = podvm_enter::userland::volumes_of(&container.env);
    // TODO/cli.md T-1830: a `--tmpfs` directory ends with its run. After
    // that end the record still names it, and `exec` sees the tree of the
    // image there; during the run it sees the directory. The device pairs
    // of the record are not the caller's volumes: `maps` adds them again.
    let tmpfs = store.root().join("tmpfs");
    volumes.retain(|v| !crate::lifecycle::volume::is_device_pair(v));
    volumes.retain(|v| {
        !std::path::Path::new(&v.host).starts_with(&tmpfs) || std::path::Path::new(&v.host).is_dir()
    });
    let maps = match crate::lifecycle::volume::maps("exec", rootfs, &volumes) {
        Ok(m) => m,
        Err(code) => return code,
    };
    // OQ-05 (T-1970): docker's `exec` starts in the container's directory.
    let working_dir =
        (o.workdir.clone()).unwrap_or_else(|| super::workdir_of(&container.working_dir));
    // TODO/cli.md T-1850: the cached probe of `run` (T-1746).
    let findings = crate::run::probe_cache::findings("exec", store);
    // ⭐ TODO/enter.md T-1410. The image record behind the container, read
    // for the architecture the entry decision needs: a refusal here names
    // the lookup, because without it no rung can be decided safely.
    let record = match store.find_one(&container.image) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("podvm exec: {target}: the image record cannot be read: {e}");
            return podvm_image::error::EXIT_RUNTIME_ERROR;
        }
    };
    // ⭐ TODO/milestones.md T-1112, as on the image path below and in
    // `run`'s `prepare`: the OS gate sits on every entry path.
    if let Err(code) =
        crate::lifecycle::ensure_linux_guest("exec", &record.os, &record.architecture)
    {
        return code;
    }
    let host = Platform::host();
    let support_native = matches!(
        podvm_enter::binfmt::support_for(&record.architecture, &host.arch),
        podvm_enter::binfmt::Support::Native
    );
    // ⭐ TODO/enter.md T-1410. `exec` and `run` share the entry decision:
    // where chroot is denied the loader and memfd families drive, with
    // T-0505's fresh-entry contract unchanged (separate banners, separate
    // records). A refusal names the blocking call.
    // PodUtils/podrunner#326: a running container of the namespace rung is
    // entered by a join of its namespaces (below, T-1765, T-1910), where
    // its volumes are binds already. The probe of the host decides nothing
    // there: on a host whose probe denies chroot, the decision gave the
    // entry with no chroot, which refused each volume of the container.
    let userland = if joins_the_run(state, &container.rung, container.pid) {
        None
    } else {
        if let Err(code) = super::ns::layers_need_join(&container.env) {
            return code; // TODO/enter.md T-2101
        }
        match crate::lifecycle::syscall::exec_entry(
            &container.env,
            &maps,
            &o.command,
            &working_dir,
            &mut env,
            support_native,
            &findings,
            o.unsafe_host_paths,
        ) {
            Ok(u) => u,
            Err(code) => return code,
        }
    };
    if let Err(code) =
        crate::lifecycle::volume::refuse_unserved("exec", (&volumes, &[]), userland.as_ref(), None)
    {
        return code;
    }
    // ⭐ TODO/cli.md T-1726: the read-only table of the container's run
    // (`PODVM_RO`) is configuration too: each exec takes it, or refuses by
    // name where its entry has no read-only layer. Before it, an exec wrote
    // under a `:ro` volume (PodUtils/podrunner#62).
    if let Err(code) = crate::lifecycle::volume::container_read_only(
        "exec",
        &container.env,
        userland.as_ref(),
        &mut env,
    ) {
        return code;
    }
    // ⭐ T-0702 and T-0706, as in `run`: a fresh chroot re-entry is a fresh
    // payload, so it is classified and placed again rather than inheriting
    // the first entry's answer. After the decision, so a refusal writes
    // nothing; T-1649: a `#!` payload on the userland rung classifies its
    // interpreter.
    let gargv = userland.as_ref().map_or(&o.command, |u| &u.guest_argv);
    let mut interpose_note = String::new();
    if let Err(e) = crate::interpose::apply(
        "exec",
        &maps,
        &podvm_enter::program_in(gargv.first().map_or("", String::as_str), &working_dir),
        &mut env,
        &mut interpose_note,
    ) {
        eprintln!("{e}");
        return podvm_image::error::EXIT_RUNTIME_ERROR;
    }
    // ⭐ The loader transform, mirroring `run`'s `prepare`: without a
    // chroot the loader resolves guest paths on the host root, so the
    // preload becomes the host path and the libraries come from the
    // image. A caller preload naming guest paths is dropped and named.
    let mut dropped_preload: Option<String> = None;
    if let Some(u) = &userland
        && u.family == podvm_enter::userland::Family::Loader
    {
        let (hosted, dropped) = podvm_enter::userland::host_env(
            rootfs,
            crate::interpose::GUEST_PATH,
            &env,
            &u.lib_dirs,
        );
        env = hosted;
        dropped_preload = dropped;
    }
    let path_dirs = podvm_enter::Plan::path_from(&env);
    // ⭐ TODO/complete.md T-0413: the re-entered payload's guest path, as in
    // `run`'s `prepare`. A fresh chroot re-entry is a fresh payload.
    crate::run::push_guest_exe(
        &mut env,
        &maps,
        &podvm_enter::program_in(
            gargv.first().map(String::as_str).unwrap_or(""),
            &working_dir,
        ),
        &path_dirs,
    );
    // ⭐ TODO/enter.md T-1407 and T-1409, as in `run`'s `prepare`: the
    // root-anchoring table (or the explicit opt-out) and the loader
    // exe-force join here, banner-named below.
    let mut userland_note = String::new();
    // ⭐ T-1415, as in `run`'s `prepare`: the gate below reads what
    // `userland_env` decided, not the flag beside it.
    let mut strict_unsafe = false;
    if let Some(u) = &userland {
        match crate::lifecycle::userland_env(
            "exec",
            &maps,
            env,
            u.family,
            o.unsafe_host_paths,
            &mut std::io::stderr(),
        ) {
            Ok(ue) => {
                env = ue.env;
                userland_note = ue.banner;
                strict_unsafe = ue.unsafe_paths;
            }
            Err(code) => return code,
        }
    }
    // ⭐ T-1412: the workdir is checked before the fork and any fixup.
    if !super::in_bind(&container.env, &working_dir) // OR-04 (T-1970): a bind holds it
        && let Err(e) = podvm_enter::check_workdir(&maps, &working_dir, &container.image)
    {
        eprintln!("podvm exec: {e}");
        return e.exit_code();
    }
    let selection = podvm_probe::select::Selection::choose(&findings);
    // ⭐ T-0804 rule 4, as in `run`'s `prepare`: the banner is built from
    // the rung `exec` ENTERS with, chrooted or userland.
    let entered = userland
        .as_ref()
        .map(|_| podvm_probe::select::Rung::Userland)
        .unwrap_or(podvm_enter::ENTERED_RUNG);
    let mut banner = podvm_probe::report::entry_banner(&findings, &selection, entered);
    if let Some(note) = crate::names::alias_note() {
        banner.push_str(&note);
    }
    let joins = joins_the_run(state, &container.rung, container.pid);
    banner.push_str(&super::exec_line(joins, &container)); // T-2053
    banner.push_str(&interpose_note);
    // ⭐ TODO/enter.md T-0501. The re-entry serves the container's
    // mappings, so it names them like the entry did.
    for m in &exec_devices {
        banner.push_str(&podvm_enter::device::banner_line(m));
    }
    // ⭐ TODO/enter.md T-1317, T-1407 and T-1409: the family account and
    // the anchoring beside it, as in `run`'s `prepare`.
    if let Some(u) = &userland {
        banner.push_str(&u.banner);
        banner.push('\n');
        if let Some(d) = &dropped_preload {
            banner.push_str(&format!(
                "podvm: userland: caller LD_PRELOAD {d:?} dropped: it names guest \
                 paths, which resolve on the host without a chroot\n"
            ));
        }
        banner.push_str(&userland_note);
    }
    // ⭐ M5. `exec` completes the rootfs exactly as `run` does, and for the same
    // reason: a fresh chroot re-entry is a fresh payload, and the `/dev/null` a
    // previous one turned into a file is still a file.
    let mut ask = o.ask.clone();
    super::ns::hosts_of(&mut ask, target, &container.of.hardening); // T-2075
    let mut completion = match crate::complete::prepare("exec", rootfs, &ask, &mut banner) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let mut err = std::io::stderr().lock();
    // ⭐ T-0804 rule 1, and ahead of every refusal below for `run`'s own
    // reasons: a refused caller gets the banner too, and T-0412's steps have to
    // be named before they run.
    // ⭐ TODO/cli.md T-1416: quiet by default, the banner under
    // `--verbose`; a refusal still prints, because it never passes
    // through here.
    let loud = crate::complete::banner_loud(store, &ask);
    let quiet = !loud;
    if loud {
        let _ = write!(err, "{banner}");
    }
    if let Err(code) = crate::complete::strict_refusal(
        "exec",
        &ask,
        entered.word(),
        &completion,
        &mut err,
        &crate::complete::StrictCtx {
            chroot_usable: podvm_probe::probes::chroot_usable(&findings),
            unsafe_host_paths: strict_unsafe,
            preload_dropped: dropped_preload.is_some(),
            gaps: crate::lifecycle::gaps_of(userland.as_ref()),
        },
    ) {
        return code;
    }
    // T-2043: a pty as `run -t` gives (of the devpts of a joined run), or a
    // refusal by name (T-0503).
    // podcrates@T-1113: the devpts of the payload where the init does not
    // open to this process.
    let joined = joins_the_run(state, &container.rung, container.pid)
        .then_some(container.pid)
        .flatten()
        .map(super::ns::pid_of_run);
    let ptmx = podvm_probe::probes::ptmx_usable(&findings);
    let term = match super::tty::open(o.tty, ptmx, joined, &mut err) {
        Ok(t) => t,
        Err(c) => return c,
    };
    // ⭐ T-0412, and `exec` runs them for the same reason it completes the
    // rootfs at all: a fresh chroot re-entry is a fresh payload, and a keyring
    // or a CA index a previous one destroyed is still destroyed.
    // ⭐ T-0710: steps share the container's memo, opened once for the steps
    // and again for the payload below.
    {
        use std::os::fd::AsRawFd;
        let memo_for_steps = podvm_supervise::table::open_memo(&memo_path).ok();
        let memo_host = memo_for_steps.as_ref().map(|f| f.as_raw_fd() as i64);
        crate::complete::run_steps_with_memo(
            "exec",
            rootfs,
            &env,
            &mut completion,
            quiet,
            &mut err,
            memo_host,
        );
    }
    let memo = match podvm_supervise::table::open_memo(&memo_path) {
        Ok(f) => f,
        Err(e) => {
            let _ = writeln!(
                err,
                "podvm exec: the ownership memo could not be opened: {e}"
            );
            return podvm_image::error::EXIT_RUNTIME_ERROR;
        }
    };
    // TODO/enter.md T-1658: the confine layer of the container's run. A
    // rung with no chroot refuses it by name, as `run` does.
    let confine = match container.of.hardening.confine.as_deref() {
        None => None,
        Some(spec) => {
            let ask = podvm_enter::confine::parse(spec).ok();
            match crate::lifecycle::confine::decide(
                "exec",
                ask.as_ref(),
                entered,
                rootfs,
                userland.as_ref(),
                &mut env,
            ) {
                Ok(c) if ask.is_some() => c,
                Ok(_) => {
                    let _ = writeln!(
                        err,
                        "podvm exec: the confine layer of the record ({spec:?}) does not parse \
                         (TODO/enter.md T-1658)"
                    );
                    return podvm_image::error::EXIT_RUNTIME_ERROR;
                }
                Err(code) => return code,
            }
        }
    };
    use std::os::fd::AsRawFd;
    let memo_host = memo.as_raw_fd() as i64;
    let mut pass = vec![(podvm_supervise::table::MEMO_CHILD_FD, memo_host)];
    pass.extend(
        term.as_ref()
            .map(super::tty::Term::pass)
            .into_iter()
            .flatten(),
    );
    let mut plan = Plan {
        argv: o.command.clone(),
        env: super::host_paths_out(env, userland.is_none()), // T-2054
        working_dir,
        fds: Fds { pass },
        // ⚠ Empty: the banner was printed above, before the steps.
        banner: String::new(),
        path_dirs,
        names: podvm_enter::NAMES,
        limits: Vec::new(),
        no_new_privs: false,
        die_with_parent: false,
        confine,
        hostname: None, // T-1908: `exec` joins the UTS namespace of the run
        // T-1920, OR-04; T-1830: the `--tmpfs` directory of an ended run is
        // gone, so its bind stays out, as in the table above.
        volumes: podvm_supervise::launcher::binds_of(&container.env)
            .into_iter()
            .filter(|b| {
                let h = std::path::Path::new(&b.host);
                !h.starts_with(&tmpfs) || h.is_dir()
            })
            .collect(),
        userns_ids: podvm_enter::lock::payload_ids(None), // T-2105: a fresh entry nests
        pid_namespace: false, // podcrates `v0.65.0`; T-1930 sets it for `run -d`
        // T-2054: the relay of a join of the run (PodUtils/podvm#646).
        relay_title: Some(podvm_enter::TITLE_EXEC.into()),
        tmpfs: Vec::new(), // podcrates `v0.86.0` (T-2102)
    };
    let sealed = |p: &Plan| super::ns::seal_for_join(p, &container.env, rootfs); // T-2108
    if let Err(c) = serve_devices(&mut plan).and_then(|()| sealed(&plan)) {
        return c;
    }
    let root = match RootDir::open(rootfs) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(err, "podvm exec: {e}");
            return e.exit_code();
        }
    };
    // ⭐ TODO/enter.md T-1410: the shared entry decision drives here. The
    // container's memo is its own: nothing ephemeral is removed on a way out.
    super::tty::proc_of();
    let relay = super::tty::start(term, o.interactive);
    let code = match &userland {
        Some(u) => {
            let outcome = crate::lifecycle::enter_userland_with(
                "exec",
                &root,
                &plan,
                u,
                &mut err,
                &mut |p, _| super::tty::on_spawn(p),
            );
            drop(memo);
            outcome.code
        }
        None => {
            // TODO/cli.md T-1765: `exec` joins the mount namespace of a running
            // run of the namespace rung; pod-enter names a refused join.
            let joined = joins_the_run(state, &container.rung, container.pid)
                .then_some(container.pid)
                .flatten()
                .and_then(|p| crate::userns::joined_pidfd().or_else(|| super::ns::pidfd_of_run(p)));
            let wait = |c: podvm_enter::Child, err: &mut dyn Write| {
                super::tty::on_spawn(c.pid); // T-2043: the interrupt key
                podvm_enter::wait_forwarding(&c, err)
            };
            let r = match joined {
                Some(fd) => {
                    super::ns::join_uts(&fd); // TODO/enter.md T-1908
                    let spawned = super::ns::spawn_joined(&root, &plan, &fd, &mut err);
                    let r = spawned.and_then(|c| wait(c, &mut err));
                    drop(fd);
                    r
                }
                None => podvm_enter::spawn(&root, &plan, &mut err).and_then(|c| wait(c, &mut err)),
            };
            match r {
                Ok(c) => c,
                Err(e) => {
                    let _ = writeln!(err, "podvm exec: {e}");
                    e.exit_code()
                }
            }
        }
    };
    super::tty::finish(relay, &mut err);
    code
}

pub(crate) use super::ns::{join_mounts, joins_the_run};
