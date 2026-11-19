//! The volumes of a run of the syscall tier: each host directory opened
//! once, above the payload's usual range (TODO/enter.md T-1724, T-1972).

use crate::state;
use crate::sys::{self, nr, sc};
use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::sync::atomic::Ordering;

/// `host` under the directory `base`: the base and the rest after it
/// (`.` for the base itself), or `None` where it is not under it.
fn under_base<'a>(base: &'a [u8], host: &'a [u8]) -> Option<(&'a [u8], &'a [u8])> {
    let b = base.strip_suffix(b"/").unwrap_or(base);
    if host == b {
        return Some((b, b"."));
    }
    host.strip_prefix(b)
        .and_then(|r| r.strip_prefix(b"/"))
        .filter(|r| !r.is_empty())
        .map(|r| (b, r))
}

/// The directory `rest` opened under `base` with no symbolic link and
/// nothing above the base: a descriptor, or a negative errno (`-40` ELOOP
/// or `-18` EXDEV for a link).
fn beneath(base: &[u8], rest: &[u8]) -> Result<i64, String> {
    let mut zb = base.to_vec();
    zb.push(0);
    let bfd = sys::openat2(
        sys::AT_FDCWD,
        &zb,
        sys::O_PATH | sys::O_DIRECTORY | sys::O_CLOEXEC,
        0,
        0,
    );
    if bfd < 0 {
        return Err(format!("the volume base: errno {}", -bfd));
    }
    let mut zr = rest.to_vec();
    zr.push(0);
    let fd = sys::openat2(
        bfd,
        &zr,
        sys::O_PATH | sys::O_DIRECTORY | sys::O_CLOEXEC,
        0,
        sys::RESOLVE_BENEATH | sys::RESOLVE_NO_SYMLINKS,
    );
    sys::close(bfd);
    if fd == -40 || fd == -18 {
        return Err(
            "its host path holds a symbolic link under its base (--podvm-volume-base)".into(),
        );
    }
    Ok(fd)
}

/// The volumes of `PODVM_MAPS` in `envp` (`guest:host` pairs joined by
/// commas; the root pair `/:ROOTFS` is the root): each host directory
/// opened above the payload's usual range (TODO/enter.md T-1724).
pub(crate) fn volumes(envp: &[OsString]) -> Result<(), String> {
    let Some(table) = envp
        .iter()
        .find_map(|e| e.as_bytes().strip_prefix(b"PODVM_MAPS="))
    else {
        return Ok(());
    };
    // PodUtils/podrunner#300 ask 3 (TODO/enter.md T-1972): a volume under
    // `PODVM_VOLUME_BASE` opens under it with no symbolic link; each later
    // path of the volume resolves under that descriptor (`RESOLVE_IN_ROOT`).
    let base = envp
        .iter()
        .find_map(|e| e.as_bytes().strip_prefix(b"PODVM_VOLUME_BASE="))
        .filter(|b| b.starts_with(b"/"));
    let mut n = 0usize;
    for pair in table.split(|&c| c == b',').filter(|p| !p.is_empty()) {
        let Some(cut) = pair.iter().position(|&c| c == b':') else {
            continue;
        };
        let (guest, host) = (&pair[..cut], &pair[cut + 1..]);
        if guest == b"/" {
            continue;
        }
        if n == state::MAX_VOLS {
            return Err(format!("more than {} volumes", state::MAX_VOLS));
        }
        let (fd, z) = match base.and_then(|b| under_base(b, host)) {
            Some((b, rest)) => {
                let fd = beneath(b, rest)
                    .map_err(|e| format!("the volume {}: {e}", String::from_utf8_lossy(pair)))?;
                let mut z = host.to_vec();
                z.push(0);
                (fd, z)
            }
            None => {
                let host = std::fs::canonicalize(std::ffi::OsStr::from_bytes(host))
                    .map_err(|e| format!("the volume {}: {e}", String::from_utf8_lossy(pair)))?;
                let mut z = host.as_os_str().as_bytes().to_vec();
                z.push(0);
                // TODO/cli.md T-2001 (OP-02): a volume of one file too.
                let fd = sys::openat2(sys::AT_FDCWD, &z, sys::O_PATH | sys::O_CLOEXEC, 0, 0);
                (fd, z)
            }
        };
        let host = std::path::Path::new(std::ffi::OsStr::from_bytes(&z[..z.len() - 1]));
        // TODO/image.md T-1774: podvm maps a shimmed device of the tree to
        // the host's device for the loader. This rung serves directory
        // volumes only, so the pair stays out and the tree's node serves.
        if fd < 0 {
            return Err(format!("the volume {}: errno {}", host.display(), -fd));
        }
        let kind = mode_of(fd) & S_IFMT;
        if kind != S_IFDIR && guest == &pair[cut + 1..] && guest.starts_with(b"/dev/") {
            sys::close(fd);
            continue;
        }
        if kind != S_IFDIR && kind != S_IFREG {
            sys::close(fd);
            return Err(format!(
                "the volume {}: neither a directory nor a regular file",
                host.display()
            ));
        }
        // SAFETY: fcntl and close read no memory.
        let high = unsafe { sc(nr::FCNTL, fd, 1030, 1001) };
        let fd = if high >= 0 {
            sys::close(fd);
            high
        } else {
            fd
        };
        let v = &state::VOLS[n];
        v.guest.set(guest.strip_suffix(b"/").unwrap_or(guest));
        v.host.set(&z[..z.len() - 1]);
        v.fd.store(fd, Ordering::Relaxed);
        v.file.store(kind == S_IFREG, Ordering::Relaxed);
        n += 1;
    }
    state::NVOLS.store(n, Ordering::Release);
    // TODO/enter.md T-1901: the `:ro` volumes of `PODVM_RO`.
    if let Some(ro) = envp
        .iter()
        .find_map(|e| e.as_bytes().strip_prefix(b"PODVM_RO="))
    {
        crate::ro::mark(ro);
    }
    // TODO/enter.md T-1908: the name of `--hostname`.
    if let Some(name) = envp
        .iter()
        .find_map(|e| e.as_bytes().strip_prefix(b"PODVM_HOSTNAME="))
        && !crate::uts::set(name)
    {
        return Err("PODVM_HOSTNAME is empty or longer than 64 bytes".into());
    }
    Ok(())
}

const S_IFMT: u32 = 0o170000;
const S_IFDIR: u32 = 0o040000;
const S_IFREG: u32 = 0o100000;

/// `st_mode` of `fd`, by `fstat`; 0 where it fails.
fn mode_of(fd: i64) -> u32 {
    let mut st = [0u8; 144];
    // SAFETY: fstat writes one `struct stat` of 144 bytes on x86_64.
    if unsafe { sc(nr::FSTAT, fd, st.as_mut_ptr() as i64, 0) } != 0 {
        return 0;
    }
    u32::from_ne_bytes([st[24], st[25], st[26], st[27]])
}

#[cfg(test)]
mod tests {
    /// T-1972: a directory under the base opens; a link under the base, to
    /// a directory in it or out of it, does not; a path out of the base is
    /// no business of the base.
    #[test]
    fn a_volume_opens_under_its_base_with_no_link() {
        let d = std::env::temp_dir().join(format!("podvm-vols-{}", std::process::id()));
        let (base, out) = (d.join("base"), d.join("out"));
        std::fs::create_dir_all(base.join("work/sub")).unwrap();
        std::fs::create_dir_all(&out).unwrap();
        std::os::unix::fs::symlink(&out, base.join("away")).unwrap();
        std::os::unix::fs::symlink(base.join("work"), base.join("near")).unwrap();
        let b = base.to_str().unwrap().as_bytes();
        let fd = super::beneath(b, b"work/sub").unwrap();
        assert!(fd >= 0, "{fd}");
        crate::sys::close(fd);
        for link in [&b"away"[..], b"near", b"near/sub"] {
            assert!(
                super::beneath(b, link).is_err(),
                "{}",
                String::from_utf8_lossy(link)
            );
        }
        assert_eq!(super::under_base(b, b"/elsewhere"), None);
        assert_eq!(super::under_base(b, b), Some((b, &b"."[..])));
        let _ = std::fs::remove_dir_all(&d);
    }
}
