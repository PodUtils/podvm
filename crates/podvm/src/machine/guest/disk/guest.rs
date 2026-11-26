//! `podvm guest-disk --load M1,M2,... NEXT FAIL`: the root of a guest on
//! its sized disk, from inside the guest (TODO/machine.md T-1968).
//!
//! The init execs this as PID 1 after its first mounts. It loads the
//! modules, finds the virtio disk by its serial, mounts it as ext4, mounts
//! an overlay with the root of the initramfs as the lower layer and the
//! disk as the upper layer, moves `/dev`, `/proc` and `/sys` into the
//! overlay, makes the overlay the root (as `switch_root` does, but the
//! initramfs stays: it is the lower layer), and execs `/bin/sh NEXT`.
//! Each failure writes its reason to [`super::ERR`] and execs
//! `/bin/sh FAIL`, which ends the run with 125 before the command.
//!
//! `podvm guest-root --load M1,M2,... [--disk] NONCE` (TODO/machine.md
//! T-1994) is the first process of a guest whose root is the disk of its
//! image: the kernel runs it (`rdinit=/podvm/podvm -- guest-root ...`),
//! and the initramfs holds no shell. It mounts `/proc`, `/sys` and `/dev`,
//! loads the modules, mounts the root disk read-only, and makes the root an
//! overlay of the initramfs (podvm's files, on top) and the disk, with the
//! upper layer on the sized disk (`--disk`) or on a tmpfs. Then it execs
//! the image's `/bin/sh /podvm-init`. A failure ends the run with 125 and
//! its reason itself, with the markers of `NONCE`.

use std::ffi::CString;
use std::path::Path;
use std::time::{Duration, Instant};

unsafe extern "C" {
    fn mount(
        src: *const std::ffi::c_char,
        target: *const std::ffi::c_char,
        fstype: *const std::ffi::c_char,
        flags: u64,
        data: *const u8,
    ) -> i32;
    fn chroot(path: *const std::ffi::c_char) -> i32;
    fn reboot(cmd: i32) -> i32;
    fn sync();
}

const MS_RDONLY: u64 = 1;
const MS_MOVE: u64 = 8192;
/// Where the disks, the tmpfs of the upper layer and the new root are
/// mounted, in the initramfs.
const DISK: &str = "/podvm-disk";
const ROOT: &str = "/podvm-root";
const LOWER: &str = "/podvm-lower";
const UPPER: &str = "/podvm-upper";
/// The bound of the wait for the disk device after its driver loads.
const DEVICE_BOUND: Duration = Duration::from_secs(10);

fn c(s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|_| format!("{s:?} holds a NUL byte"))
}

/// mount(2); `fs` and `data` empty are none.
fn mount_at(src: &str, target: &str, fs: &str, flags: u64, data: &str) -> Result<(), String> {
    let (s, t, f, d) = (c(src)?, c(target)?, c(fs)?, c(data)?);
    let fs_ptr = if fs.is_empty() {
        std::ptr::null()
    } else {
        f.as_ptr()
    };
    let data_ptr = if data.is_empty() {
        std::ptr::null()
    } else {
        d.as_ptr() as *const u8
    };
    // SAFETY: mount(2) with NUL-terminated strings of this frame or null.
    if unsafe { mount(s.as_ptr(), t.as_ptr(), fs_ptr, flags, data_ptr) } != 0 {
        let how = if flags & MS_MOVE != 0 {
            "move".to_string()
        } else {
            format!("as {fs}")
        };
        return Err(format!(
            "mount {src} on {target} ({how}): {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// The device node of the virtio disk whose serial is `want`.
fn device(want: &str) -> Result<String, String> {
    let start = Instant::now();
    loop {
        if let Ok(rd) = std::fs::read_dir("/sys/block") {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let serial = std::fs::read_to_string(e.path().join("serial")).unwrap_or_default();
                let node = format!("/dev/{name}");
                if serial.trim() == want && Path::new(&node).exists() {
                    return Ok(node);
                }
            }
        }
        if start.elapsed() > DEVICE_BOUND {
            return Err(format!(
                "no virtio disk with the serial {want} came within {} s",
                DEVICE_BOUND.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn mkdir(p: &str) -> Result<(), String> {
    std::fs::create_dir_all(p).map_err(|e| format!("mkdir {p}: {e}"))
}

/// Where the upper layer of the root goes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Upper {
    /// The sized disk of `--storage-opt size=` (T-1968).
    Disk,
    /// A tmpfs in the memory of the guest.
    Tmpfs,
}

/// Each step up to the new root, an overlay of `lowerdir` with its upper
/// layer on `upper`; the process is then in it.
fn up(modules: &[&str], lowerdir: &str, upper: Upper) -> Result<(), String> {
    for m in modules {
        crate::machine::guest::net::load(m)?;
    }
    let base = match upper {
        Upper::Disk => {
            let dev = device(super::SERIAL)?;
            mkdir(DISK)?;
            mount_at(&dev, DISK, "ext4", 0, "")?;
            DISK
        }
        Upper::Tmpfs => {
            mkdir(UPPER)?;
            mount_at("tmpfs", UPPER, "tmpfs", 0, "mode=0755")?;
            UPPER
        }
    };
    let (upper, work) = (format!("{base}/upper"), format!("{base}/work"));
    mkdir(&upper)?;
    mkdir(&work)?;
    mkdir(ROOT)?;
    mount_at(
        "overlay",
        ROOT,
        "overlay",
        0,
        &format!("lowerdir={lowerdir},upperdir={upper},workdir={work}"),
    )?;
    for m in ["/dev", "/proc", "/sys"] {
        let to = format!("{ROOT}{m}");
        mkdir(&to)?;
        mount_at(m, &to, "", MS_MOVE, "")?;
    }
    std::env::set_current_dir(ROOT).map_err(|e| format!("cd {ROOT}: {e}"))?;
    mount_at(".", "/", "", MS_MOVE, "")?;
    let dot = c(".")?;
    // SAFETY: chroot(2) with a NUL-terminated string of this frame.
    if unsafe { chroot(dot.as_ptr()) } != 0 {
        return Err(format!(
            "chroot {ROOT}: {}",
            std::io::Error::last_os_error()
        ));
    }
    std::env::set_current_dir("/").map_err(|e| format!("cd / of the disk: {e}"))
}

/// exec `/bin/sh script`; it returns only on a failure.
fn exec_sh(script: &str) -> std::io::Error {
    use std::os::unix::process::CommandExt;
    std::process::Command::new("/bin/sh").arg(script).exec()
}

/// The subcommand. It does not return where an exec works.
pub fn main(args: &[String]) -> i32 {
    let (load, next, fail) = match args {
        [flag, load, next, fail] if flag == "--load" => (load, next, fail),
        _ => {
            eprintln!("podvm guest-disk: the arguments are --load M1,M2,... NEXT FAIL");
            return 125;
        }
    };
    let modules: Vec<&str> = load.split(',').filter(|m| !m.is_empty()).collect();
    let why = match up(&modules, "/", Upper::Disk) {
        Ok(()) => format!("the rest of the init: {}", exec_sh(next)),
        Err(e) => e,
    };
    if let Err(e) = std::fs::write(super::ERR, &why) {
        eprintln!("podvm guest-disk: {}: {e}", super::ERR);
    }
    let e = exec_sh(fail);
    eprintln!("podvm guest-disk: {why}; {fail}: {e}");
    125
}

/// The first mounts of a guest with no shell: `/proc`, `/sys` and
/// `/dev`, each where a file of it is not there yet (the kernel can mount
/// `devtmpfs` itself).
fn first_mounts() -> Result<(), String> {
    for (fs, at, mark) in [
        ("proc", "/proc", "/proc/self"),
        ("sysfs", "/sys", "/sys/kernel"),
        ("devtmpfs", "/dev", "/dev/null"),
    ] {
        if !Path::new(mark).exists() {
            mkdir(at)?;
            mount_at(fs, at, fs, 0, "")?;
        }
    }
    Ok(())
}

/// The root on the disk of the image (see the module note).
fn root_up(modules: &[&str], upper: Upper) -> Result<(), String> {
    first_mounts()?;
    for m in modules {
        crate::machine::guest::net::load(m)?;
    }
    let dev = device(super::ROOT_SERIAL)?;
    mkdir(LOWER)?;
    mount_at(&dev, LOWER, "ext4", MS_RDONLY, "")?;
    // The initramfs on top: podvm, its modules, `/podvm-init` and the
    // copies of the run win over a name of the image, as a later archive
    // of an initrd does.
    up(&[], &format!("/:{LOWER}"), upper)
}

/// The end of a guest whose root did not come: the markers of `nonce`
/// with 125, and the reason on the second port (stderr where it is not
/// there), then a reset, which ends the emulator (`-no-reboot`).
fn root_fail(nonce: &str, why: &str) -> ! {
    end_run(
        nonce,
        &format!(
            "podvm: machine tier: the root of the guest is not on the disk of its image: {why} \
             (TODO/machine.md T-1994)\n"
        ),
    )
}

/// The end of a run before its command: the ready marker of `nonce`,
/// `line` on the second port (stderr where it is not there), the exit
/// marker with 125, then a reset, which ends the emulator (`-no-reboot`).
pub(crate) fn end_run(nonce: &str, line: &str) -> ! {
    use std::io::Write;
    let say = |text: &str| {
        let mut out = std::io::stdout();
        out.write_all(text.as_bytes())
            .and_then(|()| out.flush())
            .is_ok()
    };
    // A line of its own: the console can hold the queries of an init.
    say(&format!("\nPODVM-READY-{nonce}\n"));
    let to_port = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/ttyS1")
        .and_then(|mut f| f.write_all(line.as_bytes()));
    if to_port.is_err() {
        eprint!("{line}");
    }
    say(&format!("\nPODVM-EXIT-{nonce} 125\n"));
    const RB_AUTOBOOT: i32 = 0x0123_4567;
    // SAFETY: sync(2) and reboot(2) take no pointer.
    unsafe {
        sync();
        reboot(RB_AUTOBOOT);
    }
    loop {
        std::thread::sleep(Duration::from_secs(3600));
    }
}

/// The arguments of `podvm guest-root`.
#[derive(Debug, PartialEq, Eq)]
pub struct RootArgs {
    pub load: Vec<String>,
    pub disk: bool,
    /// `--init=SECS`: exec the init of the image (systemd) with the bound
    /// of its boot (TODO/machine.md T-1994, step 2).
    pub init: Option<u64>,
    pub nonce: String,
}

/// `--load M1,M2,... [--disk] [--init=SECS] NONCE`.
pub fn root_args(args: &[String]) -> Result<RootArgs, String> {
    let usage = "the arguments are --load M1,M2,... [--disk] [--init=SECS] NONCE";
    let (nonce, flags) = args.split_last().ok_or(usage)?;
    if nonce.is_empty() || !nonce.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("{nonce:?} is not the nonce of a run"));
    }
    let mut a = RootArgs {
        load: Vec::new(),
        disk: false,
        init: None,
        nonce: nonce.clone(),
    };
    let mut it = flags.iter();
    while let Some(f) = it.next() {
        match f.as_str() {
            "--load" => {
                let l = it.next().ok_or(usage)?;
                a.load = l
                    .split(',')
                    .filter(|m| !m.is_empty())
                    .map(String::from)
                    .collect();
            }
            "--disk" => a.disk = true,
            _ => match f.strip_prefix("--init=").map(str::parse::<u64>) {
                Some(Ok(s)) if s > 0 => a.init = Some(s),
                _ => return Err(format!("{f:?}: {usage}")),
            },
        }
    }
    Ok(a)
}

/// exec the init of the image as PID 1; it returns only on a failure.
fn exec_init() -> std::io::Error {
    use std::os::unix::process::CommandExt;
    std::process::Command::new(crate::initns::INIT).exec()
}

/// `podvm guest-root`. It does not return.
pub fn root_main(args: &[String]) -> i32 {
    let a = match root_args(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("podvm guest-root: {e}");
            return 125;
        }
    };
    let modules: Vec<&str> = a.load.iter().map(String::as_str).collect();
    let upper = if a.disk { Upper::Disk } else { Upper::Tmpfs };
    let why = match (root_up(&modules, upper), a.init) {
        (Ok(()), None) => format!("the init of the run: {}", exec_sh("/podvm-init")),
        (Ok(()), Some(bound)) => {
            match crate::machine::guest::pid1::prepare(Path::new("/"), &a.nonce, bound) {
                Ok(()) => format!(
                    "the init of the image {}: {}",
                    crate::initns::INIT,
                    exec_init()
                ),
                Err(e) => format!("the unit of the run: {e}"),
            }
        }
        (Err(e), _) => e,
    };
    root_fail(&a.nonce, &why)
}
