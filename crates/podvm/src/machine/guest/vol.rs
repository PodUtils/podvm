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

/// One volume: the tag of the emulator, the guest path, read-only or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vol {
    pub tag: String,
    pub guest: String,
    pub ro: bool,
}

/// The volumes of a run, each with its tag `podvmN`.
pub fn vols(guests: &[(String, bool)]) -> Vec<Vol> {
    guests
        .iter()
        .enumerate()
        .map(|(i, (g, ro))| Vol {
            tag: format!("podvm{i}"),
            guest: g.clone(),
            ro: *ro,
        })
        .collect()
}

/// The emulator's argument of one volume.
pub fn virtfs(host: &std::path::Path, v: &Vol) -> String {
    format!(
        "local,path={},mount_tag={},security_model=none{}",
        host.display(),
        v.tag,
        if v.ro { ",readonly=on" } else { "" }
    )
}

/// The argument of the subcommand for one volume.
pub fn arg(v: &Vol) -> String {
    format!("{}={}{}", v.tag, v.guest, if v.ro { ":ro" } else { "" })
}

/// Parse one argument of the subcommand.
pub fn parse(a: &str) -> Option<Vol> {
    let (tag, rest) = a.split_once('=')?;
    let (guest, ro) = match rest.strip_suffix(":ro") {
        Some(g) => (g, true),
        None => (rest, false),
    };
    (!tag.is_empty() && guest.starts_with('/')).then(|| Vol {
        tag: tag.to_string(),
        guest: guest.to_string(),
        ro,
    })
}

fn mount_one(v: &Vol) -> Result<(), String> {
    std::fs::create_dir_all(&v.guest).map_err(|e| format!("mkdir {}: {e}", v.guest))?;
    let c = |s: &str| std::ffi::CString::new(s).map_err(|e| e.to_string());
    let (src, target, fs) = (c(&v.tag)?, c(&v.guest)?, c("9p")?);
    let data = c(OPTIONS)?;
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
                None => failed.push(format!("{a:?} is not TAG=/PATH[:ro]")),
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
pub fn notices(verb: &str, volumes: &[(std::path::PathBuf, String, bool)], nine: bool, disk: bool) {
    let unbounded = if disk {
        "; --storage-opt size= does not bound it"
    } else {
        ""
    };
    for (host, guest, ro) in volumes {
        let host = host.display();
        if nine && *ro {
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
