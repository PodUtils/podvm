//! TODO/machine.md T-2008: the save of the state of a guest through the
//! monitor of the emulator, and the wait for a guest that loads one.

use std::path::Path;
use std::time::{Duration, Instant};

use podvm_windows::qmp::Monitor;

use super::store::{Files, Meta};
use super::{Transport, READY_BOUND, SAVE_BOUND};

/// One save that ended well.
#[derive(Debug, Clone, Copy)]
pub struct Saved {
    pub bytes: u64,
    pub took: Duration,
}

/// The status of a migration in a response of `query-migrate`.
pub(super) fn migration(line: &str) -> (String, String) {
    let v: serde_json::Value = serde_json::from_str(line).unwrap_or_default();
    let r = &v["return"];
    (
        r["status"].as_str().unwrap_or_default().to_string(),
        r["error-desc"].as_str().unwrap_or_default().to_string(),
    )
}

/// One `migrate` of the guest to `part`, to its end.
pub(super) fn migrate(m: &mut Monitor, transport: Transport, part: &str) -> Result<(), String> {
    let uri = transport.save_uri(part);
    let body = serde_json::json!({"execute": "migrate", "arguments": {"uri": uri}}).to_string();
    m.ask(&body).map_err(|e| e.to_string())?;
    let end = Instant::now() + SAVE_BOUND;
    loop {
        let line = m
            .ask("{\"execute\":\"query-migrate\"}")
            .map_err(|e| e.to_string())?;
        let (status, why) = migration(&line);
        match status.as_str() {
            "completed" => return Ok(()),
            "failed" | "cancelled" => {
                return Err(format!("the migration is {status}: {why}"));
            }
            _ if Instant::now() >= end => {
                let _ = m.ask("{\"execute\":\"migrate_cancel\"}");
                return Err(format!(
                    "the migration did not end in {} s (status {status:?})",
                    SAVE_BOUND.as_secs()
                ));
            }
            _ => std::thread::sleep(Duration::from_millis(25)),
        }
    }
}

/// Save the state of the guest behind the QMP socket `qmp` into the files
/// of `f`, and let the guest go on. `unique` names the partial files of
/// this run. The second value is false where the guest does not run after
/// the save: the caller then boots another one.
pub fn save(
    qmp: &Path,
    f: &Files,
    fingerprint: &str,
    mem_mib: u64,
    first: Transport,
    unique: &str,
) -> (Result<Saved, String>, bool) {
    let started = Instant::now();
    let mut m = match Monitor::connect(qmp, Duration::from_secs(10)) {
        Ok(m) => m,
        // Nothing was asked of the emulator: its guest runs as before.
        Err(e) => return (Err(e.to_string()), true),
    };
    let part = f.state.with_extension(format!("vm.part-{unique}"));
    let meta_part = f.meta.with_extension(format!("json.part-{unique}"));
    let path = part.to_string_lossy().into_owned();
    // No limit of the rate: the default is 1 Gbit/s on some versions. A
    // refusal leaves the default, and the save is then slower.
    let _ = m.ask("{\"execute\":\"migrate-set-parameters\",\"arguments\":{\"max-bandwidth\":0}}");
    // The guest stops for the save, as `pod_guest::warm::save` does: the
    // state is the one of this moment, and no page changes under the copy.
    if let Err(e) = m.ask("{\"execute\":\"stop\"}") {
        return (Err(e.to_string()), m.ask("{\"execute\":\"cont\"}").is_ok());
    }
    let mut transport = first;
    let mut done = migrate(&mut m, transport, &path);
    // A QEMU before 8.2 has no `file:` transport: the pipe is the second path.
    if done.is_err() && transport == Transport::File {
        let _ = std::fs::remove_file(&part);
        transport = Transport::Pipe;
        let second = migrate(&mut m, transport, &path);
        done = match (second, done) {
            (Ok(()), _) => Ok(()),
            (Err(pipe), Err(file)) => Err(format!("{file}; through a pipe: {pipe}")),
            (Err(pipe), Ok(())) => Err(pipe),
        };
    }
    // A guest after a migration is paused; a guest whose migration failed
    // runs or is paused. `cont` is right for each.
    let runs = m.ask("{\"execute\":\"cont\"}").is_ok();
    let stored = done.and_then(|()| {
        let io = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
        let file = std::fs::File::open(&part).map_err(|e| io(&part, e))?;
        // The bytes are on the disk before the file gets its name: a state
        // that a crash cut is not loaded as a whole one.
        file.sync_all().map_err(|e| io(&part, e))?;
        let bytes = file.metadata().map_err(|e| io(&part, e))?.len();
        if bytes == 0 {
            return Err(format!("{}: the emulator wrote no byte", part.display()));
        }
        let meta = Meta {
            fingerprint: fingerprint.to_string(),
            mem_mib,
            bytes,
            transport,
        };
        std::fs::write(&meta_part, meta.text()).map_err(|e| io(&meta_part, e))?;
        std::fs::File::open(&meta_part)
            .and_then(|m| m.sync_all())
            .map_err(|e| io(&meta_part, e))?;
        // The record first goes away, so that no reader pairs the record
        // of the state before with the bytes of this one.
        let _ = std::fs::remove_file(&f.meta);
        std::fs::rename(&part, &f.state).map_err(|e| io(&f.state, e))?;
        std::fs::rename(&meta_part, &f.meta).map_err(|e| io(&f.meta, e))?;
        if let Some(dir) = f.state.parent().and_then(|d| std::fs::File::open(d).ok()) {
            let _ = dir.sync_all();
        }
        Ok(Saved {
            bytes,
            took: started.elapsed(),
        })
    });
    if stored.is_err() {
        let _ = std::fs::remove_file(&part);
        let _ = std::fs::remove_file(&meta_part);
    }
    (stored, runs)
}

/// Wait until the guest of an emulator that loads a state runs. A state
/// that was saved from a stopped guest loads as a stopped guest: `cont`
/// starts it. An error where the monitor does not answer or the guest
/// does not run within [`READY_BOUND`].
pub fn running(qmp: &Path) -> Result<(), String> {
    let mut m = Monitor::connect(qmp, Duration::from_secs(10)).map_err(|e| e.to_string())?;
    let end = Instant::now() + READY_BOUND;
    loop {
        let line = m
            .ask("{\"execute\":\"query-status\"}")
            .map_err(|e| e.to_string())?;
        let v: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
        let status = v["return"]["status"].as_str().unwrap_or_default();
        match status {
            "running" => return Ok(()),
            "inmigrate" => {}
            // The load is complete, and the guest waits for `cont`.
            "paused" | "prelaunch" | "postmigrate" => {
                m.ask("{\"execute\":\"cont\"}").map_err(|e| e.to_string())?;
            }
            other => return Err(format!("the guest is {other:?} after the load")),
        }
        if Instant::now() >= end {
            return Err(format!(
                "the load did not end in {} s",
                READY_BOUND.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
