//! `--storage-opt size=N` on the machine tier: a sized writable disk of
//! the guest, TODO/machine.md T-1968 (PodUtils/podrunner#296 ask 5).
//!
//! podvm makes a sparse file of N bytes in the directory of the run, writes
//! an empty ext4 file system into it with no tool of the host ([`ext`]),
//! and gives it to the guest as a virtio disk with the serial
//! [`SERIAL`]. In the guest, the init execs `podvm guest-disk` ([`guest`])
//! after its first mounts: it loads the modules of the pinned modloop,
//! mounts the disk, mounts an overlay with the root of the initramfs as the
//! lower layer and the disk as the upper layer, moves `/dev`, `/proc` and
//! `/sys` into it, makes it the root, and execs the rest of the init.
//! A write to the root of the guest and to its working directory then
//! lands on the disk, and a write past its size fails with ENOSPC.
//!
//! A `-v` volume is a 9p mount of a host directory over that root: the
//! disk does not bound it. The disk goes with the directory of the run.
//!
//! TODO/machine.md T-1994: the root disk of an image ([`root`]) is a
//! read-only ext4 of the image's tree ([`tree`], [`image`], [`inode`]),
//! built once for each digest. A guest of it boots `podvm guest-root`
//! from an initramfs of podvm's files only, with the same modules; the
//! upper layer of its root is the sized disk where the run has one.

pub(crate) mod ext;
mod guest;
pub(crate) mod image;
mod inode;
pub mod root;
pub(crate) mod tree;

use std::path::{Path, PathBuf};

pub(crate) use guest::end_run;
pub use guest::main as guest_main;
#[cfg(test)]
pub(crate) use guest::root_args;
pub use guest::root_main as guest_root_main;

/// The modules of the disk, out of the pinned modloop, with each module
/// that `modules.dep` names for them. `crc32c_generic` comes first: ext4
/// asks for the algorithm `crc32c` at its mount ("Cannot load crc32c
/// driver" without it), and `modules.dep` does not name it.
pub const MODULES: [&str; 4] = ["crc32c_generic", "virtio_blk", "ext4", "overlay"];
/// The serial of the virtio disk, by which the guest finds it.
pub const SERIAL: &str = "podvm-disk";
/// The subcommand that the kernel of a guest of the root disk runs first.
pub const ROOT_VERB: &str = "guest-root";
/// The serial of the root disk of the image (T-1994).
pub const ROOT_SERIAL: &str = "podvm-root";
/// `PODVM_MACHINE_ROOT`: `disk` (the default where the guest can have
/// it) or `initramfs`, the root of the guest before T-1994.
pub const ROOT_VAR: &str = "PODVM_MACHINE_ROOT";
/// The rest of the init after the root moved to the disk, the init of a
/// failure, and the reason of the failure.
pub const NEXT: &str = "/podvm/init-run";
pub const FAIL: &str = "/podvm/init-disk-fail";
pub const ERR: &str = "/podvm/disk.err";
const FILE: &str = "disk.raw";

/// The bytes of a value of `--storage-opt size=` as the parser keeps it
/// (`NB`, `run/parse/limits.rs`).
pub fn bytes_of(value: &str) -> Option<u64> {
    value.strip_suffix('B')?.parse().ok()
}

/// The disk of a run from the kept value of `--storage-opt size=`
/// (`None`: no flag), or the refusal of the run before its pull. `amd64`
/// is a guest of the pinned x86_64 kernel.
pub fn of_value(value: Option<&str>, amd64: bool) -> Result<Option<u64>, String> {
    let Some(v) = value else {
        return Ok(None);
    };
    let bytes = bytes_of(v).ok_or_else(|| {
        format!("--storage-opt size={v}: not a count of bytes (TODO/machine.md T-1968)")
    })?;
    let pinned = std::env::var_os("PODVM_MACHINE_KERNEL").is_none();
    match refusal(amd64, pinned, bytes) {
        Some(why) => Err(why),
        None => Ok(Some(bytes)),
    }
}

/// Why a guest cannot have a disk of `bytes`, or `None`. `amd64` is a
/// guest of the pinned x86_64 kernel; `pinned` is a kernel with the module
/// set of the pin (no `PODVM_MACHINE_KERNEL`).
pub fn refusal(amd64: bool, pinned: bool, bytes: u64) -> Option<String> {
    let why = if !amd64 {
        "the guest needs podvm and the modules of the pinned x86_64 kernel, and a guest of \
         linux/arm64 has neither yet (TODO/machine.md T-1890)"
            .to_string()
    } else if !pinned {
        "the guest needs the ext4, virtio disk and overlay modules of the pinned kernel, and \
         PODVM_MACHINE_KERNEL names another kernel"
            .to_string()
    } else {
        ext::Layout::of(bytes).err()?
    };
    Some(format!(
        "--storage-opt size={bytes}: {why} (TODO/machine.md T-1968)"
    ))
}

/// The module files of the disk in the store, beside the kernel.
pub fn modules(store_root: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    super::engine::set(
        store_root,
        "disk",
        &MODULES,
        "--storage-opt size= on the machine tier needs the modules of the pinned kernel, and \
         PODVM_MACHINE_KERNEL names another kernel (TODO/machine.md T-1968)",
    )
}

/// TODO/machine.md T-1994: whether the root of the guest is the disk of
/// its image, for the value of [`ROOT_VAR`]. The default takes it where
/// the guest is x86_64 with the module set of the pinned kernel, and keeps
/// the initramfs elsewhere; `disk` refuses such a guest by name. The
/// default keeps the initramfs for `--podvm-engine` (`engine`) too: the
/// overlay2 driver of an engine refuses a directory on overlayfs.
pub fn root_choice(
    var: Option<&str>,
    amd64: bool,
    pinned: bool,
    engine: bool,
) -> Result<bool, String> {
    let can = amd64 && pinned;
    match var {
        None | Some("") => Ok(can && !engine),
        Some("initramfs") => Ok(false),
        Some("disk") if can => Ok(true),
        Some("disk") => Err(format!(
            "{ROOT_VAR}=disk: the root disk needs podvm and the ext4, virtio disk and overlay \
             modules of the pinned x86_64 kernel in the guest, and this guest is {} \
             (TODO/machine.md T-1994)",
            if amd64 {
                "of PODVM_MACHINE_KERNEL"
            } else {
                "linux/arm64"
            }
        )),
        Some(other) => Err(format!(
            "{ROOT_VAR}={other}: the values are disk and initramfs (TODO/machine.md T-1994)"
        )),
    }
}

/// [`root_choice`] for the run `r`; `modules`: the kernel has the module
/// set of the pin.
pub fn root_wanted(r: &super::Request, modules: bool) -> Result<bool, String> {
    let pinned = modules && std::env::var_os("PODVM_MACHINE_KERNEL").is_none();
    let var = std::env::var(ROOT_VAR).ok();
    root_choice(
        var.as_deref(),
        r.arch == super::arch::Arch::Amd64,
        pinned,
        r.engine,
    )
}

/// The root disk of the run `r`, from the cache or built now; a damaged
/// disk of the cache is named on stderr before its build.
pub fn root_of(r: &super::Request) -> Result<(PathBuf, root::Got), String> {
    let got = root::ensure(r.store_root, r.digest, r.rootfs)?;
    if let root::Got::Built(Some(why)) = &got.1 {
        eprintln!(
            "podvm {}: machine tier: the root disk {} of the cache was damaged ({why}); podvm \
             built it again (TODO/machine.md T-1994)",
            r.verb,
            got.0.display()
        );
    }
    Ok(got)
}

/// TODO/machine.md T-1968, T-1994: where the run asks for a disk or its
/// root is the disk of its image (`root`), the refusal of a guest that
/// cannot have a disk, or the modules of the disk added to `modules`
/// (each name once). `pinned`: the kernel has the module set of the pin.
/// The names of the disk's modules come back in the order of their load;
/// none where the run has neither. `own`: the modules of the kernel of
/// the image in place of the pinned set (T-1994, `kernel_image`).
pub fn add_modules(
    r: &super::Request,
    root: bool,
    pinned: bool,
    own: Option<&[(String, PathBuf)]>,
    modules: &mut Vec<(String, PathBuf)>,
) -> Result<Vec<String>, String> {
    if let Some(bytes) = r.disk
        && let Some(why) = refusal(r.arch == super::arch::Arch::Amd64, pinned, bytes)
    {
        return Err(why);
    }
    if r.disk.is_none() && !root {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    let set = match own {
        Some(o) => o.to_vec(),
        None => self::modules(r.store_root)?,
    };
    for (n, p) in set {
        names.push(n.clone());
        if !modules.iter().any(|(m, _)| *m == n) {
            modules.push((n, p));
        }
    }
    Ok(names)
}

/// The init of a run and the archive that its initrd adds: `script` as it
/// is and none where `load` (the modules of the disk, in their order) is
/// empty; else the split of [`split`], with [`NEXT`] and [`FAIL`] in the
/// archive.
pub fn init(script: String, load: &[String], nonce: &str) -> std::io::Result<(String, Vec<u8>)> {
    if load.is_empty() {
        return Ok((script, Vec::new()));
    }
    let head = super::init::head();
    let (init, rest) = split(&script, &head, load).map_err(std::io::Error::other)?;
    let more = archive(&rest, &super::init::disk_fail(nonce))?;
    Ok((init, more))
}

/// Make the disk of a run in `dir`: a new sparse file of `bytes` with an
/// empty ext4 file system. A disk of an earlier boot of the same run goes
/// first (TODO/machine.md T-2016): a guest that crashed leaves no layer.
pub fn make(dir: &Path, bytes: u64) -> Result<PathBuf, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let layout = ext::Layout::of(bytes)?;
    // The host must hold each byte that the guest can write: a write past
    // the free space of the host is EIO in the guest, not ENOSPC.
    crate::assets::space_floor(dir, "the disk of --storage-opt size=", bytes)?;
    let path = dir.join(FILE);
    let _ = std::fs::remove_file(&path);
    let what = |e: std::io::Error| format!("the disk {}: {e}", path.display());
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(what)?;
    let made = random()
        .and_then(|(uuid, seed)| {
            file.set_len(bytes)?;
            ext::write(&file, &layout, uuid, seed)
        })
        .map_err(what);
    if made.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    made.map(|()| path)
}

/// The UUID of a file system and the seed of its directory hashes.
fn random() -> std::io::Result<([u8; 16], [u8; 16])> {
    use std::io::Read;
    let (mut uuid, mut seed) = ([0u8; 16], [0u8; 16]);
    let mut urandom = std::fs::File::open("/dev/urandom")?;
    urandom.read_exact(&mut uuid)?;
    urandom.read_exact(&mut seed)?;
    uuid[6] = (uuid[6] & 0x0f) | 0x40; // version 4
    uuid[8] = (uuid[8] & 0x3f) | 0x80;
    Ok((uuid, seed))
}

/// The emulator's arguments of the disk. A write error of the host goes to
/// the guest (`werror=report`): QEMU's default stops the guest on ENOSPC
/// of the host, and the run would wait for its timeout. The disk is
/// disposable, so no write waits for the host's disk (`cache=unsafe`).
pub fn argv(path: &Path) -> Vec<String> {
    // QEMU reads a comma of a value as the end of the option; two are one.
    let file = path.display().to_string().replace(',', ",,");
    vec![
        "-drive".to_string(),
        format!(
            "if=none,id=podvmdisk,format=raw,file={file},cache=unsafe,werror=report,\
             rerror=report"
        ),
        "-device".to_string(),
        format!("virtio-blk-pci,drive=podvmdisk,serial={SERIAL}"),
    ]
}

/// The init of a run with a disk: the first lines of `script` (`head`, the
/// mounts and the modules of each guest), then `podvm guest-disk`, which
/// loads `load` in its order (each module after the modules that it
/// needs: a module before its symbols fails with ENOENT) and execs
/// [`NEXT`], the rest of `script`. `Err` where `script` does not start
/// with `head`, or a name is not a word of a module.
pub fn split(script: &str, head: &str, load: &[String]) -> Result<(String, String), String> {
    let rest = script
        .strip_prefix(head)
        .ok_or("the init script does not start with its mounts")?;
    if let Some(bad) = load
        .iter()
        .find(|n| n.is_empty() || !n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
    {
        return Err(format!("{bad:?} is not the name of a module"));
    }
    let load = load.join(",");
    let init = format!("{head}exec /podvm/podvm guest-disk --load {load} {NEXT} {FAIL}\n");
    Ok((init, format!("#!/bin/sh\n{rest}")))
}

/// The archive that the initrd of a run with a disk adds: [`NEXT`] and
/// [`FAIL`].
pub fn archive(next: &str, fail: &str) -> std::io::Result<Vec<u8>> {
    use super::cpio;
    let mut w = cpio::Newc::new(Vec::new());
    w.entry("podvm", cpio::DIR | 0o755, &[], (0, 0))?;
    for (path, text) in [(NEXT, next), (FAIL, fail)] {
        let name = path.trim_start_matches('/');
        w.entry(name, cpio::REG | 0o755, text.as_bytes(), (0, 0))?;
    }
    w.finish()
}

#[cfg(test)]
mod image_tests;
#[cfg(test)]
mod tests;
