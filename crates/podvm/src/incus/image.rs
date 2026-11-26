//! The incus image of an OCI image, TODO/machine.md T-1718: a unified
//! tarball of its layers, with a tree-based path for images that need it.
//!
//! In the fallback, podvm extracts with no ownership change and records what the image
//! meant in `.meta.jsonl` beside the rootfs (`podvm extract --help`). The
//! tarball gives each entry those ids and that mode, so that the files in
//! the instance belong to whom the image says (`/etc/shadow` is
//! `root:shadow`). An entry with no record gets the owner 0 and its mode on
//! disk.
//!
//! The tarball also carries podvm itself at `/.podvm/podvm`, the init of
//! the instance (TODO/machine.md T-1722): a run then pushes no binary. The
//! alias names podvm's own digest, so a new podvm imports the image again.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

/// The version of the tarball's form; a change gives a new alias.
pub const IMAGE_FORMAT: u32 = 4;

/// The directory and the file that podvm adds to the rootfs.
pub(super) const INIT_DIR: &str = ".podvm";
pub(super) const INIT_FILE: &str = ".podvm/podvm";

/// The alias of an image of `digest` with the podvm of digest `me`.
pub fn alias(digest: &str, me: &str) -> String {
    let hex = digest.trim_start_matches("sha256:");
    format!(
        "podvm-{}-{}-v{IMAGE_FORMAT}",
        &hex[..hex.len().min(16)],
        &me[..me.len().min(16)]
    )
}

/// An image's layers in manifest order. The store hold protects their blobs.
pub struct Layers {
    pub blobs: Vec<podvm_extract::Layer>,
    pub loud: bool,
}

impl Layers {
    /// The estimate of the gzip upload is the size of the compressed layers.
    pub(super) fn compressed_bytes(&self) -> u64 {
        self.blobs
            .iter()
            .fold(0u64, |n, l| n.saturating_add(l.size))
    }

    /// Measure the final tree without creating it. The tar reader counts the
    /// members that survive whiteouts and replacement, and counts a hard link
    /// once per inode. Tar headers and padding are not file bytes.
    /// `None` means the image needs the extraction path.
    pub(super) fn measure(&self, podvm_len: u64) -> Result<Option<super::space::Need>, String> {
        let (read, write) =
            std::io::pipe().map_err(|e| format!("a pipe for measuring the image: {e}"))?;
        std::thread::scope(|scope| {
            let writer = std::thread::Builder::new()
                .spawn_scoped(scope, move || {
                    super::cut::block_pipe();
                    self.flatten(&mut std::io::BufWriter::new(write), &[], true)
                })
                .map_err(|e| format!("the writer for measuring the image: {e}"))?;
            let mut reserved_init = false;
            let need = need_from_tar(read, &mut reserved_init);
            let flattened = writer
                .join()
                .map_err(|_| "the writer for measuring the image ended with a panic")?;
            match flattened {
                // A hard link whose data entry already went out can lose
                // records, so use the tree when any record could be lost.
                Ok(stats) if stats.links > 0 && stats.xattrs.unset > 0 => Ok(None),
                Ok(_) => need
                    .map(|mut need| {
                        need.bytes = need.bytes.saturating_add(podvm_len);
                        Some(need)
                    })
                    .map_err(|e| format!("the unified tarball: {e}")),
                // The flattener refuses a collision with the bundled
                // binary. The extraction path replaces the reserved entry.
                Err(e) if e.class == pod_core::Class::Usage && reserved_init => Ok(None),
                Err(e) if needs_tree(&e) => Ok(None),
                Err(e) => Err(e.chain()),
            }
        })
    }

    /// Write a unified archive. A fixed, nonzero creation date keeps the
    /// fingerprint stable when separate stores import the same bytes.
    pub(super) fn flatten<W: std::io::Write>(
        &self,
        out: &mut W,
        podvm: &[u8],
        report: bool,
    ) -> Result<podvm_extract::flatten::Flattened, pod_core::Failure> {
        use podvm_extract::flatten::{flatten_with, Options};
        let mut opts = Options {
            prefix: "rootfs/".into(),
            ..Options::default()
        };
        if report && self.loud {
            opts.report.events =
                std::sync::Arc::new(|event: pod_core::Event| eprintln!("{}", event.text));
        }
        flatten_with(
            &self.blobs,
            out,
            &opts,
            |entries| entries.file("metadata.yaml", 0o644, metadata(1).as_bytes()),
            |entries| {
                // An image can already have the directory. Writing the
                // binary also checks its parent and refuses a file or link.
                if let Err(e) = entries.dir("rootfs/.podvm", 0o755)
                    && e.class != pod_core::Class::Usage
                {
                    return Err(e);
                }
                entries.file("rootfs/.podvm/podvm", 0o755, podvm)
            },
        )
    }
}

/// These refusals can occur for valid images that an extracted tree holds.
fn needs_tree(e: &pod_core::Failure) -> bool {
    e.class == pod_core::Class::Unsupported
        || podvm_extract::refused(e)
            .is_some_and(|r| matches!(r.refusal, podvm_extract::safety::Refusal::Stream(_)))
}

fn need_from_tar(
    read: impl std::io::Read,
    reserved_init: &mut bool,
) -> std::io::Result<super::space::Need> {
    // The count is the one of the unpacked bytes of `inspect`
    // (`podvm_image::unpacked`, T-2104): the sizes of the file entries and
    // the entries that are not hard links, below `rootfs/`.
    let count = podvm_image::unpacked::count_tar(read, b"rootfs/", |path, kind| {
        if path == INIT_FILE.as_bytes() || (path == INIT_DIR.as_bytes() && !kind.is_dir()) {
            *reserved_init = true;
        }
    })?;
    Ok(super::space::Need {
        bytes: count.bytes,
        entries: count.entries,
    })
}

fn metadata(creation_date: u64) -> String {
    format!(
        "architecture: x86_64\ncreation_date: {creation_date}\nproperties:\n  description: an OCI image of podvm\n"
    )
}

/// The intended uid, gid and mode for each path of the sidecar; a later
/// line (a later layer) wins.
pub fn intended(sidecar: &str) -> HashMap<String, (u64, u64, u32)> {
    let mut m = HashMap::new();
    for line in sidecar.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let (Some(p), Some(u), Some(g)) = (
            v.get("path").and_then(Value::as_str),
            v.get("uid").and_then(Value::as_u64),
            v.get("gid").and_then(Value::as_u64),
        ) else {
            continue;
        };
        let Some(mode) = v
            .get("mode")
            .and_then(Value::as_str)
            .and_then(|s| u32::from_str_radix(s, 8).ok())
        else {
            continue;
        };
        m.insert(p.trim_start_matches('/').to_string(), (u, g, mode));
    }
    m
}

/// The unified tarball of `rootfs` at `out`: `metadata.yaml` and
/// `rootfs/`, with the owners and modes of `.meta.jsonl` beside `rootfs`,
/// and `podvm` at `rootfs/.podvm/podvm` (owner 0, mode 0755).
#[cfg(test)]
pub fn unified_tarball(rootfs: &Path, out: &Path, podvm: &[u8]) -> Result<(), String> {
    let f = std::fs::File::create(out).map_err(|e| format!("{}: {e}", out.display()))?;
    unified_tarball_to(rootfs, f, podvm).map(drop)
}

/// [`unified_tarball`] into `w`: a pipe to the import of incusd, so that
/// no tarball file lies in the store (TODO/machine.md T-1997). It gives
/// `w` back, flushed, so that a gzip writer can end its stream (T-2085).
pub fn unified_tarball_to<W: std::io::Write>(
    rootfs: &Path,
    w: W,
    podvm: &[u8],
) -> Result<W, String> {
    let sidecar = rootfs.with_file_name(".meta.jsonl");
    let text = std::fs::read_to_string(&sidecar).unwrap_or_default();
    let meant = intended(&text);
    let unset = super::xattrs::unset_of(&text); // T-1999
    let mut b = tar::Builder::new(std::io::BufWriter::new(w));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(1);
    // incus refuses a creation_date of 0 ("Missing creation date").
    let meta = metadata(now);
    let mut h = tar::Header::new_gnu();
    h.set_size(meta.len() as u64);
    h.set_mode(0o644);
    h.set_uid(0);
    h.set_gid(0);
    h.set_mtime(now);
    h.set_cksum();
    let err = |e: std::io::Error| format!("the unified tarball of {}: {e}", rootfs.display());
    b.append_data(&mut h, "metadata.yaml", meta.as_bytes())
        .map_err(err)?;
    let mut firsts = Firsts::new();
    walk(&mut b, rootfs, "", &meant, &unset, &mut firsts).map_err(err)?;
    for (path, mode, data) in [
        ("rootfs/.podvm", 0o755, None),
        ("rootfs/.podvm/podvm", 0o755, Some(podvm)),
    ] {
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(if data.is_some() {
            tar::EntryType::Regular
        } else {
            tar::EntryType::Directory
        });
        h.set_size(data.map_or(0, |d| d.len() as u64));
        h.set_mode(mode);
        h.set_uid(0);
        h.set_gid(0);
        h.set_mtime(now);
        h.set_cksum();
        b.append_data(&mut h, path, data.unwrap_or(&[]))
            .map_err(err)?;
    }
    b.into_inner()
        .and_then(|w| w.into_inner().map_err(|e| e.into_error()))
        .and_then(|mut w| std::io::Write::flush(&mut w).map(|()| w))
        .map_err(err)
}

/// TODO/cli.md T-2079 (OR-B39-08): the first name in the archive of each
/// file of more than one link, by its device and inode, with the owner,
/// the mode and the attributes that its entry gave.
type Firsts = HashMap<(u64, u64), (String, (u64, u64, u32), Vec<super::xattrs::Attr>)>;

/// The name in the archive that the file `md` at the path `at` is a hard
/// link of: the first name of its inode, where that name gave the same
/// owner, mode and attributes. `None` writes the file with its data, and
/// records it as the first name of its inode where it has more links.
fn link_of(
    firsts: &mut Firsts,
    md: &std::fs::Metadata,
    at: &str,
    ids: (u64, u64, u32),
    attrs: &[super::xattrs::Attr],
) -> Option<String> {
    use std::os::unix::fs::MetadataExt;
    if md.nlink() < 2 {
        return None;
    }
    match firsts.get(&(md.dev(), md.ino())) {
        Some((first, i, a)) if *i == ids && a == attrs => Some(first.clone()),
        // A name with other records keeps them: a full copy.
        Some(_) => None,
        None => {
            let first = (at.to_string(), ids, attrs.to_vec());
            firsts.insert((md.dev(), md.ino()), first);
            None
        }
    }
}

fn walk<W: std::io::Write>(
    b: &mut tar::Builder<W>,
    dir: &Path,
    rel: &str,
    meant: &HashMap<String, (u64, u64, u32)>,
    unset: &HashMap<String, Vec<super::xattrs::Attr>>,
    firsts: &mut Firsts,
) -> std::io::Result<()> {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .and_then(|d| d.collect::<Result<_, _>>())
        .map_err(|e| std::io::Error::new(e.kind(), format!("{}: {e}", dir.display())))?;
    names.sort_by_key(|e| e.file_name());
    for e in names {
        let name = e.file_name().to_string_lossy().into_owned();
        let r = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        // Podvm installs its own init. A source entry at that path and
        // its descendants do not go into the archive.
        if r == INIT_FILE {
            continue;
        }
        let p = e.path();
        let md = std::fs::symlink_metadata(&p)?;
        if r == INIT_DIR && !md.is_dir() {
            continue;
        }
        let mut h = tar::Header::new_gnu();
        h.set_metadata_in_mode(&md, tar::HeaderMode::Deterministic);
        let (u, g, mode) = meant.get(&r).copied().unwrap_or((
            0,
            0,
            std::os::unix::fs::PermissionsExt::mode(&md.permissions()),
        ));
        h.set_uid(u);
        h.set_gid(g);
        h.set_mode(mode & 0o7777);
        let at = format!("rootfs/{r}");
        // T-1999: the attributes of the image, as PAX records before it.
        let attrs = super::xattrs::merge(
            super::xattrs::on_disk_readable(&p, &md, super::xattrs::of_image)?,
            unset.get(&r),
        );
        let ft = md.file_type();
        // T-2079 (OR-B39-08): each other name of a file goes out as a hard
        // link entry, so that incusd unpacks one inode, not a copy each.
        let first = match ft.is_file() {
            true => link_of(firsts, &md, &at, (u, g, mode & 0o7777), &attrs),
            false => None,
        };
        if let Some(first) = first {
            h.set_entry_type(tar::EntryType::Link);
            h.set_size(0);
            b.append_link(&mut h, &at, first)?;
            continue;
        }
        if !attrs.is_empty() {
            let data = super::xattrs::pax_records(&attrs);
            let mut x = tar::Header::new_ustar();
            x.set_entry_type(tar::EntryType::XHeader);
            x.set_path("PaxHeaders/x")?;
            x.set_size(data.len() as u64);
            x.set_mode(0o644);
            x.set_cksum();
            b.append(&x, &data[..])?;
        }
        if ft.is_symlink() {
            let target = std::fs::read_link(&p)?;
            b.append_link(&mut h, &at, target)?;
        } else if ft.is_dir() {
            h.set_size(0);
            b.append_data(&mut h, &at, std::io::empty())?;
            // A directory or a file of the image that its owner cannot read
            // (`/etc/shadow` of mode 0000 in ubi-init, experiment 434 clause
            // 19) is podvm's own: read it with the owner's bits, then restore
            // the image's mode. The header keeps the image's mode.
            podvm_extract::tree::with_owner_bits(&p, 0o500, || {
                walk(b, &p, &r, meant, unset, firsts)
            })?;
        } else if ft.is_file() {
            let f = owned(&p, 0o400, || std::fs::File::open(&p))?;
            b.append_data(&mut h, &at, f)?;
        }
        // A socket, a fifo or a device: an extracted rootfs holds none.
    }
    Ok(())
}

/// [`podvm_extract::tree::with_owner_bits`], with the path in its error.
fn owned<T>(p: &Path, bits: u32, f: impl FnOnce() -> std::io::Result<T>) -> std::io::Result<T> {
    podvm_extract::tree::with_owner_bits(p, bits, f)
        .map_err(|e| std::io::Error::new(e.kind(), format!("{}: {e}", p.display())))
}
