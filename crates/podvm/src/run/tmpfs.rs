//! `--tmpfs GUEST:size=N`: a tmpfs in memory with a byte limit on the
//! namespace rung (TODO/machine.md T-1968, TODO/deps.md T-2102; podcrates
//! `v0.86.0`, `Plan::tmpfs`, PodUtils/podvm#667).
//!
//! A `--tmpfs` with `size=` is a mount of pod-enter, not a directory of the
//! store: `run`, `run -d` and `create` hold it, and a write past N gets
//! `ENOSPC`. Only the namespace rung has a mount namespace for it. Each
//! other entry (the chroot rung, the userland rung, a rung that
//! `PODVM_MODE` forces, the init of the image) refuses the run by name with
//! 125, and podvm never puts a directory with no limit in its place. On the
//! syscall tier, `--tmpfs /tmp:size=N` stays the cap of its handler
//! (TODO/enter.md T-1657). A `--tmpfs` with no `size=` stays the directory
//! of T-1655.

use std::num::NonZeroU64;

use podvm_enter::TmpfsMount;
use podvm_image::error::EXIT_RUNTIME_ERROR;

use super::Opts;

/// The `size=VALUE` of `--tmpfs PATH`: the cap of `/tmp` on the syscall
/// tier and the limit of the mount of PATH. `size=0` is docker's "no
/// limit": PATH stays a directory. `Err` is why VALUE is not a size.
pub(super) fn sized(path: &str, value: &str, o: &mut Opts) -> Result<(), String> {
    o.tmpfs_sizes.retain(|(p, _)| p != path);
    if !value.is_empty() && value.trim_start_matches('0').is_empty() {
        return Ok(());
    }
    let n = crate::tier::parse_mem(value).ok_or_else(|| format!("size={value:?} is not a size"))?;
    if path == "/tmp" {
        o.tmp_size = Some(n);
    }
    o.tmpfs_sizes.push((path.to_string(), n));
    Ok(())
}

/// Is `a` the path `b` or a path under it?
fn under(a: &str, b: &str) -> bool {
    let b = b.trim_end_matches('/');
    a == b || a.strip_prefix(b).is_some_and(|r| r.starts_with('/'))
}

/// The tmpfs mounts of the run: each `--tmpfs` with a limit leaves the
/// directories of [`super::add_tmpfs`]. A path with a comma (the separator
/// of the record) and a path at, under or over a volume refuse by name:
/// the volume would hide the mount, or the mount the volume.
pub(super) fn take(verb: &str, syscall: bool, o: &mut Opts) -> Result<Vec<TmpfsMount>, i32> {
    let refuse = |guest: &str, n: u64, why: &str| {
        eprintln!(
            "podvm {verb}: --tmpfs {guest}:size={n} is refused: {why}; podvm does not give a \
             directory with no limit in its place (TODO/machine.md T-1968)"
        );
        EXIT_RUNTIME_ERROR
    };
    let mut out = Vec::new();
    for (guest, n) in &o.tmpfs_sizes {
        if syscall {
            if guest != "/tmp" {
                let why = "the syscall tier has no mount namespace, and it caps /tmp alone";
                return Err(refuse(guest, *n, why));
            }
            continue; // the cap of the handler (T-1657)
        }
        if guest.contains(',') {
            return Err(refuse(guest, *n, "the path holds a comma"));
        }
        if o.ro_volumes.contains(guest) {
            return Err(refuse(
                guest,
                *n,
                "pod-enter mounts a tmpfs with a limit writable, not ro",
            ));
        }
        if let Some(v) = o
            .volumes
            .iter()
            .find(|v| under(&v.guest, guest) || under(guest, &v.guest))
        {
            let why = format!("it is at, under or over the volume of {}", v.guest);
            return Err(refuse(guest, *n, &why));
        }
        let Some(bytes) = NonZeroU64::new(*n) else {
            continue; // parse_mem gives no 0
        };
        out.push(TmpfsMount {
            guest: guest.clone(),
            bytes,
        });
    }
    o.tmpfs.retain(|t| !out.iter().any(|m| &m.guest == t));
    Ok(out)
}

/// Why the entry of this run cannot hold `mounts`, or none: only the
/// namespace rung of pod-enter mounts a tmpfs with a limit. `rung` is the
/// word of the banner; `userland` an entry with no chroot; `forced` the
/// rung of `PODVM_MODE`; `init_image` the init of the image.
pub(super) fn wrong_entry(
    mounts: &[TmpfsMount],
    rung: &str,
    userland: bool,
    forced: Option<&str>,
    init_image: bool,
) -> Option<String> {
    let first = mounts.first()?;
    let why = if init_image {
        "the init of the image (--podvm-init=image) mounts no tmpfs with a limit".to_string()
    } else if let Some(mode) = forced {
        format!("PODVM_MODE forces the {mode} rung, which has no mount namespace of its own")
    } else if userland || rung != "namespace" {
        format!(
            "it needs the namespace rung, and this host gives the {rung} rung, which has no \
             mount namespace of its own"
        )
    } else {
        return None;
    };
    Some(format!(
        "--tmpfs {} is refused: {why}; podvm does not give a directory with no limit in its \
         place (TODO/machine.md T-1968)",
        podvm_enter::tmpfs_label(first)
    ))
}

/// The record of the mounts in `env` ([`podvm_supervise::launcher::tmpfs`]),
/// and one line of the banner for each: a tmpfs is memory.
pub(super) fn push(env: &mut Vec<String>, mounts: &[TmpfsMount], banner: &mut String) {
    use podvm_supervise::launcher::tmpfs;
    tmpfs::drop_var(env);
    if let Some(v) = tmpfs::value(mounts) {
        env.push(format!("{}={v}", tmpfs::VAR));
    }
    for m in mounts {
        banner.push_str(&format!(
            "podvm: --tmpfs {}: a tmpfs in memory of at most {} bytes on the namespace rung; a \
             write past it gets ENOSPC (TODO/machine.md T-1968)\n",
            m.guest, m.bytes
        ));
    }
}

/// The words of each mount in `env` for `inspect` and `--report`:
/// `GUEST:size=BYTES (memory)`.
pub(crate) fn words(env: &[String]) -> Vec<String> {
    podvm_supervise::launcher::tmpfs::of(env)
        .unwrap_or_default()
        .iter()
        .map(|m| format!("{} (memory)", podvm_enter::tmpfs_label(m)))
        .collect()
}

#[cfg(test)]
mod tests;
