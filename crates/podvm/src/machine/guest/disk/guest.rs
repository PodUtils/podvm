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
}

const MS_MOVE: u64 = 8192;
/// Where the disk and the new root are mounted, in the initramfs.
const DISK: &str = "/podvm-disk";
const ROOT: &str = "/podvm-root";
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

/// The device node of the virtio disk whose serial is [`super::SERIAL`].
fn device() -> Result<String, String> {
    let start = Instant::now();
    loop {
        if let Ok(rd) = std::fs::read_dir("/sys/block") {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let serial = std::fs::read_to_string(e.path().join("serial")).unwrap_or_default();
                let node = format!("/dev/{name}");
                if serial.trim() == super::SERIAL && Path::new(&node).exists() {
                    return Ok(node);
                }
            }
        }
        if start.elapsed() > DEVICE_BOUND {
            return Err(format!(
                "no virtio disk with the serial {} came within {} s",
                super::SERIAL,
                DEVICE_BOUND.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn mkdir(p: &str) -> Result<(), String> {
    std::fs::create_dir_all(p).map_err(|e| format!("mkdir {p}: {e}"))
}

/// Each step up to the new root; the process is then in it.
fn up(modules: &[&str]) -> Result<(), String> {
    for m in modules {
        crate::machine::guest::net::load(m)?;
    }
    let dev = device()?;
    mkdir(DISK)?;
    mount_at(&dev, DISK, "ext4", 0, "")?;
    let (upper, work) = (format!("{DISK}/upper"), format!("{DISK}/work"));
    mkdir(&upper)?;
    mkdir(&work)?;
    mkdir(ROOT)?;
    mount_at(
        "overlay",
        ROOT,
        "overlay",
        0,
        &format!("lowerdir=/,upperdir={upper},workdir={work}"),
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
    let why = match up(&modules) {
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
