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
    /// The longest time that the monitor took for one answer during the
    /// migration. The emulator answers no request while it holds its lock
    /// for the last part of the state; the monitor of `pod-guest` waits
    /// 5 s for an answer.
    pub longest: Duration,
}

/// The status of a migration in a response of `query-migrate`, and why it
/// failed: the text of the emulator, and each reason for which it blocks a
/// migration (`blocked-reasons`: a device or a CPU feature of `-cpu host`
/// that has no state to save).
pub(super) fn migration(line: &str) -> (String, String) {
    let v: serde_json::Value = serde_json::from_str(line).unwrap_or_default();
    let r = &v["return"];
    let mut why = r["error-desc"].as_str().unwrap_or_default().to_string();
    let blocked: Vec<&str> = r["blocked-reasons"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default();
    if !blocked.is_empty() {
        why.push_str(&format!(
            "; the emulator blocks a migration: {}",
            blocked.join("; ")
        ));
    }
    if why.is_empty() {
        why.push_str("the emulator gave no reason");
    }
    (r["status"].as_str().unwrap_or_default().to_string(), why)
}

/// True for a request that the monitor did not answer within its bound for
/// one read: no refusal, and no end of the connection.
fn no_answer(e: &pod_core::Failure) -> bool {
    e.class == pod_core::Class::Timeout
}

/// What one failed request of the monitor means for a line of stderr: the
/// request, and for a missing answer what the emulator does then.
pub(super) fn asked(what: &str, e: &pod_core::Failure) -> String {
    if no_answer(e) {
        format!(
            "the monitor of the emulator gave no answer to {what} within its bound ({e}): the \
             emulator holds its lock (it writes the last part of the state to a slow disk), or \
             it hangs"
        )
    } else {
        format!("{what}: {e}")
    }
}

/// The size of the partial file of a save and the free bytes of its file
/// system, for a line of a failure: a full disk and a slow disk show here.
fn disk(part: &str) -> String {
    let mib = |b: u64| b.div_ceil(1 << 20);
    let wrote = std::fs::metadata(part).map_or("no file".to_string(), |m| {
        format!("{} MiB written", mib(m.len()))
    });
    let dir = Path::new(part).parent().unwrap_or(Path::new("/"));
    let free = podvm_image::space::read(&dir.to_string_lossy())
        .map_or("free space not read".to_string(), |h| {
            format!("{} MiB free", mib(h.free.bytes))
        });
    format!("{wrote}, {free} in {}", dir.display())
}

/// One `migrate` of the guest to `part`, to its end: the longest time of
/// one answer of the monitor. An error names the transport, the request
/// that failed, the status before it, the time, and the disk.
pub(super) fn migrate(
    m: &mut Monitor,
    qmp: &Path,
    transport: Transport,
    part: &str,
) -> Result<Duration, String> {
    let uri = transport.save_uri(part);
    let via = transport.word();
    let body = serde_json::json!({"execute": "migrate", "arguments": {"uri": uri}}).to_string();
    m.ask(&body)
        .map_err(|e| format!("through {via}: {}", asked("migrate", &e)))?;
    let started = Instant::now();
    let end = started + SAVE_BOUND;
    let mut longest = Duration::ZERO;
    let mut before = "none".to_string();
    let mut quiet = 0u32;
    loop {
        let at = Instant::now();
        let line = match m.ask("{\"execute\":\"query-migrate\"}") {
            Ok(line) => line,
            // T-2008 (`kvm.yml` run 44749207855): under KVM the emulator
            // answered nothing for longer than the read bound of the
            // monitor while it wrote the last part of the state. The late
            // answer goes to this connection, which no read can follow: a
            // new connection asks again, until the bound of the save.
            Err(e) if no_answer(&e) && Instant::now() < end => {
                quiet += 1;
                longest = longest.max(at.elapsed());
                *m = reconnect(qmp, end).map_err(|again| {
                    format!(
                        "through {via}: {}; a new connection to the monitor: {again} ({:.1} s \
                         after the start of the migration; {})",
                        asked("query-migrate", &e),
                        started.elapsed().as_secs_f64(),
                        disk(part)
                    )
                })?;
                continue;
            }
            Err(e) => {
                return Err(format!(
                    "through {via}: {} {:.1} s after the start of the migration (status \
                     {before:?} before; {quiet} answers late before; {})",
                    asked("query-migrate", &e),
                    started.elapsed().as_secs_f64(),
                    disk(part)
                ))
            }
        };
        longest = longest.max(at.elapsed());
        let (status, why) = migration(&line);
        match status.as_str() {
            "completed" => return Ok(longest),
            "failed" | "cancelled" => {
                return Err(format!(
                    "through {via}: the migration is {status} after {:.1} s: {why} ({})",
                    started.elapsed().as_secs_f64(),
                    disk(part)
                ));
            }
            _ if Instant::now() >= end => {
                let _ = m.ask("{\"execute\":\"migrate_cancel\"}");
                return Err(format!(
                    "through {via}: the migration did not end in {} s (status {status:?}; {})",
                    SAVE_BOUND.as_secs(),
                    disk(part)
                ));
            }
            _ => std::thread::sleep(Duration::from_millis(25)),
        }
        before = status;
    }
}

/// A new connection to the monitor at `qmp`, tried again until `end`: the
/// emulator greets a new client only when it lets go of its lock.
fn reconnect(qmp: &Path, end: Instant) -> Result<Monitor, String> {
    loop {
        match Monitor::connect(qmp, Duration::from_secs(5)) {
            Ok(m) => return Ok(m),
            Err(e) if Instant::now() >= end => return Err(e.to_string()),
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

/// Save the state of the guest behind the QMP socket `qmp` into the files
/// of `f`, and let the guest go on. `unique` names the partial files of
/// this run. The second value is the reason where the guest does not run
/// after the save: the caller then boots another one.
pub fn save(
    qmp: &Path,
    f: &Files,
    fingerprint: &str,
    mem_mib: u64,
    first: Transport,
    unique: &str,
) -> (Result<Saved, String>, Result<(), String>) {
    let started = Instant::now();
    let mut m = match Monitor::connect(qmp, Duration::from_secs(10)) {
        Ok(m) => m,
        // Nothing was asked of the emulator: its guest runs as before.
        Err(e) => {
            let why = format!(
                "the monitor of the emulator ({}): {}",
                qmp.display(),
                asked("the connection", &e)
            );
            return (Err(why), Ok(()));
        }
    };
    let part = f.state.with_extension(format!("vm.part-{unique}"));
    let meta_part = f.meta.with_extension(format!("json.part-{unique}"));
    let path = part.to_string_lossy().into_owned();
    // No limit of the rate: the default is 1 Gbit/s on some versions. A
    // refusal leaves the default, and the save is then slower.
    let _ = m.ask("{\"execute\":\"migrate-set-parameters\",\"arguments\":{\"max-bandwidth\":0}}");
    // The guest stops for the save, as `pod_guest::warm::save` does: the
    // state is the one of this moment, and no page changes under the copy.
    let cont = |m: &mut Monitor| {
        m.ask("{\"execute\":\"cont\"}")
            .map(drop)
            .map_err(|e| asked("cont", &e))
    };
    if let Err(e) = m.ask("{\"execute\":\"stop\"}") {
        return (Err(asked("stop", &e)), cont(&mut m));
    }
    let mut transport = first;
    let mut done = migrate(&mut m, qmp, transport, &path);
    // A QEMU before 8.2 has no `file:` transport: the pipe is the second path.
    if done.is_err() && transport == Transport::File {
        let _ = std::fs::remove_file(&part);
        transport = Transport::Pipe;
        let second = migrate(&mut m, qmp, transport, &path);
        done = match (second, done) {
            (Ok(t), _) => Ok(t),
            (Err(pipe), Err(file)) => Err(format!("{file}; then {pipe}")),
            (Err(pipe), Ok(_)) => Err(pipe),
        };
    }
    // A guest after a migration is paused; a guest whose migration failed
    // runs or is paused. `cont` is right for each.
    let runs = cont(&mut m);
    let stored = done.and_then(|longest| {
        let io = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
        let file = std::fs::File::open(&part).map_err(|e| io(&part, e))?;
        // The bytes are on the disk before the file gets its name: a state
        // that a crash cut is not loaded as a whole one.
        file.sync_all().map_err(|e| io(&part, e))?;
        let bytes = file.metadata().map_err(|e| io(&part, e))?.len();
        if bytes == 0 {
            return Err(format!(
                "{}: the emulator wrote no byte through {}",
                part.display(),
                transport.word()
            ));
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
            longest,
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
