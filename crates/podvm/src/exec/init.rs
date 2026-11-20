//! TODO/enter.md T-2002: `exec` into a running container of the namespace
//! rung whose PID 1 is the init of its image (`run -d --podvm-init=image`).
//! The command joins the namespaces of the init ([`crate::initns::enter`]):
//! it sees the units, the bus and the processes of the container, where a
//! fresh entry of the tree would see none of them.

use super::Opts;

/// The exit code of the `exec`, or `None` where the container is not such
/// a run, and `exec` goes on as before.
pub(super) fn exec_if(
    target: &str,
    rootfs: &str,
    state: podvm_supervise::table::State,
    o: &Opts,
    store: &podvm_image::Store,
) -> Option<i32> {
    let c = podvm_supervise::get(store, target).ok()?;
    let supervisor = c.pid.and_then(|p| i32::try_from(p).ok())?;
    if !(super::joins_the_run(state, &c.rung, c.pid)
        && podvm_supervise::launcher::init_of_image(&c))
    {
        return None;
    }
    // The user namespace of the supervisor: podvm starts again in it and
    // comes back here (TODO/enter.md T-1910).
    crate::userns::rejoin(c.pid);
    if !crate::userns::joined() {
        eprintln!(
            "podvm exec: --podvm-init=image: podvm could not start again in the user namespace \
             of the run (TODO/enter.md T-2002)"
        );
        return Some(podvm_image::error::EXIT_RUNTIME_ERROR);
    }
    // The environment of the container, then each `-e` of `exec`, as
    // docker's `exec` (TODO/cli.md T-1765).
    let env = podvm_enter::Plan::env_for(&podvm_enter::NAMES, &c.env, &o.env);
    let ids = match o.user.as_deref() {
        Some(spec) => match crate::lifecycle::resolve_user("exec", rootfs, spec) {
            Ok(ids) => Some(ids),
            Err(code) => return Some(code),
        },
        None => c
            .env
            .iter()
            .find_map(|e| {
                e.strip_prefix(crate::interpose::IDENTITY_VAR)?
                    .strip_prefix('=')
            })
            .and_then(|v| v.split_once(':'))
            .and_then(|(u, g)| Some((u.parse().ok()?, g.parse().ok()?))),
    };
    let workdir = (o.workdir.clone()).unwrap_or_else(|| super::workdir_of(&c.working_dir));
    let state =
        podvm_supervise::table::dir(store, &c.id).join(podvm_supervise::launcher::INIT_STATE);
    Some(crate::initns::enter::exec(
        &state,
        supervisor,
        &crate::initns::enter::Ask {
            command: &o.command,
            env: &env,
            workdir: &workdir,
            ids,
            rootfs,
            tty: o.tty,
        },
    ))
}
