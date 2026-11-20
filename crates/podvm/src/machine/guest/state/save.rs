//! TODO/machine.md T-2008: the save of the state of a guest through the
//! monitor of the emulator, and the wait for a guest that loads one.

use std::path::Path;
use std::time::{Duration, Instant};

use podvm_windows::qmp::Monitor;

use super::store::{Files, Meta};
use super::{Transport, READY_BOUND, SAVE_BOUND};

/// The bound of one connection to the monitor, outside a migration.
const CONNECT_BOUND: Duration = Duration::from_secs(15);
/// The shortest bound of a new connection during a migration: one whole
/// try, which is the read bound of `pod_guest::qmp::Monitor` (5 s) for
/// the greeting, and a margin.
const GREETING: Duration = Duration::from_secs(6);

/// The bounds of one save: the migration of one transport, and each
/// connection to the monitor outside a migration.
#[derive(Debug, Clone, Copy)]
pub(super) struct Bounds {
    pub save: Duration,
    pub connect: Duration,
}

/// The bounds of a save of a run.
const BOUNDS: Bounds = Bounds {
    save: SAVE_BOUND,
    connect: CONNECT_BOUND,
};

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
/// that failed, the status before it, the time, and the disk. `m` is
/// `None` after the call where the monitor did not come back: then no
/// other request can go to the emulator.
pub(super) fn migrate(
    m: &mut Option<Monitor>,
    qmp: &Path,
    transport: Transport,
    part: &str,
    bounds: Bounds,
) -> Result<Duration, String> {
    let uri = transport.save_uri(part);
    let via = transport.word();
    let body = serde_json::json!({"execute": "migrate", "arguments": {"uri": uri}}).to_string();
    m.as_mut()
        .ok_or_else(|| format!("through {via}: no connection to the monitor"))?
        .ask(&body)
        .map_err(|e| format!("through {via}: {}", asked("migrate", &e)))?;
    let started = Instant::now();
    let end = started + bounds.save;
    let mut longest = Duration::ZERO;
    let mut before = "none".to_string();
    let mut quiet = 0u32;
    loop {
        let at = Instant::now();
        let asked_now = match m.as_mut() {
            Some(mon) => mon.ask("{\"execute\":\"query-migrate\"}"),
            None => return Err(format!("through {via}: no connection to the monitor")),
        };
        let line = match asked_now {
            Ok(line) => line,
            // T-2008 (`kvm.yml` run 44749207855): under KVM the emulator
            // answered nothing for longer than the read bound of the
            // monitor while it wrote the last part of the state. The late
            // answer goes to this connection, which no read can follow: a
            // new connection asks again, until the bound of the save.
            //
            // T-2048 (`kvm.yml` run 44786700578): QEMU serves one client
            // of the monitor at a time, and its socket queues one more
            // connection. The new connection came while this one was open:
            // QEMU never took it, the third `connect` waited in the kernel
            // with no bound, and the run hung until the `timeout` of the
            // caller with no line. This connection closes first.
            Err(e) if no_answer(&e) && Instant::now() < end => {
                quiet += 1;
                longest = longest.max(at.elapsed());
                *m = None;
                // At least one whole try: the greeting comes when the
                // emulator lets go of its lock.
                let wait = end.saturating_duration_since(Instant::now()).max(GREETING);
                *m = Some(connect(qmp, wait).map_err(|again| {
                    format!(
                        "through {via}: {}; a new connection to the monitor: {again} ({:.1} s \
                         after the start of the migration; {})",
                        asked("query-migrate", &e),
                        started.elapsed().as_secs_f64(),
                        disk(part)
                    )
                })?);
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
                if let Some(mon) = m.as_mut() {
                    let _ = mon.ask("{\"execute\":\"migrate_cancel\"}");
                }
                return Err(format!(
                    "through {via}: the migration did not end in {} s (status {status:?}; {})",
                    bounds.save.as_secs(),
                    disk(part)
                ));
            }
            _ => std::thread::sleep(Duration::from_millis(25)),
        }
        before = status;
    }
}

/// A connection to the monitor at `qmp` within `wait`, tried again until
/// then: the emulator greets a new client only when it lets go of its
/// lock, and only when no other client is connected.
///
/// QEMU queues one connection that it does not take (the backlog of its
/// socket is 1), and a `connect` to a full queue waits in the kernel with
/// no bound (T-2048). So each try runs in a thread of its own. Where no
/// try ends within the bound, the caller goes on with an error; the
/// thread ends when the emulator takes its connection (the connection
/// then closes at once) or when the emulator ends.
fn connect(qmp: &Path, wait: Duration) -> Result<Monitor, String> {
    let (tx, rx) = std::sync::mpsc::channel();
    let path = qmp.to_path_buf();
    let end = Instant::now() + wait;
    std::thread::spawn(move || {
        let got = loop {
            // The wait of `Monitor::connect` is the wait for the socket:
            // the emulator makes it when it starts.
            match Monitor::connect(&path, end.saturating_duration_since(Instant::now())) {
                Ok(m) => break Ok(m),
                Err(e) if Instant::now() >= end => break Err(asked("the connection", &e)),
                Err(_) => std::thread::sleep(Duration::from_millis(200)),
            }
        };
        let _ = tx.send(got);
    });
    match rx.recv_timeout(wait) {
        Ok(got) => got,
        Err(_) => Err(format!(
            "no connection within {:.1} s: the emulator takes no new client (it holds its \
             lock, or it hangs)",
            wait.as_secs_f64()
        )),
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
    save_within(qmp, f, fingerprint, mem_mib, first, unique, BOUNDS)
}

/// [`save`] with the bounds of `bounds`: each wait of a save has one.
pub(super) fn save_within(
    qmp: &Path,
    f: &Files,
    fingerprint: &str,
    mem_mib: u64,
    first: Transport,
    unique: &str,
    bounds: Bounds,
) -> (Result<Saved, String>, Result<(), String>) {
    let started = Instant::now();
    let mut m = match connect(qmp, bounds.connect) {
        Ok(m) => Some(m),
        // Nothing was asked of the emulator: its guest runs as before.
        Err(e) => {
            let why = format!("the monitor of the emulator ({}): {e}", qmp.display());
            return (Err(why), Ok(()));
        }
    };
    let part = f.state.with_extension(format!("vm.part-{unique}"));
    let meta_part = f.meta.with_extension(format!("json.part-{unique}"));
    let path = part.to_string_lossy().into_owned();
    // No limit of the rate: the default is 1 Gbit/s on some versions. A
    // refusal leaves the default, and the save is then slower.
    if let Some(mon) = m.as_mut() {
        let _ =
            mon.ask("{\"execute\":\"migrate-set-parameters\",\"arguments\":{\"max-bandwidth\":0}}");
    }
    // The guest stops for the save, as `pod_guest::warm::save` does: the
    // state is the one of this moment, and no page changes under the copy.
    // A connection that a late answer closed is made again, within its
    // bound, for `cont`.
    let cont = |m: &mut Option<Monitor>| -> Result<(), String> {
        if m.is_none() {
            *m = Some(
                connect(qmp, bounds.connect)
                    .map_err(|e| format!("cont: a new connection to the monitor: {e}"))?,
            );
        }
        match m.as_mut() {
            Some(mon) => mon
                .ask("{\"execute\":\"cont\"}")
                .map(drop)
                .map_err(|e| asked("cont", &e)),
            None => Err("cont: no connection to the monitor".to_string()),
        }
    };
    let stopped = match m.as_mut() {
        Some(mon) => mon.ask("{\"execute\":\"stop\"}"),
        None => {
            return (
                Err("stop: no connection to the monitor".to_string()),
                Ok(()),
            )
        }
    };
    if let Err(e) = stopped {
        return (Err(asked("stop", &e)), cont(&mut m));
    }
    let mut transport = first;
    let mut done = migrate(&mut m, qmp, transport, &path, bounds);
    // A QEMU before 8.2 has no `file:` transport: the pipe is the second
    // path. A monitor that did not come back takes no second migration.
    if done.is_err() && transport == Transport::File && m.is_some() {
        let _ = std::fs::remove_file(&part);
        transport = Transport::Pipe;
        let second = migrate(&mut m, qmp, transport, &path, bounds);
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
    let mut m = connect(qmp, Duration::from_secs(10))?;
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
