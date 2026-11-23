//! TODO/machine.md T-2008: the files of a saved state in the store, and
//! what a run does with them.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use podvm_image::store::Lock;

use podvm_windows::warm::line;

use super::save::NOTE_MEM;
use super::{Transport, BAD_HOLDS, KEEP, STRIKES};

/// The files of one state in the store.
#[derive(Debug, Clone)]
pub struct Files {
    pub state: PathBuf,
    pub meta: PathBuf,
    pub lock: PathBuf,
    pub bad: PathBuf,
    /// The nonce of the markers of the guest before a job.
    pub nonce: String,
}

/// The prefix of each file of a state of the image `digest`.
pub(super) fn prefix(digest: &str) -> String {
    format!("state-{}-", digest.trim_start_matches("sha256:"))
}

/// The extension of the record of a state. `json` is the record of the
/// layout before T-2051: the sweep takes it as a record, so it removes the
/// states of that layout as other old states.
const RECORD: &str = "record";
const OLD_RECORD: &str = "json";

/// The hex digits of the digest of a fingerprint in the name of a state.
const KEY_DIGITS: usize = 24;

pub fn files(store_root: &Path, digest: &str, fingerprint: &str) -> Files {
    let key = super::super::kernel::sha256_hex(fingerprint.as_bytes());
    let nonce = super::super::kernel::sha256_hex(format!("held\n{fingerprint}").as_bytes());
    let base = store_root.join("machine").join(format!(
        "{}{}",
        prefix(digest),
        key.chars().take(KEY_DIGITS).collect::<String>()
    ));
    Files {
        state: base.with_extension("vm"),
        meta: base.with_extension(RECORD),
        lock: base.with_extension("lock"),
        bad: base.with_extension("bad"),
        nonce: nonce.chars().take(32).collect(),
    }
}

/// What the record of a state says (`pod_guest::warm::line::Record`,
/// T-2051), as a run reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    pub mem_mib: u64,
    pub bytes: u64,
    pub transport: Transport,
}

/// The state of `f` where it is whole and is the one of `fingerprint`
/// (`pod_guest::warm::line::check`), with the memory of its guest. An error
/// is what is wrong: the state is then no state, and the run saves a new
/// one.
pub(super) fn valid(f: &Files, fingerprint: &str) -> Result<Meta, String> {
    let r = line::check(&f.state, &f.meta, fingerprint).map_err(|e| e.to_string())?;
    let mem_mib = r
        .note(NOTE_MEM)
        .and_then(|m| m.parse::<u64>().ok())
        .filter(|m| *m > 0 && *m <= 1 << 24)
        .ok_or_else(|| format!("{}: no memory size", f.meta.display()))?;
    Ok(Meta {
        mem_mib,
        bytes: r.bytes,
        transport: r.transport,
    })
}

/// The count and the last reason of the marker of a failed state, where
/// the marker is younger than [`BAD_HOLDS`]. A marker that podvm cannot
/// read counts as one that holds: no run pays for a state behind it.
pub(super) fn strikes(f: &Files, now: SystemTime) -> Option<(u32, String)> {
    let m = std::fs::metadata(&f.bad).ok()?;
    let age = now.duration_since(m.modified().ok()?).unwrap_or_default();
    if age >= BAD_HOLDS {
        return None;
    }
    let text = std::fs::read_to_string(&f.bad).unwrap_or_default();
    let (count, why) = text.split_once('\n').unwrap_or((&text, ""));
    Some((
        count.trim().parse().unwrap_or(STRIKES),
        why.lines().next().unwrap_or("no reason").to_string(),
    ))
}

/// The reason of a marker of a failed state that holds: [`STRIKES`]
/// failures in a row, the last one less than [`BAD_HOLDS`] ago.
pub(super) fn marked_bad(f: &Files, now: SystemTime) -> Option<String> {
    strikes(f, now)
        .filter(|(count, _)| *count >= STRIKES)
        .map(|(_, why)| why)
}

/// Count one failure of the state in its marker, and remove its files.
/// A load that works removes the marker ([`touch`]).
pub fn mark_bad(f: &Files, why: &str) {
    let count = strikes(f, SystemTime::now()).map_or(0, |(c, _)| c);
    let _ = std::fs::write(
        &f.bad,
        format!("{}\n{}\n", count.saturating_add(1), why.replace('\n', " ")),
    );
    let _ = std::fs::remove_file(&f.meta);
    let _ = std::fs::remove_file(&f.state);
}

/// What a run does with the state of its fingerprint.
pub enum Plan {
    /// Load the state.
    Load(Meta),
    /// Boot the guest of the state and save it, under this lock.
    Save {
        /// Held until the end of the run.
        _lock: Lock,
    },
    /// Boot as a run with no state, for this reason.
    Boot(String),
}

/// Decide. `need` is the most bytes that a save can write.
pub fn plan(f: &Files, fingerprint: &str, need: u64) -> Plan {
    if let Some(why) = marked_bad(f, SystemTime::now()) {
        return Plan::Boot(format!(
            "{STRIKES} runs in a row could not use a saved state (the last one: {why}); podvm \
             tries again after one day, or after the removal of {}",
            f.bad.display()
        ));
    }
    if let Ok(meta) = valid(f, fingerprint) {
        return Plan::Load(meta);
    }
    // QEMU reads a comma in a `file:` URI as the start of an option.
    if f.state.to_str().is_none_or(|p| p.contains(',')) {
        return Plan::Boot(format!(
            "the path {} is not UTF-8 or has a comma, which QEMU cannot take",
            f.state.display()
        ));
    }
    let dir = f.state.parent().unwrap_or(Path::new("/"));
    let lock = match Lock::try_exclusive(&f.lock) {
        Ok(Some(l)) => l,
        Ok(None) => return Plan::Boot("another run saves the state now".to_string()),
        Err(e) => return Plan::Boot(format!("the lock {}: {e}", f.lock.display())),
    };
    // Under the lock: another run can have saved the state since the check.
    if let Ok(meta) = valid(f, fingerprint) {
        return Plan::Load(meta);
    }
    sweep(dir, f);
    if let Err(e) = podvm_image::space::require(
        &dir.to_string_lossy(),
        "the saved state of a guest",
        need,
        2,
        64 << 20,
    ) {
        return Plan::Boot(format!("no room for a state: {e}"));
    }
    Plan::Save { _lock: lock }
}

/// Under the lock of `f`: remove what no run uses. Each partial file of
/// this state (its writer held this lock); each partial file of another
/// state that is older than a day; each lock of a day with no record of a
/// state; and each state of the same image past the [`KEEP`] that were
/// used last.
pub(super) fn sweep(dir: &Path, f: &Files) {
    let own = f
        .state
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();
    // `state-DIGEST-`: the stem is that and the digits of the key.
    let image = own[..own.len().saturating_sub(KEY_DIGITS)].to_string();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let now = SystemTime::now();
    let age = |p: &Path| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| now.duration_since(t).ok())
            .unwrap_or_default()
    };
    let mut metas: Vec<(Duration, PathBuf)> = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.starts_with("state-") {
            continue;
        }
        let partial = name.contains(".part-");
        // The lock of a state that went: old, and with no record beside it.
        let orphan = name.ends_with(".lock")
            && !name.starts_with(&own)
            && !p.with_extension(RECORD).exists()
            && !p.with_extension(OLD_RECORD).exists()
            && age(&p) > BAD_HOLDS;
        if orphan || (partial && (name.starts_with(&own) || age(&p) > BAD_HOLDS)) {
            let _ = std::fs::remove_file(&p);
        } else if !partial
            && name.starts_with(&image)
            && !name.starts_with(&own)
            && (name.ends_with(&format!(".{RECORD}")) || name.ends_with(&format!(".{OLD_RECORD}")))
        {
            metas.push((age(&p), p));
        }
    }
    metas.sort();
    // This state is one of the kept ones.
    for (_, meta) in metas.into_iter().skip(KEEP - 1) {
        for ext in ["vm", RECORD, OLD_RECORD, "bad", "lock"] {
            let _ = std::fs::remove_file(meta.with_extension(ext));
        }
    }
}

/// Record a use of the state: [`sweep`] keeps the states that were used
/// last, and the count of its failures starts again.
pub fn touch(f: &Files) {
    if let Ok(file) = std::fs::OpenOptions::new().write(true).open(&f.meta) {
        let _ = file.set_modified(SystemTime::now());
    }
    let _ = std::fs::remove_file(&f.bad);
}
