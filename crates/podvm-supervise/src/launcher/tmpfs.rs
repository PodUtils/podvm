//! The size-limited tmpfs mounts of a run in its record (TODO/machine.md
//! T-1968, TODO/deps.md T-2102): `--tmpfs GUEST:size=N` on the namespace
//! rung, as `Plan::tmpfs` of pod-enter (podcrates `v0.86.0`).
//!
//! The record carries them in `PODVM_TMPFS` (`GUEST:size=BYTES`, comma
//! separated), as `PODVM_BINDS` carries the volumes. The foreground run and
//! the launcher read them into `Plan::tmpfs` and drop the variable from the
//! env of the payload. `exec` joins the mount namespace of the run, which
//! holds each mount with its limit, and mounts nothing.

use std::num::NonZeroU64;

use podvm_enter::TmpfsMount;

/// The variable of the record.
pub const VAR: &str = "PODVM_TMPFS";

/// The value of [`VAR`] for `mounts`, or `None` where there is none. The
/// caller refused a guest path with a comma or a colon before.
pub fn value(mounts: &[TmpfsMount]) -> Option<String> {
    (!mounts.is_empty()).then(|| {
        mounts
            .iter()
            .map(podvm_enter::tmpfs_label)
            .collect::<Vec<_>>()
            .join(",")
    })
}

/// The tmpfs mounts of [`VAR`] in `env`. A part that does not read as
/// `GUEST:size=BYTES` with a limit over 0 is an `Err` that names it: a
/// record with a damaged limit never runs with no limit.
pub fn of(env: &[String]) -> Result<Vec<TmpfsMount>, String> {
    let prefix = format!("{VAR}=");
    let Some(v) = env.iter().find_map(|e| e.strip_prefix(&prefix)) else {
        return Ok(Vec::new());
    };
    v.split(',')
        .map(|part| {
            part.rsplit_once(":size=")
                .and_then(|(guest, n)| {
                    let bytes = NonZeroU64::new(n.parse().ok()?)?;
                    Some(TmpfsMount {
                        guest: guest.to_string(),
                        bytes,
                    })
                })
                .ok_or_else(|| {
                    format!(
                        "the record names the tmpfs {part:?}, which is not GUEST:size=BYTES \
                         (TODO/machine.md T-1968)"
                    )
                })
        })
        .collect()
}

/// Drop [`VAR`] from the env of the payload.
pub fn drop_var(env: &mut Vec<String>) {
    let prefix = format!("{VAR}=");
    env.retain(|e| !e.starts_with(&prefix));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One rule for each value of the record: a value that `value` writes
    /// reads back the same, and each damaged part is an `Err` that names it.
    #[test]
    fn the_record_reads_back_each_tmpfs_or_names_the_bad_part() {
        let t = |g: &str, n: u64| TmpfsMount {
            guest: g.to_string(),
            bytes: NonZeroU64::new(n).unwrap(),
        };
        let list = vec![t("/w", 64 << 20), t("/var/cache", 4096)];
        let env = vec![
            "A=1".to_string(),
            format!("{VAR}={}", value(&list).unwrap()),
        ];
        assert_eq!(of(&env).unwrap(), list);
        assert_eq!(value(&[]), None);
        assert_eq!(of(&["A=1".to_string()]).unwrap(), Vec::new());
        for bad in ["/w", "/w:size=0", "/w:size=", "/w:size=1x", "/w:size=64m"] {
            let e = of(&[format!("{VAR}={bad}")]).unwrap_err();
            assert!(e.contains(bad) && e.contains("T-1968"), "{bad}: {e}");
        }
        let mut env = env;
        drop_var(&mut env);
        assert_eq!(env, vec!["A=1".to_string()]);
    }
}
