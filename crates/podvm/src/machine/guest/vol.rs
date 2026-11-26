//! `podvm guest-9p TAG=PATH[:ro]...`: the volumes of a guest, from inside
//! it, TODO/machine.md T-1723.
//!
//! The host gives each `-v HOST:GUEST` to the emulator as
//! `-virtfs local,path=HOST,mount_tag=podvmN,security_model=none`. In the
//! guest this loads the 9p modules of the pinned kernel (out of its
//! modloop, [`super::kernel`]) with `finit_module(2)` and mounts each tag
//! at its path over virtio, so a write in the guest lands in the host
//! directory. `:ro` mounts read-only. podvm writes the files with the
//! host user's ownership (`security_model=none`): the guest's `chown`
//! of a file in a volume does not reach the host.
//!
//! TODO/machine.md T-2103 (PodUtils/podrunner#371): `-v HOST:GUEST:mapped`
//! gives `security_model=mapped-xattr,fmode=0644,dmode=0755`. QEMU makes
//! each file and directory as the host user, with those modes, and keeps
//! the owner, the group and the mode of the guest in `user.virtfs.*`
//! attributes of the host file. The guest mounts the tag with
//! `access=client`: its kernel checks each access against those owners, so
//! a user of the image acts as its own uid, and the host user can remove
//! each file with no root. The file system of HOST must take user
//! attributes ([`probe_xattrs`]).

#[cfg(test)]
mod tests; // TODO/machine.md T-2103

/// The modules, in the order that their symbols need.
pub const MODULES: [&str; 4] = ["netfs", "9pnet", "9pnet_virtio", "9p"];

/// The path of each module in the modloop, under `modules/KVER/`.
pub const MODLOOP_PATHS: [(&str, &str); 4] = [
    ("netfs", "kernel/fs/netfs/netfs.ko"),
    ("9pnet", "kernel/net/9p/9pnet.ko"),
    ("9pnet_virtio", "kernel/net/9p/9pnet_virtio.ko"),
    ("9p", "kernel/fs/9p/9p.ko"),
];

/// The mount options of each tag.
pub const OPTIONS: &str = "trans=virtio,version=9p2000.L,msize=262144";
/// The mount options of a mapped tag (T-2103): the guest kernel checks
/// each access against the owners and modes of the attributes.
pub const MAPPED_OPTIONS: &str = "trans=virtio,version=9p2000.L,msize=262144,access=client";
/// The model of a mapped tag, with the modes of the host files (T-2103).
pub const MAPPED_MODEL: &str = "security_model=mapped-xattr,fmode=0644,dmode=0755";

const MS_RDONLY: u64 = 1;

unsafe extern "C" {
    fn mount(
        src: *const std::ffi::c_char,
        target: *const std::ffi::c_char,
        fstype: *const std::ffi::c_char,
        flags: u64,
        data: *const u8,
    ) -> i32;
}

/// One volume: the tag of the emulator, the guest path, read-only or not,
/// and mapped (T-2103) or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vol {
    pub tag: String,
    pub guest: String,
    pub ro: bool,
    pub mapped: bool,
}

/// The volumes of a run, each with its tag `podvmN`; a guest path of
/// `mapped` is a mapped volume.
pub fn vols(guests: &[(String, bool)], mapped: &[String]) -> Vec<Vol> {
    guests
        .iter()
        .enumerate()
        .map(|(i, (g, ro))| Vol {
            tag: format!("podvm{i}"),
            guest: g.clone(),
            ro: *ro,
            mapped: mapped.contains(g),
        })
        .collect()
}

/// A value of an option of the emulator: a `,` of `s` doubles, as QEMU
/// reads it.
fn opt(s: &std::path::Path) -> String {
    s.display().to_string().replace(',', ",,")
}

/// The volumes of the `-v` of a run ([`super::Request::volumes`]).
pub fn of_run(volumes: &[(std::path::PathBuf, String, bool)], mapped: &[String]) -> Vec<Vol> {
    let g: Vec<_> = volumes.iter().map(|(_, g, ro)| (g.clone(), *ro)).collect();
    vols(&g, mapped)
}

/// The emulator's argument of `-virtfs` for one volume that is not mapped.
pub fn virtfs(host: &std::path::Path, v: &Vol) -> String {
    format!(
        "local,path={},mount_tag={},security_model=none{}",
        opt(host),
        v.tag,
        if v.ro { ",readonly=on" } else { "" }
    )
}

/// The emulator's arguments of one volume. TODO/machine.md T-2103: QEMU's
/// `-virtfs` does not pass `fmode` and `dmode` to its file system device
/// (measured with QEMU 11.1.3: the files came with 0600 and 0700), so a
/// mapped volume is an `-fsdev` and a `virtio-9p-pci` device.
pub fn argv(host: &std::path::Path, v: &Vol) -> Vec<String> {
    if !v.mapped {
        return vec!["-virtfs".to_string(), virtfs(host, v)];
    }
    vec![
        "-fsdev".to_string(),
        format!(
            "local,id={},path={},{MAPPED_MODEL}{}",
            v.tag,
            opt(host),
            if v.ro { ",readonly=on" } else { "" }
        ),
        "-device".to_string(),
        format!("virtio-9p-pci,fsdev={},mount_tag={}", v.tag, v.tag),
    ]
}

/// The argument of the subcommand for one volume:
/// `TAG=PATH[:mapped][:ro]`. A guest path holds no `:` (`-v` splits on it).
pub fn arg(v: &Vol) -> String {
    format!(
        "{}={}{}{}",
        v.tag,
        v.guest,
        if v.mapped { ":mapped" } else { "" },
        if v.ro { ":ro" } else { "" }
    )
}

/// Parse one argument of the subcommand.
pub fn parse(a: &str) -> Option<Vol> {
    let (tag, rest) = a.split_once('=')?;
    let (rest, ro) = rest
        .strip_suffix(":ro")
        .map_or((rest, false), |g| (g, true));
    let (guest, mapped) = rest
        .strip_suffix(":mapped")
        .map_or((rest, false), |g| (g, true));
    (!tag.is_empty() && guest.starts_with('/') && !guest.contains(':')).then(|| Vol {
        tag: tag.to_string(),
        guest: guest.to_string(),
        ro,
        mapped,
    })
}

unsafe extern "C" {
    fn lsetxattr(
        path: *const std::ffi::c_char,
        name: *const std::ffi::c_char,
        value: *const u8,
        size: usize,
        flags: i32,
    ) -> i32;
    fn lremovexattr(path: *const std::ffi::c_char, name: *const std::ffi::c_char) -> i32;
    fn lgetxattr(
        path: *const std::ffi::c_char,
        name: *const std::ffi::c_char,
        value: *mut u8,
        size: usize,
    ) -> isize;
}

/// TODO/machine.md T-2103: a `:mapped` volume needs the 9p modules (a copy
/// keeps no owner) and a file system of HOST with user attributes.
pub fn check_mapped(
    volumes: &[(std::path::PathBuf, String, bool)],
    mapped: &[String],
    nine: bool,
) -> Result<(), String> {
    for (host, guest, ro) in volumes.iter().filter(|v| mapped.contains(&v.1)) {
        let refuse = |why: &str| {
            format!(
                "-v {}:{guest}:mapped: {why}, and podvm does not take it as another volume \
                 (TODO/machine.md T-2103)",
                host.display()
            )
        };
        if !nine {
            return Err(refuse(
                "the kernel of PODVM_MACHINE_KERNEL has no 9p module, so the volume would be a \
                 copy that keeps no owner",
            ));
        }
        probe_xattrs(host, *ro).map_err(|e| refuse(&format!("{}: {e}", host.display())))?;
    }
    Ok(())
}

/// The attribute that the probe of a writable mapped volume sets and
/// removes on HOST.
pub const PROBE_NAME: &str = "user.podvm.probe";

/// TODO/machine.md T-2103: whether the file system of `host` keeps the
/// owners of a mapped volume. A writable volume needs a write of a user
/// attribute on HOST (which the probe removes again); a read-only one needs
/// a file system that has user attributes (a read of one gives `ENODATA`,
/// not `ENOTSUP`). `Err` is the cause, for one line of refusal.
pub fn probe_xattrs(host: &std::path::Path, ro: bool) -> Result<(), String> {
    use std::os::unix::ffi::OsStrExt;
    const ENODATA: i32 = 61;
    const ENOTSUP: i32 = 95;
    let p = std::ffi::CString::new(host.as_os_str().as_bytes())
        .map_err(|_| "the host path holds a NUL".to_string())?;
    let name = |n: &str| std::ffi::CString::new(n).map_err(|e| e.to_string());
    let why = |e: std::io::Error| match e.raw_os_error() {
        Some(ENOTSUP) => "its file system takes no user attributes (ENOTSUP)".to_string(),
        _ => format!("a user attribute on it: {e}"),
    };
    if ro {
        let n = name("user.virtfs.uid")?;
        // SAFETY: NUL-ended path and name; a size of 0 asks for the length.
        let r = unsafe { lgetxattr(p.as_ptr(), n.as_ptr(), std::ptr::null_mut(), 0) };
        let e = std::io::Error::last_os_error();
        return match (r, e.raw_os_error()) {
            (r, _) if r >= 0 => Ok(()),
            (_, Some(ENODATA)) => Ok(()),
            _ => Err(why(e)),
        };
    }
    let n = name(PROBE_NAME)?;
    // SAFETY: NUL-ended path and name; the value is 1 byte of this frame.
    if unsafe { lsetxattr(p.as_ptr(), n.as_ptr(), b"1".as_ptr(), 1, 0) } != 0 {
        return Err(why(std::io::Error::last_os_error()));
    }
    // SAFETY: NUL-ended path and name. ENODATA is a probe of another run
    // that removed the name first.
    if unsafe { lremovexattr(p.as_ptr(), n.as_ptr()) } != 0 {
        let e = std::io::Error::last_os_error();
        if e.raw_os_error() != Some(ENODATA) {
            return Err(format!(
                "the removal of the probe attribute {PROBE_NAME}: {e}"
            ));
        }
    }
    Ok(())
}

fn mount_one(v: &Vol) -> Result<(), String> {
    std::fs::create_dir_all(&v.guest).map_err(|e| format!("mkdir {}: {e}", v.guest))?;
    let c = |s: &str| std::ffi::CString::new(s).map_err(|e| e.to_string());
    let (src, target, fs) = (c(&v.tag)?, c(&v.guest)?, c("9p")?);
    let data = c(if v.mapped { MAPPED_OPTIONS } else { OPTIONS })?;
    let flags = if v.ro { MS_RDONLY } else { 0 };
    // SAFETY: mount(2) with four NUL-terminated strings of this frame.
    let r = unsafe {
        mount(
            src.as_ptr(),
            target.as_ptr(),
            fs.as_ptr(),
            flags,
            data.as_ptr() as *const u8,
        )
    };
    if r != 0 {
        return Err(format!(
            "mount {} on {}: {}",
            v.tag,
            v.guest,
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// `guest-9p --hold FILE TAG=PATH...` (TODO/machine.md T-2008): the mounts
/// go into the mount namespace of the held init of `FILE`, which booted
/// before the volumes of the run were known.
pub const HOLD_FLAG: &str = "--hold";

unsafe extern "C" {
    fn setns(fd: i32, nstype: i32) -> i32;
}

/// Join the mount namespace of the held init. `Ok(false)` where its boot
/// failed: no mount has a place then, and the run names the failure.
fn join_held(file: &str) -> Result<bool, String> {
    use std::os::fd::AsRawFd;
    const CLONE_NEWNS: i32 = 0x0002_0000;
    let pid = match crate::initns::hold::Held::read(std::path::Path::new(file))? {
        crate::initns::hold::Held::Up { pid, .. } => pid,
        crate::initns::hold::Held::Failed(_) => return Ok(false),
    };
    let ns = format!("/proc/{pid}/ns/mnt");
    let f = std::fs::File::open(&ns).map_err(|e| format!("{ns}: {e}"))?;
    // SAFETY: setns with an open descriptor; this process has one thread.
    if unsafe { setns(f.as_raw_fd(), CLONE_NEWNS) } != 0 {
        return Err(format!("{ns}: {}", std::io::Error::last_os_error()));
    }
    std::env::set_current_dir("/").map_err(|e| format!("the root of the held init: {e}"))?;
    Ok(true)
}

/// The subcommand. Exit 0 where each volume is mounted, 1 otherwise.
pub fn main(args: &[String]) -> i32 {
    let mut failed = Vec::new();
    for m in MODULES {
        if let Err(e) = super::net::load(m) {
            failed.push(e);
        }
    }
    let (held, args) = match args {
        [flag, file, rest @ ..] if flag == HOLD_FLAG => (Some(file), rest),
        _ => (None, args),
    };
    if let (true, Some(file)) = (failed.is_empty(), held) {
        match join_held(file) {
            Ok(true) => {}
            Ok(false) => return 0,
            Err(e) => failed.push(format!("the mount namespace of the held init: {e}")),
        }
    }
    if failed.is_empty() {
        for a in args {
            match parse(a) {
                Some(v) => {
                    if let Err(e) = mount_one(&v) {
                        failed.push(e);
                    }
                }
                None => failed.push(format!("{a:?} is not TAG=/PATH[:mapped][:ro]")),
            }
        }
    }
    for f in &failed {
        eprintln!("podvm guest-9p: {f}");
    }
    i32::from(!failed.is_empty())
}

/// The line of each `-v` of a run on stderr: a 9p mount where `nine`, else
/// a copy into the guest. TODO/machine.md T-1968: a sized disk (`disk`)
/// holds the root and not a 9p mount, so the line says that the disk does
/// not bound the volume.
pub fn notices(
    verb: &str,
    volumes: &[(std::path::PathBuf, String, bool)],
    mapped: &[String],
    nine: bool,
    disk: bool,
) {
    let unbounded = if disk {
        "; --storage-opt size= does not bound it"
    } else {
        ""
    };
    for (host, guest, ro) in volumes {
        let host = host.display();
        if nine && mapped.contains(guest) {
            eprintln!(
                "podvm {verb}: machine tier: -v {host}:{guest}:mapped{} is a 9p mount with \
                 {MAPPED_MODEL}: the owners and modes of the guest live in user.virtfs.* \
                 attributes of the host files, which the host user owns{unbounded} \
                 (TODO/machine.md T-2103)",
                if *ro { ",ro" } else { "" }
            );
        } else if nine && *ro {
            eprintln!(
                "podvm {verb}: machine tier: -v {host}:{guest}:ro is a read-only 9p mount of the \
                 host directory (TODO/machine.md T-1723)"
            );
        } else if nine {
            eprintln!(
                "podvm {verb}: machine tier: -v {host}:{guest} is a 9p mount of the host \
                 directory: a write in the guest lands there, with the host user's ownership\
                 {unbounded} (TODO/machine.md T-1723)"
            );
        } else {
            eprintln!(
                "podvm {verb}: machine tier: -v {host}:{guest} is a copy into the guest: the \
                 kernel of PODVM_MACHINE_KERNEL has no 9p module, so writes in the guest do not \
                 return to the host (TODO/machine.md T-1723)"
            );
        }
    }
}
