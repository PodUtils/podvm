//! TODO/cli.md T-1825: the end of a detached run in its `--report`. `run
//! -d` writes the report at the start; the launcher, and `run -d` again
//! after its write, put the end of the record into it. Each writes the
//! same values, so the order of the two does not matter.

use crate::Container;
use pod_supervise::end::Reason;
use std::path::Path;

/// Write the end of `c` into the report at `path`. A record with no end
/// yet, or a path with no report yet, changes nothing.
pub fn finish(path: &Path, c: &Container) -> Result<(), String> {
    let Some(code) = c.exit_code else {
        return Ok(());
    };
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut r: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let end = c.end.as_ref();
    let signalled = end.is_some_and(|e| matches!(e.reason, Reason::Signal | Reason::Wall));
    r["exit_code"] = code.into();
    r["signal"] = if signalled && code > 128 {
        (code - 128).into()
    } else {
        serde_json::Value::Null
    };
    r["timed_out"] = end.is_some_and(|e| e.reason == Reason::Wall).into();
    r["ended"] = c.finished_at.clone().into();
    // PodUtils/podrunner#296 ask 6 (TODO/supervise.md T-1969): the OOM
    // kills of the leaf of the run, where the run has one.
    if let Some(n) = own_leaf().and_then(|l| oom_kills_of(&l)) {
        r["oom_kills"] = n.into();
    }
    if let Some(e) = end {
        r["wall_ms"] = e.run_ms.into();
        r["user_ms"] = e.user_ms.into();
        r["system_ms"] = e.system_ms.into();
        r["max_rss_kib"] = e.max_rss_kib.into();
    }
    let body = serde_json::to_string_pretty(&r).map_err(|e| e.to_string())? + "\n";
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.end.tmp", std::process::id()));
    std::fs::write(&tmp, body).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{}: {e}", path.display())
    })
}

/// The count `oom_kill` of `memory.events` of the cgroup `dir`, or `None`
/// where the file or the line is not there.
pub fn oom_kills_of(dir: &Path) -> Option<u64> {
    std::fs::read_to_string(dir.join("memory.events"))
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("oom_kill "))
        .and_then(|n| n.trim().parse().ok())
}

/// The cgroup of this process where it is a leaf of podvm (`podvm-ID`,
/// TODO/cli.md T-1747), else `None`: a cgroup of another owner has no
/// count of this run.
pub fn own_leaf() -> Option<std::path::PathBuf> {
    let text = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let rel = text.lines().find_map(|l| l.strip_prefix("0::"))?;
    let leaf = Path::new("/sys/fs/cgroup").join(rel.trim_start_matches('/'));
    let name = leaf.file_name()?.to_str()?;
    (name.starts_with("podvm-") && !rel.contains("..")).then_some(leaf)
}

#[cfg(test)]
mod tests {
    /// T-1969: the count of `oom_kill`, and none where the line is not there.
    #[test]
    fn the_oom_count_comes_from_memory_events() {
        let d = std::env::temp_dir().join(format!("podvm-oom-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        assert_eq!(super::oom_kills_of(&d), None);
        std::fs::write(
            d.join("memory.events"),
            "low 0\nhigh 0\nmax 4\noom 2\noom_kill 2\n",
        )
        .unwrap();
        assert_eq!(super::oom_kills_of(&d), Some(2));
        std::fs::write(d.join("memory.events"), "low 0\n").unwrap();
        assert_eq!(super::oom_kills_of(&d), None);
        let _ = std::fs::remove_dir_all(&d);
    }
}
