/// Create the host memo file where absent, leaving what is there alone.
///
/// An existing memo is a container's ownership memory and must survive a
/// second entry, so this never truncates. A fresh file is empty, which reads
/// as "no record yet" rather than as degraded.
pub fn ensure_memo_file(path: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map(|_| ())
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Open the host memo for handing to the payload.
///
/// `O_APPEND` so the kernel serialises one-record writes from concurrent
/// payload processes with no lock in the interposer. The returned handle must
/// outlive the spawn: the child dups it to `MEMO_CHILD_FD` before the
/// `chroot`.
pub fn open_memo(path: &std::path::Path) -> Result<std::fs::File, String> {
    ensure_memo_file(path)?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Emulation tally counts, T-0708: how many times the preloaded tier told
/// the payload a compatibility story (a regular file for a node, a mount
/// success, an unshare success, a stripped clone) rather than the kernel's
/// answer.
///
/// T-0413 adds the fifth story: a `/proc/self` answer where no procfs is
/// mounted (a descriptor duplicate, a `readlink` synthesis, an exe
/// passthrough, a mount-table serve).
///
/// Read from the ownership memo file beside the container record: the
/// interposer appends 32-byte tally records (device `u64::MAX`, the
/// operation in the inode word, mount path chains behind their header)
/// through the same descriptor it writes ownership intent through. Counts
/// count complete records only: a mount header whose path chain breaks is
/// dropped, never half-counted.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct EmulatedCounts {
    pub mknod: u64,
    pub mount: u64,
    pub unshare: u64,
    pub clone: u64,
    pub procfs: u64,
}

/// Tally record kinds, in the inode word beside `u64::MAX`. One fact in two
/// homes with `podvm-interpose`'s `emulate` module; the values agree by
/// driving the behaviour end to end.
pub(super) const TALLY_MKNOD: u64 = 1;
pub(super) const TALLY_MOUNT: u64 = 2;
pub(super) const TALLY_UNSHARE: u64 = 3;
pub(super) const TALLY_CLONE: u64 = 4;
pub(super) const TALLY_PROC: u64 = 5;

fn tally_u64(b: &[u8]) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[..8]);
    u64::from_le_bytes(a)
}

fn tally_u32(b: &[u8]) -> u32 {
    let mut a = [0u8; 4];
    a.copy_from_slice(&b[..4]);
    u32::from_le_bytes(a)
}

/// Scan the memo file for complete tally records, T-0708.
///
/// Ownership records pass through untouched (their device word is a real
/// one). A mount header opens a chain of exactly the continuations it
/// names; anything else, including another header, drops the open chain.
/// The scan streams in fixed chunks, so a memo grown large by a busy
/// payload cannot grow this reader's memory with it.
pub fn emulated_counts(memo: &std::path::Path) -> EmulatedCounts {
    const REC: usize = 32;
    const CHUNK_RECS: usize = 4096;
    let mut out = EmulatedCounts::default();
    let Ok(mut f) = std::fs::File::open(memo) else {
        return out;
    };
    use std::io::Read;
    let mut buf = vec![0u8; REC * CHUNK_RECS];
    // Continuations still owed to an open mount header. Zero means none.
    let mut pending: u64 = 0;
    loop {
        // Fill to a record boundary: a record never straddles two reads,
        // so the scan below needs no carry-over.
        let mut have = 0usize;
        while have < buf.len() {
            match f.read(&mut buf[have..]) {
                Ok(0) => break,
                Ok(n) => have += n,
                Err(_) => break,
            }
        }
        if have == 0 {
            break;
        }
        let mut at = 0usize;
        while at + REC <= have {
            let r = &buf[at..at + REC];
            if tally_u64(&r[0..8]) != u64::MAX {
                // An ownership record. It also breaks any open chain: the
                // writer places each chain contiguously, so anything else
                // between a header and its continuations is a torn write.
                pending = 0;
                at += REC;
                continue;
            }
            match tally_u64(&r[8..16]) {
                TALLY_MKNOD => {
                    pending = 0;
                    out.mknod += 1;
                }
                TALLY_MOUNT => {
                    let want = tally_u32(&r[20..24]) as u64 + tally_u32(&r[24..28]) as u64;
                    if want == 0 {
                        out.mount += 1;
                    } else {
                        pending = want;
                    }
                }
                TALLY_UNSHARE => {
                    pending = 0;
                    out.unshare += 1;
                }
                TALLY_CLONE => {
                    pending = 0;
                    out.clone += 1;
                }
                TALLY_PROC => {
                    pending = 0;
                    out.procfs += 1;
                }
                // A continuation where one is owed feeds the open chain;
                // anywhere else (stray, or an unknown kind) it is skipped,
                // and an unknown header drops the open chain with it.
                0 => {
                    if pending > 0 {
                        pending -= 1;
                        if pending == 0 {
                            out.mount += 1;
                        }
                    }
                }
                _ => {
                    pending = 0;
                }
            }
            at += REC;
        }
        if have < buf.len() {
            break;
        }
    }
    out
}

/// The variable that names the memo's file to the interposer: `DEV:INO` of
/// the host file, in decimal (TODO/interpose.md T-1928,
/// PodUtils/podrunner#197). The interposer uses descriptor
/// [`super::MEMO_CHILD_FD`] only while it is this file: a payload that
/// closes 17 and opens a file of its own there keeps that file intact.
///
/// One fact in two homes with `podvm-interpose`'s `MEMO_ID_VAR`, as
/// [`super::MEMO_FD_VAR`].
pub const MEMO_ID_VAR: &str = "PODVM_MEMO_ID";

/// Make the memo at `path` where it is missing, and hand it in `env`:
/// `PODVM_MEMO_FD` and `PODVM_MEMO_ID`, each in place of an older value.
pub fn hand_memo(env: &mut Vec<String>, path: &std::path::Path) -> Result<(), String> {
    ensure_memo_file(path)?;
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    hand_memo_meta(env, &meta, path);
    Ok(())
}

/// [`hand_memo`] for a memo that is open from `path`: the identity of the
/// file that the payload gets on its descriptor.
pub fn hand_memo_file(
    env: &mut Vec<String>,
    memo: &std::fs::File,
    path: &std::path::Path,
) -> Result<(), String> {
    let meta = memo
        .metadata()
        .map_err(|e| format!("the ownership memo: {e}"))?;
    hand_memo_meta(env, &meta, path);
    Ok(())
}

/// The variable that names the memo's host path to the interposer, for a
/// process whose descriptor a parent closed (TODO/interpose.md T-2000). One
/// fact in two homes with `podvm-interpose`'s `MEMO_PATH_VAR`.
pub const MEMO_PATH_VAR: &str = "PODVM_MEMO_PATH";

/// The variable through which podvm names its own host path to the
/// interposer of the loader family (`podvm_dispatch::SELF_VAR`).
pub const DISPATCH_SELF_VAR: &str = "PODVM_DISPATCH_SELF";

/// TODO/enter.md T-2054 (OQ-08): remove from the environment of an entry
/// with a root (the namespace and the chroot rung, the ladder) each variable
/// that names a host path for the interposer of the loader family: the
/// memo's path (T-2000) and podvm's own (T-1724). Neither resolves in the
/// root, and the payload read the paths of the store and of podvm in them.
pub fn drop_host_paths(env: &mut Vec<String>) {
    let host = [MEMO_PATH_VAR, DISPATCH_SELF_VAR];
    env.retain(|e| {
        !host
            .iter()
            .any(|v| e.strip_prefix(v).is_some_and(|r| r.starts_with('=')))
    });
}

fn hand_memo_meta(env: &mut Vec<String>, meta: &std::fs::Metadata, path: &std::path::Path) {
    use std::os::unix::fs::MetadataExt;
    let (fd, id, at) = (
        format!("{}=", super::MEMO_FD_VAR),
        format!("{MEMO_ID_VAR}="),
        format!("{MEMO_PATH_VAR}="),
    );
    env.retain(|e| !e.starts_with(&fd) && !e.starts_with(&id) && !e.starts_with(&at));
    env.push(super::memo_fd_env());
    env.push(format!("{id}{}:{}", meta.dev(), meta.ino()));
    // An absolute path in UTF-8 only; else the descriptor is the one way.
    let abs = std::path::absolute(path).ok();
    if let Some(p) = abs.as_deref().and_then(|p| p.to_str()) {
        env.push(format!("{at}{p}"));
    }
}

/// The ids of the payload inside the user namespace of the namespace rung,
/// `UID:GID` in decimal, from the record of `run -d` and `create`
/// (TODO/enter.md T-1927, PodUtils/podrunner#196). The launcher gives them
/// to `pod_enter::Plan::userns_ids` (podcrates `v0.62.0`); with no value the
/// payload is uid 0 of the namespace, as before.
pub const USERNS_IDS_VAR: &str = "PODVM_USERNS_IDS";

/// The ids of [`USERNS_IDS_VAR`] in `env`, or `None`.
pub fn userns_ids_of(env: &[String]) -> Option<(u32, u32)> {
    let v = env
        .iter()
        .rev()
        .find_map(|e| e.strip_prefix(&format!("{USERNS_IDS_VAR}=")))?;
    let (u, g) = v.split_once(':')?;
    Some((u.parse().ok()?, g.parse().ok()?))
}
