//! The root disk of an image in the store: built once for each image
//! digest and format, then only read (TODO/machine.md T-1994).
//!
//! The rules of the cache:
//!
//! * the name holds the digest and [`FORMAT`]: a change of the writer
//!   bumps the format, so a disk of an older writer is never read;
//! * one run builds under the lock of the digest, into a name of its own
//!   (a random part: a pid is not unique across PID namespaces), syncs the
//!   file and gives it its name last: a reader finds a whole disk or none.
//!   Each other run waits for the lock or for the disk, within a bound;
//! * the last block of the file is a trailer: the count of blocks of the
//!   file system and the SHA-256 of its bytes. A run uses a disk only where
//!   the trailer, the size and the superblock agree, and where the file is
//!   the one that the last check saw (its device, inode, size, change time
//!   and modification time in the `.ok` record). Else the run hashes the
//!   disk again; a disk whose hash is not its trailer's is built again;
//! * the disk is mode 0444, and each guest attaches it read-only.

use std::os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::ext::{self, BLOCK};
use super::{image, tree};

/// The version of the root disk format.
pub const FORMAT: u32 = 1;
const MAGIC: &str = "podvm-root-disk";
/// The bound of a wait for the build of another run.
const WAIT: Duration = Duration::from_secs(30 * 60);
const POLL: Duration = Duration::from_millis(250);

/// The file of the root disk of `digest` in the store, and its lock and
/// check record.
pub fn path(store_root: &Path, digest: &str) -> PathBuf {
    let hex = digest.trim_start_matches("sha256:");
    store_root
        .join("machine")
        .join(format!("root-{hex}-v{FORMAT}.img"))
}

fn sibling(p: &Path, ext: &str) -> PathBuf {
    p.with_extension(ext)
}

/// The trailer block of a file system of `blocks` blocks with `sha` (hex).
pub fn trailer(blocks: u64, sha: &str) -> Vec<u8> {
    let mut t = format!("{MAGIC} {FORMAT}\nblocks {blocks}\nsha256 {sha}\n").into_bytes();
    t.resize(BLOCK as usize, 0);
    t
}

/// The blocks and the SHA-256 that the trailer of `p` names, after a check
/// of the size of the file and of the superblock. Err is what is wrong.
pub fn read_trailer(p: &Path) -> Result<(u64, String), String> {
    let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    if len < 2 * BLOCK || len % BLOCK != 0 {
        return Err(format!("{len} bytes are no whole disk and trailer"));
    }
    let mut t = vec![0u8; BLOCK as usize];
    f.read_exact_at(&mut t, len - BLOCK)
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&t[..t.iter().position(|&b| b == 0).unwrap_or(t.len())])
        .into_owned();
    let mut lines = text.lines();
    let head = format!("{MAGIC} {FORMAT}");
    if lines.next() != Some(head.as_str()) {
        return Err("no trailer of this format".to_string());
    }
    let blocks = lines
        .next()
        .and_then(|l| l.strip_prefix("blocks "))
        .and_then(|b| b.parse::<u64>().ok())
        .ok_or("the trailer has no count of blocks")?;
    let sha = lines
        .next()
        .and_then(|l| l.strip_prefix("sha256 "))
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("the trailer has no SHA-256")?
        .to_string();
    if (blocks + 1) * BLOCK != len {
        return Err(format!(
            "the trailer names {blocks} blocks, and the file has {len} bytes"
        ));
    }
    let mut sb = [0u8; 12];
    f.read_exact_at(&mut sb, 1024).map_err(|e| e.to_string())?;
    let count = u64::from(u32::from_le_bytes([sb[4], sb[5], sb[6], sb[7]]));
    f.read_exact_at(&mut sb[..2], 1024 + 56)
        .map_err(|e| e.to_string())?;
    if sb[..2] != [0x53, 0xef] || count != blocks {
        return Err("the superblock is not the one of the trailer".to_string());
    }
    Ok((blocks, sha))
}

/// The SHA-256 of the first `blocks` blocks of `p`.
fn hash(p: &Path, blocks: u64) -> Result<String, String> {
    use sha2::Digest;
    use std::io::Read;
    let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut r = f.take(blocks * BLOCK);
    let mut h = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut seen = 0u64;
    loop {
        let n = r
            .read(&mut buf)
            .map_err(|e| format!("{}: {e}", p.display()))?;
        if n == 0 {
            break;
        }
        seen += n as u64;
        h.update(&buf[..n]);
    }
    if seen != blocks * BLOCK {
        return Err(format!("{seen} bytes of {} could be read", blocks * BLOCK));
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The identity of the file at `p` that the `.ok` record holds.
fn identity(p: &Path) -> Option<String> {
    let m = std::fs::metadata(p).ok()?;
    Some(format!(
        "{} {} {} {}.{} {}.{}",
        m.dev(),
        m.ino(),
        m.len(),
        m.ctime(),
        m.ctime_nsec(),
        m.mtime(),
        m.mtime_nsec()
    ))
}

/// The name of a file of `p` in the making.
fn partial(p: &Path, nonce: &str) -> PathBuf {
    PathBuf::from(format!("{}.partial-{nonce}", p.display()))
}

/// Write `text` to `p` through a name of its own and a rename.
fn put(p: &Path, text: &[u8], nonce: &str) -> Result<(), String> {
    let tmp = partial(p, nonce);
    let r = std::fs::write(&tmp, text).and_then(|()| std::fs::rename(&tmp, p));
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r.map_err(|e| format!("{}: {e}", p.display()))
}

/// `Ok` where the disk at `p` can be used: its trailer holds, and the file
/// is the one of the last check, or its hash is its trailer's now.
pub fn check(p: &Path, nonce: &str) -> Result<(), String> {
    let (blocks, sha) = read_trailer(p)?;
    let ok = sibling(p, "ok");
    let id = identity(p).ok_or("the disk does not stat")?;
    let want = format!("{id} {sha}\n");
    if std::fs::read_to_string(&ok).is_ok_and(|t| t == want) {
        return Ok(());
    }
    let got = hash(p, blocks)?;
    if got != sha {
        return Err(format!("its SHA-256 is {got}, and its trailer names {sha}"));
    }
    put(&ok, want.as_bytes(), nonce)
}

/// Build the root disk of `rootfs` at `p`: a partial file, synced, then
/// renamed. Each other partial file of `p` is of a dead build (the caller
/// holds the lock), and goes.
fn build(p: &Path, rootfs: &Path, nonce: &str, seed: &str) -> Result<(), String> {
    let dir = p.parent().ok_or("the root disk has no directory")?;
    let stem = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = stem.trim_end_matches(".img");
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with(stem) && name.contains(".partial-") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let t = tree::Tree::walk(rootfs)?;
    let plan = image::plan(&t)?;
    let bytes = (plan.layout.blocks + 1) * BLOCK;
    crate::assets::space_floor(dir, "the root disk of the image", bytes)?;
    let tmp = partial(p, nonce);
    let what = |e: std::io::Error| format!("{}: {e}", tmp.display());
    let made = (|| {
        let f = std::fs::OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .mode(0o644)
            .open(&tmp)
            .map_err(what)?;
        f.set_len(plan.layout.blocks * BLOCK).map_err(what)?;
        // The names of the file system come from the digest: the same
        // image gives the same bytes.
        let key = <sha2::Sha256 as sha2::Digest>::digest(seed.as_bytes());
        let (a, b) = key.split_at(16);
        let (mut uuid, mut hash_seed) = ([0u8; 16], [0u8; 16]);
        uuid.copy_from_slice(a);
        hash_seed.copy_from_slice(b);
        uuid[6] = (uuid[6] & 0x0f) | 0x40;
        uuid[8] = (uuid[8] & 0x3f) | 0x80;
        let names = ext::Names {
            uuid,
            seed: hash_seed,
            label: b"podvm-root",
        };
        image::write(&plan, &t, &f, &names)?;
        f.sync_all().map_err(what)?;
        let sha = hash(&tmp, plan.layout.blocks)?;
        f.write_all_at(
            &trailer(plan.layout.blocks, &sha),
            plan.layout.blocks * BLOCK,
        )
        .map_err(what)?;
        f.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o444))
            .map_err(what)?;
        f.sync_all().map_err(what)?;
        std::fs::rename(&tmp, p).map_err(|e| format!("{}: {e}", p.display()))?;
        // The record of this file: the next check need not hash it again.
        let id = identity(p).ok_or("the disk just built does not stat")?;
        put(&sibling(p, "ok"), format!("{id} {sha}\n").as_bytes(), nonce)
    })();
    if made.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    made
}

/// What [`ensure`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Got {
    /// The disk of the cache.
    Cached,
    /// Built now; with the reason where a disk of the cache was damaged.
    Built(Option<String>),
}

/// The root disk of `digest` from the cache, or built now from `rootfs`.
pub fn ensure(store_root: &Path, digest: &str, rootfs: &Path) -> Result<(PathBuf, Got), String> {
    let p = path(store_root, digest);
    let nonce = super::super::init::nonce()?;
    let dir = p.parent().ok_or("the root disk has no directory")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if check(&p, &nonce).is_ok() {
        return Ok((p, Got::Cached));
    }
    let lock_path = sibling(&p, "lock");
    let start = Instant::now();
    loop {
        let lock = podvm_image::store::Lock::try_exclusive(&lock_path)
            .map_err(|e| format!("the lock of the root disk: {e}"))?;
        if let Some(_held) = lock {
            let damaged = match check(&p, &nonce) {
                Ok(()) => return Ok((p, Got::Cached)),
                Err(_) if !p.exists() => None,
                Err(why) => Some(why),
            };
            let seed = format!("{digest} {FORMAT}");
            build(&p, rootfs, &nonce, &seed)
                .map_err(|e| format!("the root disk of the image: {e}"))?;
            check(&p, &nonce).map_err(|e| format!("the root disk just built: {e}"))?;
            return Ok((p, Got::Built(damaged)));
        }
        if start.elapsed() > WAIT {
            return Err(format!(
                "another run held the lock {} of the root disk for {} s",
                lock_path.display(),
                WAIT.as_secs()
            ));
        }
        std::thread::sleep(POLL);
    }
}

/// The emulator's arguments of the root disk: read-only, so no guest can
/// change the disk of the cache.
pub fn argv(path: &Path) -> Vec<String> {
    let file = path.display().to_string().replace(',', ",,");
    vec![
        "-drive".to_string(),
        format!("if=none,id=podvmroot,format=raw,file={file},readonly=on"),
        "-device".to_string(),
        format!(
            "virtio-blk-pci,drive=podvmroot,serial={}",
            super::ROOT_SERIAL
        ),
    ]
}

/// The words of the notice of the tier for where the root of the guest
/// comes from: the root disk, or the base archive of the initramfs.
pub fn source(root: &Option<(PathBuf, Got)>, base: &Option<(PathBuf, bool)>) -> String {
    match (root, base) {
        (Some((_, Got::Cached)), _) => "root disk from the cache".to_string(),
        (Some((_, Got::Built(_))), _) => "root disk built now".to_string(),
        (None, Some((_, true))) => "base archive from the cache".to_string(),
        (None, _) => "base archive built now".to_string(),
    }
}

/// The kernel runs `podvm guest-root` as the first process in place of
/// the `/podvm-init` of the initramfs: `load` are the modules in their
/// order, `disk` the sized disk of the upper layer, `nonce` the one of the
/// markers. The words after `--` of the kernel line are the arguments of
/// the init, so they come last.
pub fn with_init(argv: &mut [String], load: &[String], disk: bool, nonce: &str) {
    let Some(i) = argv.iter().position(|a| a == "-append") else {
        return;
    };
    let Some(line) = argv.get_mut(i + 1) else {
        return;
    };
    let init = format!(
        "rdinit=/podvm/podvm -- {} --load {}{} {nonce}",
        super::ROOT_VERB,
        load.join(","),
        if disk { " --disk" } else { "" }
    );
    *line = line.replace("rdinit=/podvm-init", &init);
}
