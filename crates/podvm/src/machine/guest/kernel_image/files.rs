//! The files of the kernel of an image that podvm reads on the host
//! (TODO/machine.md T-1994, part 2): the kernel, the modules and the text
//! of the release, each within a bound.

use std::io::Read;
use std::path::{Path, PathBuf};

use super::super::pid1::resolve;
use super::{is_release, Want, VAR};

/// The bounds of the files that podvm reads out of the image.
const MAX_KERNEL: u64 = 256 << 20;
const MAX_MODULE: u64 = 64 << 20;
const MAX_TEXT: u64 = 16 << 20;

/// The releases of the kernels of an image: each `boot/vmlinuz-RELEASE`
/// and each `RELEASE/vmlinuz` of the module directories, sorted, once.
pub fn releases(rootfs: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let names = |p: &str| -> Vec<String> {
        resolve(rootfs, p)
            .and_then(|d| std::fs::read_dir(d).ok())
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default()
    };
    for n in names("/boot") {
        if let Some(r) = n.strip_prefix("vmlinuz-").filter(|r| is_release(r)) {
            out.push(r.to_string());
        }
    }
    for dir in ["/usr/lib/modules", "/lib/modules"] {
        for r in names(dir) {
            if is_release(&r) && kernel_file(rootfs, &r).is_some() {
                out.push(r);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The kernel file of `release` in the image, where one is a file.
pub(super) fn kernel_file(rootfs: &Path, release: &str) -> Option<PathBuf> {
    [
        format!("/boot/vmlinuz-{release}"),
        format!("/usr/lib/modules/{release}/vmlinuz"),
        format!("/lib/modules/{release}/vmlinuz"),
    ]
    .iter()
    .filter_map(|p| resolve(rootfs, p))
    .find(|p| p.is_file())
}

/// The release of `want` among `have`, or the refusal that names them.
pub fn pick(want: &Want, have: &[String]) -> Result<String, String> {
    let list = if have.is_empty() {
        "none".to_string()
    } else {
        have.join(", ")
    };
    match want {
        Want::One if have.len() == 1 => Ok(have[0].clone()),
        Want::One if have.is_empty() => {
            Err("the image has no kernel: no /boot/vmlinuz-RELEASE and no \
             /usr/lib/modules/RELEASE/vmlinuz"
                .to_string())
        }
        Want::One => Err(format!(
            "the image has {} kernels ({list}): {VAR}=RELEASE takes one",
            have.len()
        )),
        Want::Release(r) if have.contains(r) => Ok(r.clone()),
        Want::Release(r) => Err(format!(
            "the image has no kernel of the release {r} (its kernels: {list})"
        )),
    }
}

/// The bytes of `p`, at most `max`; more is an error.
fn read_bounded(p: &Path, max: u64) -> Result<Vec<u8>, String> {
    let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let mut v = Vec::new();
    f.take(max + 1)
        .read_to_end(&mut v)
        .map_err(|e| format!("{}: {e}", p.display()))?;
    if v.len() as u64 > max {
        return Err(format!("{}: more than {max} bytes", p.display()));
    }
    Ok(v)
}

/// Check that `p` is a kernel of x86 that QEMU boots: a bzImage, with
/// the magic `HdrS` of its setup header at 0x202.
pub(super) fn check_bzimage(p: &Path) -> Result<(), String> {
    use std::os::unix::fs::FileExt;
    let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    if len > MAX_KERNEL {
        return Err(format!("{}: {len} bytes, over {MAX_KERNEL}", p.display()));
    }
    let mut magic = [0u8; 4];
    match f.read_exact_at(&mut magic, 0x202) {
        Ok(()) if &magic == b"HdrS" => Ok(()),
        _ => Err(format!(
            "{} is not a bzImage of x86 (no HdrS at 0x202)",
            p.display()
        )),
    }
}

/// The bytes of a module file, decompressed by its suffix, at most
/// [`MAX_MODULE`] each way.
pub fn read_module(p: &Path) -> Result<Vec<u8>, String> {
    read_module_within(p, MAX_MODULE)
}

/// [`read_module`] with the bound `max`.
pub(super) fn read_module_within(p: &Path, max: u64) -> Result<Vec<u8>, String> {
    let raw = read_bounded(p, max)?;
    let name = p.to_string_lossy();
    let what = |e: std::io::Error| format!("{}: {e}", p.display());
    let mut out = Vec::new();
    let raw = std::io::Cursor::new(raw);
    let r: Box<dyn Read> = if name.ends_with(".ko.gz") {
        Box::new(flate2::read::MultiGzDecoder::new(raw))
    } else if name.ends_with(".ko.xz") {
        Box::new(xz4rust::XzReader::new(raw))
    } else if name.ends_with(".ko.zst") {
        let d = ruzstd::decoding::StreamingDecoder::new(raw)
            .map_err(|e| format!("{}: {e}", p.display()))?;
        Box::new(d)
    } else {
        return Ok(raw.into_inner());
    };
    r.take(max + 1).read_to_end(&mut out).map_err(what)?;
    if out.len() as u64 > max {
        return Err(format!(
            "{}: decompressed to more than {max} bytes",
            p.display()
        ));
    }
    Ok(out)
}

/// The text of the first of `paths` (image paths) that is a file, or none.
pub(super) fn text(rootfs: &Path, paths: &[String]) -> Result<Option<String>, String> {
    for p in paths {
        if let Some(h) = resolve(rootfs, p).filter(|h| h.is_file()) {
            let b = read_bounded(&h, MAX_TEXT)?;
            return String::from_utf8(b)
                .map(Some)
                .map_err(|_| format!("{p}: not UTF-8 text"));
        }
    }
    Ok(None)
}
