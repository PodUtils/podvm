//! The spare tree of an image, TODO/image.md T-1781.
//!
//! A run copies the extracted image into its own tree (T-1709), which is
//! the cost of a large image: 66 % of `run --rm golang true` on the cloud
//! host (experiment 442). After a run takes its tree, podvm fills the next
//! one for the same image in a detached `podvm tree-prefill`, and the next
//! run of the image takes it with one rename.
//!
//! ⭐ The prefill starts at podvm's exit ([`spawn_pending`]), not during the
//! run: the launcher of a rung kills each process of its tree at the end
//! of the payload, and a prefill must not compete with the payload. It runs
//! at the lowest CPU priority and in the idle I/O class, and holds a *fill*
//! lock of its own, which a removal ignores: a copy that loses its extraction
//! fails and removes its partial tree. `PODVM_SPARE_TREE=0` turns the spare
//! off: a spare takes the disk of one more tree for each image that ran.

use std::path::PathBuf;
use std::sync::Mutex;

/// The image whose spare podvm fills at its exit: the store and the digest.
static PENDING: Mutex<Option<(PathBuf, String)>> = Mutex::new(None);

/// Whether the spare is on (`PODVM_SPARE_TREE` is not `0`).
pub(crate) fn enabled() -> bool {
    std::env::var_os("PODVM_SPARE_TREE").is_none_or(|v| v != "0")
}

/// Fill the spare of `digest` at podvm's exit.
pub(crate) fn note(store: &podvm_image::Store, digest: &str) {
    if let Ok(mut p) = PENDING.lock() {
        *p = Some((store.root().to_path_buf(), digest.to_string()));
    }
}

/// At podvm's exit: start the detached prefill of the noted image. In the
/// user namespace of a run (TODO/probe.md T-1909), each other process of
/// the namespace ends then: pid 1 waits for the prefill only.
pub(crate) fn spawn_pending() {
    if cfg!(test) {
        return;
    }
    let helper = spawn_prefill();
    crate::userns::sweep(helper);
}

fn spawn_prefill() -> Option<u32> {
    use std::io::Write;
    use std::os::unix::process::CommandExt;
    let pending = PENDING.lock().ok().and_then(|mut p| p.take());
    let trash = podvm_extract::tree::trashed();
    let (root, digest) = match (pending, trash) {
        (Some((root, digest)), _) => (root, digest),
        (None, Some(root)) => (root, String::new()),
        (None, None) => return None,
    };
    let job = job_bytes(&root, &digest)?;
    let me = std::env::current_exe().ok()?;
    let null = std::process::Stdio::null;
    // TODO/enter.md T-2054 item 5: the store and the digest go on a pipe,
    // not in argv or the environment, so no other process reads a path of
    // the store in `/proc/PID/cmdline` of the helper.
    let mut child = std::process::Command::new(me)
        .env_clear()
        .arg(VERB)
        .stdin(std::process::Stdio::piped())
        .stdout(null())
        .stderr(null())
        .process_group(0)
        .spawn()
        .ok()?;
    let id = child.id();
    // The job is at most MAX_JOB bytes, less than the capacity of a pipe,
    // so the write does not wait for the helper.
    let sent = child
        .stdin
        .take()
        .is_some_and(|mut w| w.write_all(&job).is_ok());
    if !sent {
        // The helper reads a short job and exits 1; it holds nothing yet.
        let _ = child.kill();
    }
    Some(id)
}

/// The verb of the helper.
pub(crate) const VERB: &str = "tree-prefill";

/// The largest job on the pipe: a path of the store and a digest.
const MAX_JOB: usize = 8192;

/// The job of the helper: the store root, a NUL, the digest (empty for the
/// trash only). `None` where it does not fit [`MAX_JOB`] or the root holds a
/// NUL.
fn job_bytes(root: &std::path::Path, digest: &str) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    let r = root.as_os_str().as_bytes();
    if r.is_empty() || r.contains(&0) || digest.as_bytes().contains(&0) {
        return None;
    }
    let mut job = Vec::with_capacity(r.len() + 1 + digest.len());
    job.extend_from_slice(r);
    job.push(0);
    job.extend_from_slice(digest.as_bytes());
    (job.len() <= MAX_JOB).then_some(job)
}

/// The store root and the digest of a job, or why the job is refused.
fn parse_job(job: &[u8]) -> Result<(PathBuf, Option<String>), &'static str> {
    use std::os::unix::ffi::OsStrExt;
    if job.len() > MAX_JOB {
        return Err("the job is too long");
    }
    let Some(nul) = job.iter().position(|b| *b == 0) else {
        return Err("the job has no NUL after the store");
    };
    let (root, digest) = (&job[..nul], &job[nul + 1..]);
    if root.first() != Some(&b'/') {
        return Err("the store is not an absolute path");
    }
    if digest.contains(&0) {
        return Err("the digest holds a NUL");
    }
    let digest = match std::str::from_utf8(digest) {
        Ok("") => None,
        Ok(d) => Some(d.to_string()),
        Err(_) => return Err("the digest is not UTF-8"),
    };
    let root = PathBuf::from(std::ffi::OsStr::from_bytes(root));
    Ok((root, digest))
}

/// Read the job from standard input: at most [`MAX_JOB`] bytes.
fn read_job() -> Result<(PathBuf, Option<String>), &'static str> {
    use std::io::{IsTerminal, Read};
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Err("standard input is a terminal; podvm gives the job on a pipe");
    }
    let mut job = Vec::new();
    if stdin
        .lock()
        .take(MAX_JOB as u64 + 1)
        .read_to_end(&mut job)
        .is_err()
    {
        return Err("standard input could not be read");
    }
    parse_job(&job)
}

/// `podvm tree-prefill`, an internal verb: read the store and the digest
/// from standard input ([`job_bytes`]), empty the trash of the store, then
/// fill the spare of the image, at the lowest priority. Exit 0 where each
/// step held; 125 (the flag error) for an argument or a job that is not valid.
pub(crate) fn main(args: &[String]) -> i32 {
    if !args.is_empty() {
        eprintln!("podvm {VERB}: an internal verb takes no argument; podvm gives its job on standard input");
        return podvm_image::error::EXIT_FLAG_ERROR;
    }
    let (root, digest) = match read_job() {
        Ok(j) => j,
        Err(why) => {
            eprintln!("podvm {VERB}: {why}");
            return podvm_image::error::EXIT_FLAG_ERROR;
        }
    };
    unsafe extern "C" {
        fn setpriority(which: i32, who: u32, prio: i32) -> i32;
        fn syscall(n: i64, ...) -> i64;
    }
    // SAFETY: plain system calls on this process. 251 is ioprio_set on
    // x86_64 and 30 on aarch64; class 3 (idle) << 13.
    unsafe {
        setpriority(0, 0, 19);
        let nr = if cfg!(target_arch = "aarch64") {
            30
        } else {
            251
        };
        syscall(nr, 1i64, 0i64, 3i64 << 13);
    }
    let Ok(store) = podvm_image::Store::open(&root) else {
        return 1;
    };
    podvm_extract::tree::empty_trash(&store);
    let Some(digest) = digest else {
        return 0;
    };
    let Some(record) = store
        .list()
        .ok()
        .and_then(|l| l.into_iter().find(|r| r.manifest_digest == digest))
    else {
        return 1;
    };
    // ⛔ A fill hold, not a payload hold (T-0204 is the payload's). An `rmi`
    // or a `prune` that lands in this copy must go ahead: this fill is a
    // cache, and a source that went under it makes `fill` remove the partial
    // tree it had begun rather than refuse the removal.
    let Ok(_held) = store.hold_fill(&record) else {
        return 1;
    };
    if !podvm_extract::is_extracted(&store, &digest) {
        return 1;
    }
    match podvm_extract::tree::prefill(&store, &digest) {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

#[cfg(test)]
#[path = "spare_tests.rs"]
mod tests;
