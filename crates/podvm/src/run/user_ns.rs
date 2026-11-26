//! TODO/enter.md T-1927 (PodUtils/podrunner#196): the user of a run on the
//! namespace rung, detached (pod-enter's user namespace) or in the
//! foreground (podvm's own, where podcrates `v0.63.0` nests one).
//!
//! pod-enter maps the caller to uid 0 of a one-uid namespace unless the
//! plan names other ids (`Plan::userns_ids`, podcrates `v0.62.0`). docker
//! runs the image's `USER`; so `run -d` and `create` on the namespace rung
//! record the ids of `--user`, else of the image's `USER`, resolved against
//! the tree, and the launcher hands them to the plan. Uid 0 records nothing.

use podvm_supervise::table::USERNS_IDS_VAR;

/// Record the ids for the launcher in `env`, where they apply.
pub(super) fn push(
    verb: &str,
    o: &super::Opts,
    image_user: &str,
    rootfs: &str,
    userland: Option<&crate::lifecycle::Userland>,
    findings: &podvm_probe::Findings,
    env: &mut Vec<String>,
) -> Result<(), i32> {
    env.retain(|e| !e.starts_with(&format!("{USERNS_IDS_VAR}=")));
    let discard = crate::initns::discards(o.rm, &o.volumes, &o.ro_volumes);
    crate::initns::mark_env(env, o.init_image, o.ask.verbose, discard, o.tty); // T-2002, T-2010
    let rung = podvm_enter::entered_rung(podvm_probe::select::Selection::choose(findings).rung);
    // A detached run (pod-enter's user namespace) and a foreground run in
    // podvm's own (podcrates `v0.63.0` nests one); a host root keeps its ids.
    if crate::userns::host_root() {
        return Ok(());
    }
    if userland.is_some() || rung != podvm_probe::select::Rung::Namespace {
        // The interposer of the loader family says its own line of T-0711;
        // a rung with no interposer says nothing of its own.
        if let (Some(spec), Some(u)) = (o.user.as_deref(), userland)
            && u.dispatch_exe.is_some()
        {
            let (uid, _) = crate::lifecycle::resolve_user(verb, rootfs, spec)?;
            say_kept(verb, spec, uid, Kept::Caller);
        }
        return Ok(());
    }
    let image = (!image_user.is_empty()).then(|| image_user.to_string());
    let Some(spec) = o.user.as_ref().or(image.as_ref()) else {
        return Ok(());
    };
    let (uid, gid) = crate::lifecycle::resolve_user(verb, rootfs, spec)?;
    // TODO/probe.md T-1911: a foreground run in podvm's own namespace with
    // the subordinate range holds the ids, so the payload drops to them for
    // real; a nested namespace would map them to the root of this one, and
    // the payload would own each file of root.
    if crate::userns::active() && crate::userns::owners::holds(uid, gid) {
        return Ok(());
    }
    if uid != 0 {
        env.push(format!("{USERNS_IDS_VAR}={uid}:{gid}"));
        if let Some(spec) = o.user.as_deref() {
            say_kept(verb, spec, uid, Kept::OneId);
        }
    }
    Ok(())
}

/// How a run keeps the caller's access under `--user`.
#[derive(Clone, Copy)]
pub(super) enum Kept {
    /// A user namespace that maps one id, the caller's, to the uid.
    OneId,
    /// The caller's uid, on a rung with no interposer (the syscall tier).
    Caller,
}

/// TODO/cli.md T-2087 (OR-B40-17): the one line of a run whose `--user`
/// does not drop the caller's access, as `exec -u` says it (T-0711). docker
/// drops to the uid; a script that gives `-u` to lower the privileges of
/// the payload learns that it did not.
pub(super) fn kept_line(verb: &str, spec: &str, uid: u32, how: Kept) -> Option<String> {
    let head = format!("podvm {verb}: --user {spec}");
    match how {
        _ if uid == 0 => None,
        Kept::OneId => Some(format!(
            "{head}: the payload runs as uid {uid} of a user namespace that maps one id, the \
             caller's, so it keeps the caller's access and owns each file of the tree; a write \
             that uid {uid} could not make in docker works here. A subordinate range of the \
             caller (/etc/subuid, /etc/subgid) lets a foreground run drop to the ids (TODO/probe.md \
             T-1911)"
        )),
        Kept::Caller => Some(format!(
            "{head}: the payload keeps the caller's uid and access to each file of the tree; \
             a write that uid {uid} could not make in docker works here (TODO/interpose.md T-0711)"
        )),
    }
}

fn say_kept(verb: &str, spec: &str, uid: u32, how: Kept) {
    if let Some(line) = kept_line(verb, spec, uid, how) {
        eprintln!("{line}");
    }
}

/// Whether `env` records ids: the banner then does not say that the run
/// keeps uid 0.
pub(super) fn applied(env: &[String]) -> bool {
    podvm_supervise::table::userns_ids_of(env).is_some()
}

/// TODO/cli.md T-2087 (OR-B40-17): one rule for each row: a uid other than
/// 0 that the run does not drop to gives one line that names how the
/// caller's access stays; uid 0 gives none.
#[cfg(test)]
#[test]
fn a_user_that_keeps_the_callers_access_is_named() {
    // (uid, how, the words of the line, or none)
    let rows = [
        (0, Kept::OneId, None),
        (0, Kept::Caller, None),
        (65534, Kept::OneId, Some("maps one id")),
        (1000, Kept::Caller, Some("keeps the caller's uid")),
    ];
    for (uid, how, words) in rows {
        let line = kept_line("run", "u", uid, how);
        assert_eq!(line.is_some(), words.is_some(), "{uid}");
        if let (Some(l), Some(w)) = (line, words) {
            assert!(
                l.contains(w) && l.starts_with("podvm run: --user u: "),
                "{l}"
            );
            assert_eq!(l.lines().count(), 1, "{l}");
        }
    }
}
