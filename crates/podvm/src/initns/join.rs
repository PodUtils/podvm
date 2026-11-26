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

use crate::incus::systemd::{boot_bound, step, Step};

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

/// Put each later child of this process into the PID namespace of the
/// process `pid` (TODO/machine.md T-2008: the command of a held init, whose
/// supervisor is another process).
pub fn enter_pid_namespace(pid: i32) -> std::io::Result<()> {
    const CLONE_NEWPID: i32 = 0x2000_0000;
    let f = std::fs::File::open(format!("/proc/{pid}/ns/pid"))?;
    // SAFETY: setns with an open descriptor of a PID namespace.
    if unsafe { setns(f.as_raw_fd(), CLONE_NEWPID) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
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

    /// A command that joins before its `execve`, then takes `ids` with the
    /// supplementary `groups`, and the working directory `cwd` of the image.
    /// With `tty` (a path of the image), the command is the leader of a
    /// session of its own with that terminal on 0, 1 and 2.
    fn command(
        &self,
        program: &str,
        ids: Option<(u32, u32)>,
        groups: &[u32],
        cwd: &str,
        tty: Option<&str>,
    ) -> Command {
        let tty = tty.and_then(|t| std::ffi::CString::new(t).ok());
        // The list of setgroups: the primary group first, as login does.
        let list: Vec<u32> = ids
            .iter()
            .map(|(_, gid)| *gid)
            .chain(groups.iter().copied())
            .collect();
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
                if let Some(t) = &tty {
                    super::pty::take(t)?;
                }
                match ids {
                    Some((uid, gid)) => {
                        check(setgroups(list.len(), list.as_ptr()))?;
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

/// True for the error of a `systemctl` that does not know `--wait`
/// (before systemd 240).
pub fn no_wait_flag(stderr: &str) -> bool {
    stderr.contains("unrecognized option") || stderr.contains("invalid option")
}

/// The output of `c`, or `None` where `stop` says so before `c` ends (the
/// bound of the boot, or the end of the init): `c` is then killed.
fn output_until(
    mut c: Command,
    stop: &mut dyn FnMut() -> bool,
) -> std::io::Result<Option<std::process::Output>> {
    let mut child = c.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output().map(Some);
        }
        if stop() {
            child.kill()?;
            child.wait()?;
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Wait until the init is up. `Ok(true)`: systemd is up (`running` or
/// `degraded`). `Ok(false)`: the image has no `systemctl`, so its init has
/// no state to ask for, and the command starts at once. `ended` gives the
/// exit code of the init where it ended.
///
/// `systemctl is-system-running --wait` returns at the moment the boot is
/// complete, so no time goes to a poll (TODO/enter.md T-2010); before PID 1
/// has its bus, and with a systemctl that has no `--wait`, podvm asks again.
pub fn wait_boot(ns: &Namespaces, ended: &mut dyn FnMut() -> Option<i32>) -> Result<bool, String> {
    let bound = boot_bound()?;
    let end = Instant::now() + bound;
    let mut wait = true;
    let mut last = String::from("no answer");
    loop {
        if let Some(code) = ended() {
            return Err(format!(
                "the init of the image ended during its boot with status {code}"
            ));
        }
        let mut c = ns.command("systemctl", None, &[], "/", None);
        c.arg("is-system-running");
        if wait {
            c.arg("--wait");
        }
        c.env_clear().env("PATH", PATH).stdin(Stdio::null());
        let out = output_until(c, &mut || Instant::now() >= end || ended().is_some());
        match out {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(format!("systemctl is-system-running: {e}")),
            Ok(None) => {}
            Ok(Some(o)) => {
                let state = String::from_utf8_lossy(&o.stdout).trim().to_string();
                let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                last = format!("systemctl is-system-running gives {state:?}: {err}");
                if wait && no_wait_flag(&err) {
                    wait = false;
                    continue;
                }
                match step(&state, &err) {
                    Step::Run => return Ok(true),
                    Step::Again => {}
                    Step::Fail => return Err(format!("the init of the image is not up: {last}")),
                }
            }
        }
        if Instant::now() >= end {
            return Err(format!(
                "the init of the image did not finish its boot in {} s: {last}",
                bound.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// True for a variable of podvm's own entry, which the command of an init
/// of the image does not get: the tree has its real owners and a real
/// root, so no interposer loads.
pub(crate) fn internal(name: &str) -> bool {
    name == "LD_PRELOAD" || (name.starts_with("PODVM_") && name != "PODVM_INIT")
}

/// Run the command of the run in the namespaces of the init `init` (a pid
/// of this `/proc`), and wait for it with the forward of each shutdown
/// signal. Its exit code; 127 or 126 where the image has no such program
/// or refuses it, as docker. The environment is `env` (`KEY=VALUE`), else
/// the one of this process; a variable of podvm's own entry is left out.
pub fn run_command(
    ns: &Namespaces,
    spec: &super::boot::Spec,
    init: i32,
    env: Option<&[String]>,
) -> i32 {
    let Some(program) = spec.argv.first() else {
        eprintln!("podvm: --podvm-init=image: no command");
        return 125;
    };
    // TODO/enter.md T-2002: with `-t`, a pty of the devpts of the init. The
    // terminal that this process has belongs to another devpts, and has no
    // name in the image (`tty` gave "No such device").
    let inner = match spec.tty.then(|| super::pty::for_init(init)) {
        None => None,
        Some(Ok(p)) => Some(p),
        Some(Err(e)) => {
            eprintln!("podvm: --podvm-init=image: -t: a pty of the devpts of the init: {e}");
            return 125;
        }
    };
    let slave = inner.as_ref().map(|p| p.slave.as_str());
    let mut c = ns.command(program, spec.ids, &spec.groups, &spec.cwd, slave);
    c.args(&spec.argv[1..]).env_clear();
    // The bytes of a value stay as they are, also where they are no text.
    use std::ffi::OsString;
    let vars: Vec<(OsString, OsString)> = match env {
        Some(e) => e
            .iter()
            .filter_map(|e| e.split_once('='))
            .map(|(k, v)| (k.into(), v.into()))
            .collect(),
        None => std::env::vars_os().collect(),
    };
    let mut has_path = false;
    for (k, v) in vars {
        let name = k.to_string_lossy();
        if !internal(&name) {
            has_path |= name == "PATH";
            c.env(&k, v);
        }
    }
    if !has_path {
        c.env("PATH", PATH);
    }
    // The relay starts before the command: it is a process, as this one
    // makes no thread after it took the PID namespace of the init.
    let relay = match inner.map(super::pty::Inner::relay) {
        None => None,
        Some(Ok(r)) => Some(r),
        Some(Err(e)) => {
            eprintln!("podvm: --podvm-init=image: -t: the relay of the pty: {e}");
            return 125;
        }
    };
    let child = match c.spawn() {
        Ok(c) => c,
        Err(e) => {
            let code = match e.kind() {
                std::io::ErrorKind::NotFound => 127,
                std::io::ErrorKind::PermissionDenied => 126,
                _ => 125,
            };
            drop(relay);
            eprintln!("podvm: exec: {program:?}: {e}");
            return code;
        }
    };
    let pid = i64::from(child.id());
    // The wait of pod-enter reaps the child; the handle of std must not.
    std::mem::forget(child);
    let mut err = std::io::stderr();
    let code = podvm_enter::wait_forwarding(&podvm_enter::Child::of(pid), &mut err);
    drop(relay);
    match code {
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
    fn an_old_systemctl_is_read_from_its_error() {
        for (err, want) in [
            ("systemctl: unrecognized option '--wait'", true),
            ("systemctl: invalid option -- 'w'", true),
            ("Failed to connect to bus: No such file or directory", false),
            ("", false),
        ] {
            assert_eq!(super::no_wait_flag(err), want, "{err}");
        }
    }

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
