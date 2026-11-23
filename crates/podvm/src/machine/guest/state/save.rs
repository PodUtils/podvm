//! TODO/machine.md T-2008 and T-2051: the save of the state of a guest
//! through the monitor of the emulator (`pod_guest::warm::line::save`), and
//! the wait for a guest that loads one.
//!
//! The crate stops the guest, streams its state into a partial file, waits
//! for each answer of the monitor on the one connection (a late answer of
//! `query-migrate` within the bound of the save is no failure), lets the
//! guest go on, and gives the files their names: a reader finds a whole
//! state or none. Its `Monitor::connect` bounds each connection (podcrates
//! `v0.81.0`, PodUtils/podvm#633): QEMU serves one client of its monitor at
//! a time and queues one more, and a `connect` to a full queue fails at
//! once in place of a wait in the kernel with no bound (T-2048).

use std::path::Path;
use std::time::{Duration, Instant};

use podvm_windows::qmp::Monitor;
use podvm_windows::warm::line;

use super::store::Files;
use super::{Transport, READY_BOUND, SAVE_BOUND};

/// The note of the record of a state that holds the memory of its guest.
pub(super) const NOTE_MEM: &str = "mem_mib";
/// The bound of the connection to the monitor of a guest that loads a
/// state: the emulator makes its socket when it starts.
const CONNECT_BOUND: Duration = Duration::from_secs(10);

/// One save that ended well.
#[derive(Debug, Clone, Copy)]
pub struct Saved {
    pub bytes: u64,
    pub took: Duration,
    /// The transport that wrote the state.
    pub transport: Transport,
}

/// One failure of the crate as a line of stderr: for a missing answer,
/// also what the emulator does then.
pub(super) fn explain(e: &pod_core::Failure) -> String {
    if e.class == pod_core::Class::Timeout {
        format!(
            "{e}: the emulator holds its lock (it writes the last part of the state to a slow \
             disk), or it hangs"
        )
    } else {
        e.to_string()
    }
}

/// The free bytes of the file system of the state, for the line of a
/// failure: a full disk shows here.
fn free(state: &Path) -> String {
    let dir = state.parent().unwrap_or(Path::new("/"));
    let free = podvm_image::space::read(&dir.to_string_lossy())
        .map_or("the free space is not read".to_string(), |h| {
            format!("{} MiB free", h.free.bytes.div_ceil(1 << 20))
        });
    format!("{free} in {}", dir.display())
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
    save_within(qmp, f, fingerprint, mem_mib, first, unique, SAVE_BOUND)
}

/// [`save`] with the bound `wait` of each migration. A `file:` save that
/// the emulator refuses or fails (a QEMU before 8.2) is tried again through
/// a pipe; a timeout or a lost monitor takes no second migration.
pub(super) fn save_within(
    qmp: &Path,
    f: &Files,
    fingerprint: &str,
    mem_mib: u64,
    first: Transport,
    unique: &str,
    wait: Duration,
) -> (Result<Saved, String>, Result<(), String>) {
    let notes = [(NOTE_MEM.to_string(), mem_mib.to_string())];
    let s = line::Save {
        monitor: qmp,
        stream: &f.state,
        record: &f.meta,
        fingerprint,
        notes: &notes,
        transport: first,
        fallback: true,
        unique,
        wait,
    };
    let end = line::save(&s, &mut ());
    let saved = end
        .saved
        .map(|s| Saved {
            bytes: s.bytes,
            took: s.took,
            transport: s.transport,
        })
        .map_err(|e| {
            format!(
                "through {}: {} ({}; the monitor {})",
                first.word(),
                explain(&e),
                free(&f.state),
                qmp.display()
            )
        });
    let runs = end.goes_on.map_err(|e| format!("cont: {}", explain(&e)));
    (saved, runs)
}

/// Wait until the guest of an emulator that loads a state runs. A state
/// that was saved from a stopped guest loads as a stopped guest: `cont`
/// starts it. An error where the monitor does not answer or the guest
/// does not run within [`READY_BOUND`].
pub fn running(qmp: &Path) -> Result<(), String> {
    // T-2051: `pod_guest::warm::line::running` takes the emulator to see
    // its end during the load, and this wait runs in the thread of the
    // writer of the job, which does not hold it: with no emulator, the
    // crate tries a new connection until its bound (60 s) where this wait
    // ends at the closed connection. So podvm keeps this wait.
    let mut m = Monitor::connect(qmp, CONNECT_BOUND).map_err(|e| e.to_string())?;
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
