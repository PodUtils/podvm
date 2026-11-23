//! The record of a foreground run: made before the entry, live while the
//! payload runs, and ended with the code (TODO/supervise.md T-1421,
//! T-1786).

use super::tee::*;
use podvm_image::clock;

/// One foreground run's record-in-progress.
///
/// TODO/supervise.md T-1421. docker always records a foreground run as
/// `Exited`, so `ps -a` lists it and `logs` reads it. podvm does the
/// same: the record is created before the entry (so a duplicate `--name`
/// refuses before anything runs), the payload's stdout is teed to the
/// container log while it runs, and the record lands `Exited` with the
/// code and the times when it ends. `--rm` deletes the record on exit
/// instead of finalizing it.
pub(super) struct Foreground {
    pub(super) id: String,
    rm: bool,
    started: String,
    tee: Vec<Tee>,
    /// TODO/supervise.md T-1786: the hold of the payload, on the rungs
    /// where podvm spawns the payload itself and has its pid.
    pub(super) hold: Option<podvm_supervise::Hold>,
}

/// Create the foreground record and start the stdout tee, before the
/// entry. A `--name` that cannot be recorded refuses here naming it,
/// instead of running and dropping the name: an accepted flag is never
/// silently dropped (T-1421's decision).
pub(super) fn fg_begin(
    store: &podvm_image::Store,
    p: &crate::lifecycle::Prepared,
) -> std::result::Result<Foreground, i32> {
    let c = match podvm_supervise::create(
        store,
        p.name.as_deref(),
        &p.image,
        &p.record.manifest_digest,
        &p.rootfs,
        p.argv.clone(),
        p.env.clone(),
        p.working_dir.clone(),
        &p.rung,
        p.completion.clone(),
        p.completion_degraded,
        crate::lifecycle::detached_drive(p.userland.as_ref()),
    ) {
        // TODO/cli.md T-1695: the labels and the options of the run join
        // the record, as for `run -d`.
        Ok(c) => match podvm_supervise::set_hardening(store, &c.id, p.hardening.clone()) {
            Ok(()) => c,
            Err(e) => {
                let _ = podvm_supervise::remove(store, &c.id, true);
                eprintln!("podvm run: the run could not be recorded: {e}");
                return Err(podvm_image::error::EXIT_RUNTIME_ERROR);
            }
        },
        Err(e) => {
            crate::run::drop_unrecorded(store, &p.rootfs); // T-1709
            if let Some(why) = owner_refusal(store.root(), current_euid()) {
                eprintln!("podvm run: {why}");
                return Err(podvm_image::error::EXIT_RUNTIME_ERROR);
            }
            match &p.name {
                Some(n) => {
                    eprintln!("podvm run: --name {n:?}: the run could not be recorded: {e}")
                }
                None => eprintln!("podvm run: the run could not be recorded: {e}"),
            }
            return Err(podvm_image::error::EXIT_RUNTIME_ERROR);
        }
    };
    let log_path = podvm_supervise::table::log_path(store, &c.id);
    match tee_output_to(&log_path) {
        Ok(tee) => Ok(Foreground {
            id: c.id,
            rm: p.rm,
            started: clock::now(),
            tee,
            hold: None,
        }),
        Err(why) => {
            eprintln!("podvm run: the run's output could not be captured for its record: {why}");
            let _ = podvm_supervise::remove(store, &c.id, true);
            Err(podvm_image::error::EXIT_RUNTIME_ERROR)
        }
    }
}

/// Finalize the foreground record: stdout restored first (no byte still
/// in flight), then `Exited` with the code and the times, or the
/// record's removal under `--rm`.
pub(super) fn fg_end(store: &podvm_image::Store, fg: Foreground, code: i32) {
    // stderr first, then stdout: the reverse of the order they were taken.
    for tee in fg.tee.into_iter().rev() {
        untee(tee);
    }
    // TODO/cli.md T-1655: the `--tmpfs` directories go with the run.
    let _ = std::fs::remove_dir_all(crate::run::tmpfs_base(store));
    // TODO/supervise.md T-1786: the hold writes `Exited`, the code and the
    // time, and gives the code to each `wait`.
    let held = fg.hold.map(|h| h.finish(code).is_ok()).unwrap_or(false);
    if fg.rm {
        let _ = podvm_supervise::remove(store, &fg.id, true);
        return;
    }
    if held {
        return;
    }
    let finished = clock::now();
    let started = fg.started.clone();
    let id = fg.id.clone();
    let _ = podvm_supervise::table::update(store, |t| {
        match t.containers.iter_mut().find(|c| c.id == id) {
            Some(c) => {
                c.state = podvm_supervise::table::State::Exited;
                c.exit_code = Some(code);
                if c.started_at.is_none() {
                    c.started_at = Some(started.clone());
                }
                c.finished_at = Some(finished.clone());
                Ok(())
            }
            None => Err(podvm_supervise::Error::new(format!(
                "the foreground record {id} is gone"
            ))),
        }
    });
}

/// TODO/supervise.md T-1786: the record reads `Running` while the payload
/// `pid` runs. A hold that cannot start leaves the run as it was: the
/// record lands at the end, and a line names why.
pub(super) fn take_hold(
    store: &podvm_image::Store,
    id: &str,
    pid: i64,
    hold: &mut Option<podvm_supervise::Hold>,
    err: &mut dyn std::io::Write,
) {
    crate::run::cgroup::leave(); // TODO/cli.md T-1747: the leaf is the payload's
    crate::run::pty::set_payload(pid); // OR-02: the target of the interrupt key
    match podvm_supervise::foreground(store, id, pid) {
        Ok(h) => {
            *hold = Some(h);
            // TODO/probe.md T-1909: the record names host pids.
            if crate::userns::active() {
                let me = i64::from(std::process::id());
                match (crate::userns::host_pid(pid), crate::userns::host_pid(me)) {
                    (Some(p), Some(l)) => {
                        if let Err(e) = podvm_supervise::set_pids(store, id, p, l) {
                            writeln!(err, "podvm run: the host pids of the record: {e}")
                                .unwrap_or(());
                        }
                    }
                    _ => {
                        writeln!(err, "podvm run: the host pids of the run are unknown")
                            .unwrap_or(());
                    }
                }
            }
        }
        Err(e) => {
            let _ = writeln!(err, "podvm run: the record of the run is not live: {e}");
        }
    }
}

/// TODO/supervise.md T-1851: a store of another user. The table of
/// `pod-supervise` is replaced by a rename, so a run of root leaves it
/// owned by root, and the next run of a user cannot write it. The refusal
/// names the owner, the file and the remedy.
/// ⛔ OR-B38-02 of the seventh outside pass (TODO/cli.md T-2073): only where
/// that owner denies this process. Before, an owner of another uid gave this
/// text for each failure of the record: a name in use or a table that does
/// not parse in a store root that root owns with mode 777 read "is owned by
/// uid 65534", which was not the cause.
pub(crate) fn owner_refusal(root: &std::path::Path, euid: u32) -> Option<String> {
    owner_refusal_with(root, euid, &allowed)
}

/// [`owner_refusal`] with the test of access as an argument: `access(path,
/// mode)` is true where this process has `mode` (`R_OK`, `W_OK|X_OK`).
fn owner_refusal_with(
    root: &std::path::Path,
    euid: u32,
    access: &dyn Fn(&std::path::Path, i32) -> bool,
) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    const R_OK: i32 = 4;
    const W_OK: i32 = 2;
    const X_OK: i32 = 1;
    let table = root.join("containers.json");
    // The table is read, and replaced by a rename in the root.
    for (f, mode) in [(&table, R_OK), (&root.to_path_buf(), W_OK | X_OK)] {
        if let Ok(m) = std::fs::metadata(f)
            && m.uid() != euid
            && !access(f, mode)
        {
            return Some(format!(
                "the run could not be recorded: {} is owned by uid {}, and this \
                 process runs as uid {euid}; use a store of your own ($PODVM_STORE) \
                 (TODO/supervise.md T-1851)",
                f.display(),
                m.uid()
            ));
        }
    }
    None
}

/// `access(2)`: true where this process has `mode` on `path`.
fn allowed(path: &std::path::Path, mode: i32) -> bool {
    unsafe extern "C" {
        fn access(path: *const std::ffi::c_char, mode: i32) -> i32;
    }
    let Ok(c) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) else {
        return false;
    };
    // SAFETY: a NUL-ended path and an int.
    unsafe { access(c.as_ptr(), mode) == 0 }
}

fn current_euid() -> u32 {
    use std::os::unix::fs::MetadataExt;
    // The owner of a file this process makes is its effective uid.
    std::fs::metadata("/proc/self")
        .map(|m| m.uid())
        .unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    /// TODO/supervise.md T-1851: a table owned by another uid gives a
    /// refusal that names the uid and `$PODVM_STORE`; a store of this uid
    /// gives none.
    #[test]
    fn store_owner_refusal_names_the_uid_and_the_remedy() {
        use std::os::unix::fs::MetadataExt;
        let d = std::env::temp_dir().join(format!("podvm-t1851-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("containers.json"), "{}").unwrap();
        let mine = std::fs::metadata(&d).unwrap().uid();
        assert_eq!(super::owner_refusal(&d, mine), None);
        let other = mine.wrapping_add(4242);
        let why = super::owner_refusal_with(&d, other, &|_, _| false).unwrap();
        assert!(why.contains(&format!("owned by uid {mine}")), "{why}");
        assert!(
            why.contains("$PODVM_STORE") && why.contains("containers.json"),
            "{why}"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// T-2073 (OR-B38-02): one rule for each cause: the text of the owner
    /// only where a file of another uid denies this process its access; a
    /// root of another uid that this process may write (mode 777) gives
    /// no such text, so the caller prints the true cause.
    #[test]
    fn the_owner_is_the_cause_only_where_it_denies_the_access() {
        use std::os::unix::fs::MetadataExt;
        let d = std::env::temp_dir().join(format!("podvm-t2073-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("containers.json"), "not json").unwrap();
        let mine = std::fs::metadata(&d).unwrap().uid();
        let other = mine.wrapping_add(4242);
        let table = d.join("containers.json");
        // (euid, the path that the access test refuses, the file named)
        let cases: [(u32, Option<&std::path::Path>, Option<&std::path::Path>); 5] = [
            (mine, None, None),
            (mine, Some(&d), None),
            (other, None, None),
            (other, Some(&table), Some(&table)),
            (other, Some(&d), Some(&d)),
        ];
        for (euid, denied, want) in cases {
            let got = super::owner_refusal_with(&d, euid, &|p, _| Some(p) != denied);
            match want {
                None => assert_eq!(got, None, "euid {euid}, denied {denied:?}"),
                Some(f) => {
                    let why = got.unwrap_or_else(|| panic!("euid {euid}: no refusal"));
                    assert!(why.contains(&format!("{} is owned", f.display())), "{why}");
                }
            }
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
