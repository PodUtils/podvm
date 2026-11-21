//! TODO/enter.md T-2002: a foreground `run --podvm-init=image` on the
//! namespace rung. The supervisor ([`crate::initns`]) boots the init of the
//! image as PID 1 of a PID namespace and runs the command in it; this
//! process holds the record, forwards each shutdown signal, and removes
//! the cgroups of the run.

use super::*;

/// Start the supervisor, wait for it, and give the exit code of the run.
pub(super) fn run(
    p: &crate::lifecycle::Prepared,
    plan: &Plan,
    store: &podvm_image::Store,
    fg: &mut Foreground,
    err: &mut dyn Write,
) -> i32 {
    let (pid, base) = match crate::initns::spawn(&p.rootfs, plan) {
        Ok(started) => started,
        Err(e) => {
            eprintln!("podvm run: --podvm-init=image: {e} (TODO/enter.md T-2002)");
            return podvm_image::error::EXIT_RUNTIME_ERROR;
        }
    };
    // The banner of `prepare` names the entry of pod-enter, which this run
    // does not take: one line under --verbose says what was entered.
    if p.env.iter().any(|e| e.starts_with(crate::initns::LOG_VAR)) {
        eprintln!(
            "podvm run: --podvm-init=image: {} of the image is PID 1 of a PID namespace of this \
             run, with a mount, a cgroup, an IPC and a UTS namespace of its own, the network \
             of the host, the subordinate id range in the uid map, and its cgroups under {}. \
             The command joins those namespaces after the boot. No interposer loads: the \
             lines above about the interposer and the shimmed devices do not apply \
             (TODO/enter.md T-2002)",
            crate::initns::INIT,
            base.display()
        );
    }
    take_hold(store, &fg.id, pid, &mut fg.hold, err);
    let code = podvm_enter::wait_forwarding(&podvm_enter::Child::of(pid), err);
    crate::initns::end(&base);
    match code {
        Ok(c) => c,
        Err(e) => {
            eprintln!("podvm run: {e}");
            e.exit_code()
        }
    }
}
