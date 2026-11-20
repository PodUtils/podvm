//! The per-run directories of the machine tier under the store.

use std::path::Path;

/// TODO/machine.md T-1906: remove each `run-PID-*` directory under `dir`
/// whose PID is not a running process: a podvm that was killed never
/// reached the removal at the end of its boot. A live PID keeps its
/// directory, also where the kernel gave the PID to another process.
pub(super) fn sweep(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name();
        let Some(pid) = name
            .to_str()
            .and_then(|n| n.strip_prefix("run-"))
            .and_then(|r| r.split('-').next())
            .and_then(|p| p.parse::<u32>().ok())
        else {
            continue;
        };
        if pid != std::process::id() && !Path::new("/proc").join(pid.to_string()).exists() {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

#[cfg(test)]
mod sweep_tests {
    use super::*;

    #[test]
    fn machine_run_dirs_of_a_dead_pid_go() {
        let d = std::env::temp_dir().join(format!("podvm-t1906-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        // A PID above the kernel's limit runs nothing.
        let dead = d.join("run-4194999-abcdef01");
        let live = d.join(format!("run-{}-abcdef02", std::process::id()));
        let other = d.join("base-1.cpio");
        for p in [&dead, &live] {
            std::fs::create_dir_all(p).unwrap();
        }
        std::fs::write(&other, b"x").unwrap();
        sweep(&d);
        assert!(!dead.exists(), "the directory of a dead PID stays");
        assert!(live.exists(), "the directory of a live PID went");
        assert!(other.exists(), "a file that is no run directory went");
        let _ = std::fs::remove_dir_all(&d);
    }
}
