//! The fields of a container for `inspect` and `ps` (moved out of
//! `copy_tree.rs`, TODO/cli.md T-2070, which keeps the tree copy).

use super::*;

/// `podvm inspect` against a container rather than an image.
///
/// ⚠ Tried after the image lookup fails, so a name that is both is an image, as
/// docker resolves it.
/// PodUtils/podvm#514: the document of the container `want` (its line of
/// `template`, or its JSON object), for the caller to print with the others
/// in one array; `None` where no container has the name.
pub fn inspect_container(want: &str, template: Option<&str>) -> Option<Result<String, i32>> {
    let s = podvm_image::open_store().ok()?;
    // TODO/machine.md T-2042: a machine reads as a container (`target`).
    let (c, fields) = match crate::windows::machine::find(want) {
        Some(r) => super::target::fields(&s, &r),
        None => {
            let c = podvm_supervise::get(&s, want).ok()?;
            let f = container_fields(&s, &c);
            (c, f)
        }
    };
    let typed = super::typed::fields(&fields, &c);
    Some(match template {
        Some(t) => format::render_typed(t, &typed, &super::typed::plain(&c)).map_err(|bad| {
            eprintln!("podvm inspect: {bad}");
            EXIT_CLI_ERROR
        }),
        None => Ok(super::typed::document(&typed).to_string()),
    })
}

/// One field of the end of the last run, or a dash where no run ended.
fn end_field(c: &Container, f: impl Fn(&podvm_supervise::End) -> String) -> String {
    c.end.as_ref().map(f).unwrap_or_else(|| "-".into())
}

/// The word of an end reason, as the record writes it.
pub(super) fn reason_word(r: podvm_supervise::Reason) -> &'static str {
    match r {
        podvm_supervise::Reason::Exit => "exit",
        podvm_supervise::Reason::Signal => "signal",
        podvm_supervise::Reason::Wall => "wall",
    }
}

/// The count of the processes that stayed after the kill of the tree, or
/// `unknown`, never a zero for an unknown count.
pub(super) fn residual_word(r: podvm_supervise::Residual) -> String {
    match r {
        podvm_supervise::Residual::Count(n) => n.to_string(),
        podvm_supervise::Residual::Unknown => "unknown".to_string(),
    }
}

pub(super) fn container_fields(
    s: &podvm_image::Store,
    c: &Container,
) -> Vec<(&'static str, String)> {
    // T-0708: the emulation tally beside the container record. A dash where
    // the tier never ran (no memo file); counts where it did, zero included.
    let memo = podvm_supervise::table::memo_path(s, &c.id);
    let em = memo
        .exists()
        .then(|| podvm_supervise::table::emulated_counts(&memo));
    let count = |f: fn(&podvm_supervise::table::EmulatedCounts) -> u64| {
        em.as_ref()
            .map(f)
            .map(|x| x.to_string())
            .unwrap_or_else(|| "-".into())
    };
    let mknod = count(|e| e.mknod);
    let mount = count(|e| e.mount);
    let unshare = count(|e| e.unshare);
    let clone = count(|e| e.clone);
    // T-0413: the served /proc answers beside the four older stories.
    let procfs = count(|e| e.procfs);
    vec![
        ("Id", c.id.clone()),
        ("Name", c.name.clone()),
        ("Image", c.image.clone()),
        ("State", c.state.word().to_string()),
        // TODO/cli.md T-2001 OP-06: docker's `.State.*` names. `.State`
        // alone stays the word; the JSON nests these as docker does.
        ("State.Status", c.state.word().to_string()),
        (
            "State.Running",
            matches!(c.state, State::Running).to_string(),
        ),
        (
            "State.ExitCode",
            c.exit_code
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".into()),
        ),
        (
            "State.Pid",
            c.pid.map(|p| p.to_string()).unwrap_or_else(|| "0".into()),
        ),
        (
            "State.StartedAt",
            c.started_at.clone().unwrap_or_else(|| "-".into()),
        ),
        (
            "State.FinishedAt",
            c.finished_at.clone().unwrap_or_else(|| "-".into()),
        ),
        ("Status", c.status()),
        (
            "Pid",
            c.pid.map(|p| p.to_string()).unwrap_or_else(|| "0".into()),
        ),
        (
            "LauncherPid",
            c.launcher_pid
                .map(|p| p.to_string())
                .unwrap_or_else(|| "0".into()),
        ),
        // ⛔ A dash where there is no code, never a zero. AGENTS.md
        // absolute 3, and `wait` refuses for the same reason.
        (
            "ExitCode",
            c.exit_code
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".into()),
        ),
        ("Created", c.created_at.clone()),
        (
            "StartedAt",
            c.started_at.clone().unwrap_or_else(|| "-".into()),
        ),
        (
            "FinishedAt",
            c.finished_at.clone().unwrap_or_else(|| "-".into()),
        ),
        ("RootfsPath", c.rootfs.clone()),
        (
            "LogPath",
            podvm_supervise::table::log_path(s, &c.id)
                .display()
                .to_string(),
        ),
        ("Rung", c.rung.clone()),
        // TODO/enter.md T-1935: the tier on the rung, a dash for none.
        (
            "Tier",
            match super::syscall::cap_of(&c.env) {
                Some(_) => super::syscall::WORD.to_string(),
                None => "-".to_string(),
            },
        ),
        // TODO/cli.md T-1695: the labels of the run, KEY=VALUE, one comma
        // between two.
        ("Labels", c.hardening.labels.join(",")),
        // ⭐ T-0804 rule 3. The mode a caller reads includes the shims, or the
        // report says devices exist when they do not.
        ("Complete.Fixups", c.completion_note()),
        ("Complete.Degraded", c.completion_degraded.to_string()),
        ("Command", c.argv.join(" ")),
        // T-2060: docker's `.Config`. The strings are the flat text; the
        // typed values come from `typed::fields`.
        ("Config", String::new()),
        ("Config.Env", c.env.join(" ")),
        ("Config.Cmd", c.argv.join(" ")),
        ("Config.Image", c.image.clone()),
        ("Config.WorkingDir", c.working_dir.clone()),
        ("Config.Labels", c.hardening.labels.join(",")),
        // TODO/machine.md T-1968: each `--tmpfs` with a limit, as memory.
        ("Tmpfs", crate::run::tmpfs::words(&c.env).join(",")),
        // TODO/cli.md T-2053 (OQ-07): what `exec` into the running
        // container shares, from what its entry opened.
        ("Exec.Mode", exec_opened(c).exec_mode(&c.rung).to_string()),
        ("Exec.Shares", exec_opened(c).exec_shares(&c.rung)),
        // ⛔ SAID, NOT IMPLIED. TODO/supervise.md T-1708: the launcher is the
        // child subreaper of the payload's tree and kills each descendant at
        // the end. podvm has no PID namespace, so a process that reparents
        // above the launcher stays out of reach, and a caller reads this
        // rather than assuming docker's containment.
        (
            "Contains",
            "the payload's tree: the launcher is its child subreaper and kills each \
             descendant at the end; a process that reparents above the launcher is \
             outside its reach"
                .to_string(),
        ),
        ("Noticed", c.noticed.clone().unwrap_or_else(|| "-".into())),
        // TODO/supervise.md T-1708: the end of the last run, as the launcher
        // measured it. A dash where no run ended, never a zero.
        (
            "End.Reason",
            end_field(c, |e| reason_word(e.reason).to_string()),
        ),
        ("End.RunMs", end_field(c, |e| e.run_ms.to_string())),
        ("End.UserMs", end_field(c, |e| e.user_ms.to_string())),
        ("End.SystemMs", end_field(c, |e| e.system_ms.to_string())),
        ("End.MaxRssKiB", end_field(c, |e| e.max_rss_kib.to_string())),
        (
            "End.Residual",
            end_field(c, |e| residual_word(e.residual_processes)),
        ),
        // T-0708: how many times the preloaded tier emulated an operation
        // rather than running the kernel's answer. A dash where the tier
        // never ran; `inspect --format` reads these names, `ps` does not.
        // T-0413 adds the served /proc answers behind the same memo.
        ("Interpose.Emulated.mknod", mknod),
        ("Interpose.Emulated.mount", mount),
        ("Interpose.Emulated.unshare", unshare),
        ("Interpose.Emulated.clone", clone),
        ("Interpose.Emulated.procfs", procfs),
    ]
}
