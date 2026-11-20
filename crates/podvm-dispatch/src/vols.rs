//! The volumes of a run of the syscall tier: each host directory opened
//! once, above the payload's usual range (TODO/enter.md T-1724, T-1972).

use crate::state;
use crate::sys::{self, nr, sc};
use std::ffi::OsString;
use std::os::unix::ffi::OsStrExt;
use std::sync::atomic::Ordering;

/// The calls of `pod-beneath` through the trap of the handler: `syscall`
/// instructions of podvm's own code, which Syscall User Dispatch lets
/// through. One call each: the walk tries `EAGAIN` again itself.
struct Trap;

impl pod_beneath::Calls for Trap {
    fn openat2(
        &self,
        dirfd: core::ffi::c_int,
        path: &core::ffi::CStr,
        how: &pod_beneath::OpenHow,
    ) -> Result<core::ffi::c_int, core::ffi::c_int> {
        // SAFETY: the NUL-ended path and the `open_how` live for the call,
        // and the size is the size of that struct.
        let r = unsafe {
            sys::sc6(
                nr::OPENAT2,
                dirfd as i64,
                path.as_ptr() as i64,
                how as *const pod_beneath::OpenHow as i64,
                core::mem::size_of::<pod_beneath::OpenHow>() as i64,
                0,
                0,
            )
        };
        if r < 0 {
            return Err(-r as core::ffi::c_int);
        }
        Ok(r as core::ffi::c_int)
    }

    fn close(&self, fd: core::ffi::c_int) {
        sys::close(fd as i64);
    }
}

/// The directory `host` opened under `base` with no symbolic link and
/// nothing above the base (podcrates' `pod-beneath`): a descriptor, `None`
/// where `host` is not under `base`, or the refusal.
fn beneath(base: &[u8], host: &[u8]) -> Result<Option<i64>, String> {
    let mut buf = [0u8; state::PATH_MAX];
    match pod_beneath::open_under_base(
        &Trap,
        base,
        host,
        pod_beneath::O_PATH | pod_beneath::O_DIRECTORY,
        &mut buf,
    ) {
        Ok(fd) => Ok(Some(fd as i64)),
        Err(pod_beneath::Refusal::NotUnderBase) => Ok(None),
        Err(r @ (pod_beneath::Refusal::Link | pod_beneath::Refusal::DotDot)) => Err(format!(
            "its host path under its base (--podvm-volume-base): {}",
            r.text()
        )),
        Err(r) => Err(format!("its host path under its base: {r}")),
    }
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
        let held = match base {
            Some(b) => beneath(b, host)
                .map_err(|e| format!("the volume {}: {e}", String::from_utf8_lossy(pair)))?,
            None => None,
        };
        let (fd, z) = match held {
            Some(fd) => {
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
        let b = base.to_str().unwrap();
        for (host, opens) in [
            (format!("{b}/work/sub"), Some(true)),
            (b.to_string(), Some(true)),
            (format!("{b}/away"), None),
            (format!("{b}/near"), None),
            (format!("{b}/near/sub"), None),
            ("/elsewhere".to_string(), Some(false)),
        ] {
            match super::beneath(b.as_bytes(), host.as_bytes()) {
                Ok(Some(fd)) => {
                    assert_eq!(opens, Some(true), "{host}");
                    crate::sys::close(fd);
                }
                Ok(None) => assert_eq!(opens, Some(false), "{host}"),
                Err(e) => {
                    assert_eq!(opens, None, "{host}: {e}");
                    assert!(e.contains("symbolic link"), "{host}: {e}");
                }
            }
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
