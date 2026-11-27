//! The VM image of a run of `--podvm-vm` (TODO/machine.md T-1998): the
//! alias in incusd, and the kernel and the initrd of the image in the
//! store. Built and imported once for each digest and build of podvm,
//! under the lock of the import of its alias (T-2081).
//!
//! The parts of a build live in a directory of their own in the store
//! (a random part: a pid is not unique across PID namespaces), and go
//! after the import, also after a failure. The kernel and the initrd get
//! their names last, so a reader finds whole files or none.

use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use pod_incus::{Client, ImageData, ImageImport, Payload};

use super::super::run::{Fault, Job};
use crate::machine::guest::disk::{ext, image, tree};
use crate::machine::guest::kernel_image;

/// The version of the build: a change of the disk bumps it.
pub const FORMAT: u32 = 1;
/// The property of the image that holds the bytes of its disk.
pub const DISK_KEY: &str = "podvm.disk_bytes";
/// The property of the image that holds the label of its root.
pub const LABEL_KEY: &str = "podvm.root_label";

/// The VM image of a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmImage {
    pub alias: String,
    pub kernel: PathBuf,
    pub initrd: PathBuf,
    pub label: String,
    /// The bytes of the root disk.
    pub disk_bytes: u64,
    /// Imported by an earlier run.
    pub cached: bool,
}

/// The directory of the boot files of `alias` in the store.
pub fn boot_dir(store_root: &Path, alias: &str) -> PathBuf {
    store_root.join("incus-vm").join(alias)
}

/// The kernel, the initrd and their release in the tree of the image, with
/// the drivers of the VM checked; `net`: the run has a NIC.
pub fn boot_files(rootfs: &Path, net: bool) -> Result<(String, PathBuf, PathBuf), String> {
    let (release, kernel) = kernel_image::chosen(rootfs)?;
    let initrd = kernel_image::initrd_file(rootfs, &release).ok_or_else(|| {
        format!(
            "the image has no initrd of {release} (/boot/initrd.img-{release}, \
             /boot/initramfs-{release}.img or /usr/lib/modules/{release}/initrd): the direct \
             boot of an incus VM takes the initrd of the image"
        )
    })?;
    let (drivers, _) = kernel_image::drivers(rootfs, &release)?;
    let mut groups = vec![&super::ROOT, &super::AGENT];
    if net {
        groups.push(&kernel_image::NET);
    }
    drivers.plan(&groups)?;
    Ok((release, kernel, initrd))
}

/// Copy `src` to `dst` through a name of its own and a rename.
fn put(src: &Path, dst: &Path, nonce: &str) -> Result<(), String> {
    let tmp = PathBuf::from(format!("{}.partial-{nonce}", dst.display()));
    let r = std::fs::copy(src, &tmp).and_then(|_| std::fs::rename(&tmp, dst));
    if r.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    r.map(drop)
        .map_err(|e| format!("{} to {}: {e}", src.display(), dst.display()))
}

/// The plan of the disk: the tree of `rootfs` with the files of the agent
/// and `podvm`, and the spare of [`super::SPARE`]. `stage` holds the texts.
pub fn disk_plan(
    rootfs: &Path,
    podvm: &Path,
    stage: &Path,
) -> Result<(tree::Tree, image::Plan), String> {
    let mut t = tree::Tree::walk(rootfs)?;
    super::agent::add(&mut t, podvm, stage)?;
    let spare_blocks = super::SPARE / ext::BLOCK;
    let spare_inodes = super::SPARE / (16 << 10);
    let plan = image::plan_with(&t, spare_blocks, spare_inodes)?;
    Ok((t, plan))
}

/// The names of the disk of `digest`, with `label`.
fn names_of(digest: &str, label: &str) -> ([u8; 16], [u8; 16], Vec<u8>) {
    let key = <sha2::Sha256 as sha2::Digest>::digest(format!("{digest} vm {FORMAT}").as_bytes());
    let (mut uuid, mut seed) = ([0u8; 16], [0u8; 16]);
    let (a, b) = key.split_at(16);
    uuid.copy_from_slice(a);
    seed.copy_from_slice(b);
    uuid[6] = (uuid[6] & 0x0f) | 0x40;
    uuid[8] = (uuid[8] & 0x3f) | 0x80;
    (uuid, seed, label.as_bytes().to_vec())
}

/// Write the raw disk of `rootfs` at `raw` and its qcow2 at `qcow`; the
/// bytes of the disk.
pub fn write_disk(
    rootfs: &Path,
    digest: &str,
    label: &str,
    podvm: &Path,
    dir: &Path,
) -> Result<u64, String> {
    let stage = dir.join("files");
    std::fs::create_dir(&stage).map_err(|e| format!("{}: {e}", stage.display()))?;
    let (t, plan) = disk_plan(rootfs, podvm, &stage)?;
    let bytes = plan.layout.blocks * ext::BLOCK;
    let data = (plan.layout.blocks - super::SPARE / ext::BLOCK) * ext::BLOCK;
    crate::assets::space_floor(dir, "the disk of the incus VM", data.saturating_mul(2))?;
    let raw_path = dir.join("root.raw");
    let what = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    let raw = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&raw_path)
        .map_err(|e| what(&raw_path, e))?;
    raw.set_len(bytes).map_err(|e| what(&raw_path, e))?;
    let (uuid, seed, l) = names_of(digest, label);
    let names = ext::Names {
        uuid,
        seed,
        label: &l,
    };
    image::write(&plan, &t, &raw, &names)?;
    let qcow_path = dir.join("rootfs.img");
    let qcow = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&qcow_path)
        .map_err(|e| what(&qcow_path, e))?;
    super::qcow2::write(&raw, &qcow)?;
    drop(raw);
    std::fs::remove_file(&raw_path).map_err(|e| what(&raw_path, e))?;
    qcow.sync_all().map_err(|e| what(&qcow_path, e))?;
    Ok(bytes)
}

/// The metadata tarball of the VM image.
pub fn metadata_tar(now: u64) -> Result<Vec<u8>, String> {
    let meta = format!(
        "architecture: x86_64\ncreation_date: {now}\nproperties:\n  description: an OCI \
         image of podvm as a virtual machine\n"
    );
    let mut b = tar::Builder::new(Vec::new());
    let mut h = tar::Header::new_gnu();
    h.set_size(meta.len() as u64);
    h.set_mode(0o644);
    h.set_mtime(now);
    h.set_cksum();
    b.append_data(&mut h, "metadata.yaml", meta.as_bytes())
        .map_err(|e| format!("the metadata of the VM image: {e}"))?;
    b.into_inner()
        .map_err(|e| format!("the metadata of the VM image: {e}"))
}

/// The VM image of `job` in incusd and in the store: from an earlier run,
/// or built and imported now.
pub async fn ensure(c: &Client, job: &Job<'_>) -> Result<VmImage, Fault> {
    let one = |e: String| (e, None);
    let store = job
        .store
        .ok_or_else(|| one("--podvm-vm needs the store of the image".into()))?;
    let me = std::fs::read("/proc/self/exe").map_err(|e| one(format!("/proc/self/exe: {e}")))?;
    let alias = super::alias(job.digest, &crate::machine::guest::kernel::sha256_hex(&me));
    drop(me);
    let dir = boot_dir(store.root(), &alias);
    if let Some(img) = cached(c, &alias, &dir).await?.filter(|i| i.cached) {
        return Ok(img);
    }
    let _lock = super::super::import::import_lock(job, &alias).await?;
    let found = cached(c, &alias, &dir).await?;
    if let Some(img) = found.clone().filter(|i| i.cached) {
        return Ok(img);
    }
    let fresh = match job.tree {
        Some(t) => t.ensure().map_err(one)?,
        None => false,
    };
    let done = build_and_import(c, job, &alias, &dir, found).await;
    if fresh && let Some(w) = job.tree.and_then(|t| t.release()) {
        eprintln!("podvm {}: incus tier: {w}", job.verb);
    }
    done
}

/// The image of `alias` where incusd has it: `cached` where the boot
/// files are in `dir` too. `None` where incusd has no such image.
async fn cached(c: &Client, alias: &str, dir: &Path) -> Result<Option<VmImage>, Fault> {
    let one = |e: pod_incus::Error| (e.to_string(), None);
    let Some(a) = c.alias(alias).await.map_err(one)? else {
        return Ok(None);
    };
    let Some(i) = c.image(&a.target).await.map_err(one)? else {
        return Ok(None);
    };
    let bytes = i.properties.get(DISK_KEY).and_then(|b| b.parse().ok());
    let label = i.properties.get(LABEL_KEY).cloned();
    let (Some(disk_bytes), Some(label)) = (bytes, label) else {
        return Err((
            format!(
                "the image {alias} of incusd has no {DISK_KEY} or {LABEL_KEY}: it is not an \
                 image of this podvm; `incus image delete {}` removes it",
                a.target
            ),
            None,
        ));
    };
    let (kernel, initrd) = (dir.join("vmlinuz"), dir.join("initrd"));
    let whole = |p: &Path| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() > 0);
    Ok(Some(VmImage {
        alias: alias.to_string(),
        cached: whole(&kernel) && whole(&initrd),
        kernel,
        initrd,
        label,
        disk_bytes,
    }))
}

/// The boot files of `job` in `dir`, and the disk imported under `alias`
/// where incusd does not have it (`found` is `None`).
async fn build_and_import(
    c: &Client,
    job: &Job<'_>,
    alias: &str,
    dir: &Path,
    found: Option<VmImage>,
) -> Result<VmImage, Fault> {
    let one = |e: String| (e, None);
    let net = !job.net_none;
    let (_, k, i) = boot_files(job.rootfs, net).map_err(|e| one(format!("--podvm-vm: {e}")))?;
    let nonce = crate::machine::guest::init::nonce().map_err(one)?;
    std::fs::create_dir_all(dir).map_err(|e| one(format!("{}: {e}", dir.display())))?;
    let (kernel, initrd) = (dir.join("vmlinuz"), dir.join("initrd"));
    put(&k, &kernel, &nonce).map_err(one)?;
    put(&i, &initrd, &nonce).map_err(one)?;
    if let Some(img) = found {
        // The disk is in incusd: the boot files were all that was missing.
        return Ok(img);
    }
    let fstab = crate::machine::guest::pid1::resolve(job.rootfs, "/etc/fstab")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    let label = super::root_label(&fstab);
    let parts = dir.with_extension(format!("partial-{nonce}"));
    std::fs::create_dir(&parts).map_err(|e| one(format!("{}: {e}", parts.display())))?;
    let made = import(c, job, alias, &label, &parts).await;
    let _ = std::fs::remove_dir_all(&parts);
    let disk_bytes = made?;
    Ok(VmImage {
        alias: alias.to_string(),
        kernel,
        initrd,
        label,
        disk_bytes,
        cached: false,
    })
}

/// Write the disk into `parts` and import it under `alias`; the bytes of
/// the disk.
async fn import(
    c: &Client,
    job: &Job<'_>,
    alias: &str,
    label: &str,
    parts: &Path,
) -> Result<u64, Fault> {
    let one = |e: String| (e, None);
    let (rootfs, digest) = (job.rootfs.to_path_buf(), job.digest.to_string());
    let (l, p) = (label.to_string(), parts.to_path_buf());
    let bytes = tokio::task::spawn_blocking(move || {
        write_disk(&rootfs, &digest, &l, Path::new("/proc/self/exe"), &p)
    })
    .await
    .map_err(|e| one(format!("the writer of the VM disk: {e}")))?
    .map_err(|e| one(format!("--podvm-vm: the root disk: {e}")))?;
    let meta = metadata_tar(ext::now()).map_err(one)?;
    let qcow_path = parts.join("rootfs.img");
    let qcow = tokio::fs::File::open(&qcow_path)
        .await
        .map_err(|e| one(format!("{}: {e}", qcow_path.display())))?;
    let len = qcow
        .metadata()
        .await
        .map_err(|e| one(format!("{}: {e}", qcow_path.display())))?
        .len();
    let mut import = ImageImport::new(ImageData::Split {
        metadata: Payload::bytes(meta),
        rootfs: Payload::new(qcow, Some(len)),
        vm: true,
    });
    import.filename = "rootfs.img".to_string();
    import.aliases = vec![alias.to_string()];
    import
        .properties
        .insert(DISK_KEY.to_string(), bytes.to_string());
    import
        .properties
        .insert(LABEL_KEY.to_string(), label.to_string());
    match c.import_image(import).await {
        Ok(_) => Ok(bytes),
        // Another store on this incusd imported the same alias meanwhile.
        Err(_) if matches!(c.alias(alias).await, Ok(Some(_))) => Ok(bytes),
        Err(e) => Err((format!("--podvm-vm: the import of the VM image: {e}"), None)),
    }
}
