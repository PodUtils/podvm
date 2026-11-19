//! The join of the namespaces of an init of the image, for the state
//! queries of its boot and for the command of the run (TODO/enter.md
//! T-2002).
//!
//! A child of the supervisor is in the PID namespace of the init already
//! (`unshare(CLONE_NEWPID)` of the supervisor). Before its `execve` it
//! joins the cgroup, the IPC, the UTS and the mount namespace of the init;
//! the mount namespace last, as its root becomes the root of the child.

use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::incus::systemd::{step, Step, BOOT_BOUND};

/// The PATH of the state queries, and of an init with no PATH.
pub const PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

unsafe extern "C" {
    fn setns(fd: i32, nstype: i32) -> i32;
    fn setgroups(n: usize, list: *const u32) -> i32;
    fn setresgid(r: u32, e: u32, s: u32) -> i32;
    fn setresuid(r: u32, e: u32, s: u32) -> i32;
    fn chdir(path: *const std::ffi::c_char) -> i32;
}

/// Drop each supplementary group of this process; a refusal changes
/// nothing.
pub fn drop_groups() {
    // SAFETY: a count of 0 reads no list.
    unsafe { setgroups(0, std::ptr::null()) };
}

/// The namespaces of the init, each as an open descriptor, in join order.
pub struct Namespaces(Vec<OwnedFd>);

impl Namespaces {
    /// Open the namespaces of the process `pid` (a pid of this `/proc`).
    pub fn of(pid: i32, cgroup: bool) -> std::io::Result<Namespaces> {
        let mut kinds = vec!["ipc", "uts"];
        if cgroup {
            kinds.insert(0, "cgroup");
        }
        kinds.push("mnt");
        kinds
            .iter()
            .map(|k| std::fs::File::open(format!("/proc/{pid}/ns/{k}")).map(OwnedFd::from))
            .collect::<std::io::Result<Vec<_>>>()
            .map(Namespaces)
    }

    /// A command that joins before its `execve`, then takes `ids` and the
    /// working directory `cwd` of the image.
    fn command(&self, program: &str, ids: Option<(u32, u32)>, cwd: &str) -> Command {
        let fds: Vec<i32> = self.0.iter().map(|f| f.as_raw_fd()).collect();
        // An image with no working directory runs its command in `/`.
        let cwd = if cwd.is_empty() { "/" } else { cwd };
        let cwd = std::ffi::CString::new(cwd).unwrap_or_else(|_| c"/".to_owned());
        let mut c = Command::new(program);
        // SAFETY: the closure makes system calls only, on values that it owns.
        unsafe {
            c.pre_exec(move || {
                let check = |r: i32| {
                    if r == 0 {
                        Ok(())
                    } else {
                        Err(std::io::Error::last_os_error())
                    }
                };
                for fd in &fds {
                    check(setns(*fd, 0))?;
                }
                match ids {
                    Some((uid, gid)) => {
                        check(setgroups(1, &gid))?;
                        check(setresgid(gid, gid, gid))?;
                        check(setresuid(uid, uid, uid))?;
                    }
                    // Root of the image has no group of the caller: on the
                    // namespace rung they show as `nobody`. A map that
                    // denies setgroups keeps them.
                    None => drop_groups(),
                }
                check(chdir(cwd.as_ptr()))
            });
        }
        c
    }
}

/// Wait until the init is up. `Ok(true)`: systemd is up (`running` or
/// `degraded`). `Ok(false)`: the image has no `systemctl`, so its init has
/// no state to ask for, and the command starts at once. `ended` gives the
/// exit code of the init where it ended.
pub fn wait_boot(ns: &Namespaces, ended: &mut dyn FnMut() -> Option<i32>) -> Result<bool, String> {
    let end = Instant::now() + BOOT_BOUND;
    loop {
        if let Some(code) = ended() {
            return Err(format!(
                "the init of the image ended during its boot with status {code}"
            ));
        }
        let out = ns
            .command("systemctl", None, "/")
            .arg("is-system-running")
            .env_clear()
            .env("PATH", PATH)
            .stdin(Stdio::null())
            .output();
        let last = match out {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(format!("systemctl is-system-running: {e}")),
            Ok(o) => {
                let state = String::from_utf8_lossy(&o.stdout).trim().to_string();
                let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                let last = format!("systemctl is-system-running gives {state:?}: {err}");
                match step(&state, &err) {
                    Step::Run => return Ok(true),
                    Step::Again => {}
                    // "offline" before PID 1 made /run/systemd/system.
                    Step::Fail if state == "offline" || state == "unknown" => {}
                    Step::Fail => return Err(format!("the init of the image is not up: {last}")),
                }
                last
            }
        };
        if Instant::now() >= end {
            return Err(format!(
                "the init of the image did not finish its boot in {} s: {last}",
                BOOT_BOUND.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// True for a variable of podvm's own entry, which the command of an init
/// of the image does not get: the tree has its real owners and a real
/// root, so no interposer loads.
fn internal(name: &str) -> bool {
    name == "LD_PRELOAD" || (name.starts_with("PODVM_") && name != "PODVM_INIT")
}

/// Run the command of the run in the namespaces, and wait for it with the
/// forward of each shutdown signal. Its exit code; 127 or 126 where the
/// image has no such program or refuses it, as docker.
pub fn run_command(ns: &Namespaces, spec: &super::boot::Spec) -> i32 {
    let Some(program) = spec.argv.first() else {
        eprintln!("podvm: --podvm-init=image: no command");
        return 125;
    };
    let mut c = ns.command(program, spec.ids, &spec.cwd);
    c.args(&spec.argv[1..]).env_clear();
    let mut has_path = false;
    for (k, v) in std::env::vars_os() {
        let name = k.to_string_lossy();
        if !internal(&name) {
            has_path |= name == "PATH";
            c.env(k, v);
        }
    }
    if !has_path {
        c.env("PATH", PATH);
    }
    let child = match c.spawn() {
        Ok(c) => c,
        Err(e) => {
            let code = match e.kind() {
                std::io::ErrorKind::NotFound => 127,
                std::io::ErrorKind::PermissionDenied => 126,
                _ => 125,
            };
            eprintln!("podvm: exec: {program:?}: {e}");
            return code;
        }
    };
    let pid = i64::from(child.id());
    // The wait of pod-enter reaps the child; the handle of std must not.
    std::mem::forget(child);
    let mut err = std::io::stderr();
    match podvm_enter::Child::of(pid).wait_forwarding(&mut err) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("podvm: --podvm-init=image: the wait for the command: {e}");
            125
        }
    }
}

#[cfg(test)]
mod tests {
    use super::internal;

    #[test]
    fn the_command_gets_no_variable_of_the_entry() {
        for (name, want) in [
            ("LD_PRELOAD", true),
            ("PODVM_MAPS", true),
            ("PODVM_IDENTITY", true),
            ("PODVM_INIT", false),
            ("PATH", false),
            ("PODVMX", false),
            ("HOME", false),
        ] {
            assert_eq!(internal(name), want, "{name}");
        }
    }
}
