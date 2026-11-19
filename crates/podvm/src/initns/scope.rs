//! TODO/enter.md T-2004: a delegated cgroup2 subtree from the user manager
//! of systemd, for an init of the image on the namespace rung.
//!
//! The shell of a login is in `session-N.scope`, which belongs to root. The
//! user manager (`user@UID.service`) owns a delegated subtree, and it moves
//! a process of its user into a new scope there on request
//! (`StartTransientUnit` with `PIDs=` and `Delegate=yes`, as rootless
//! podman and crun ask). podvm asks with `busctl` of the host, before the
//! user namespace, and reads `/proc/self/cgroup` until the move is done.
//! The scope ends with its last process.
//!
//! Each step that fails gives its reason; the caller then goes on as with
//! no delegated subtree.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The bound of the call of `busctl`, and of the move after it.
const BOUND: Duration = Duration::from_secs(5);

unsafe extern "C" {
    fn getuid() -> u32;
}

/// The socket of the user bus: `$XDG_RUNTIME_DIR/bus`, else
/// `/run/user/UID/bus`. `None` where no such socket exists.
pub fn user_bus(runtime_dir: Option<&str>, uid: u32) -> Option<PathBuf> {
    let dir = match runtime_dir.filter(|d| d.starts_with('/')) {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(format!("/run/user/{uid}")),
    };
    let bus = dir.join("bus");
    std::fs::metadata(&bus).is_ok().then_some(bus)
}

/// The arguments of `busctl` for a scope `name` with the process `pid`.
pub fn call_args(name: &str, pid: u32) -> Vec<String> {
    [
        "--user",
        "--no-pager",
        "call",
        "org.freedesktop.systemd1",
        "/org/freedesktop/systemd1",
        "org.freedesktop.systemd1.Manager",
        "StartTransientUnit",
        "ssa(sv)a(sa(sv))",
        name,
        "fail",
        "2",
        "PIDs",
        "au",
        "1",
        &pid.to_string(),
        "Delegate",
        "b",
        "true",
        "0",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// True where the membership text of `/proc/self/cgroup` names the scope.
pub fn moved(membership: &str, name: &str) -> bool {
    membership
        .lines()
        .filter_map(|l| l.strip_prefix("0::"))
        .any(|p| p.rsplit('/').next() == Some(name))
}

fn tool(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// Ask the user manager for a scope with `Delegate=yes` that holds this
/// process. `Ok` where `/proc/self/cgroup` names the scope after the call.
pub fn ask() -> Result<(), String> {
    // SAFETY: getuid cannot fail.
    let uid = unsafe { getuid() };
    let runtime = std::env::var("XDG_RUNTIME_DIR").ok();
    let bus = user_bus(runtime.as_deref(), uid).ok_or("no user bus of systemd answers")?;
    let busctl = tool("busctl").ok_or("the host has no busctl")?;
    let nonce = crate::machine::guest::init::nonce()?;
    let name = format!("podvm-init-{nonce}.scope");
    let dir = bus.parent().unwrap_or(Path::new("/"));
    let mut child = Command::new(busctl)
        .args(call_args(&name, std::process::id()))
        .env("XDG_RUNTIME_DIR", dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("busctl: {e}"))?;
    let end = Instant::now() + BOUND;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "the user manager gave no answer in {} s",
                    BOUND.as_secs()
                ));
            }
            Err(e) => return Err(format!("busctl: {e}")),
        }
    };
    if !status.success() {
        let mut text = String::new();
        if let Some(mut e) = child.stderr.take() {
            use std::io::Read;
            let _ = e.read_to_string(&mut text);
        }
        let line = text.lines().next().unwrap_or("no line").trim().to_string();
        return Err(format!("the user manager refused a scope: {line}"));
    }
    let end = Instant::now() + BOUND;
    loop {
        let membership = std::fs::read_to_string("/proc/self/cgroup").unwrap_or_default();
        if moved(&membership, &name) {
            return Ok(());
        }
        if Instant::now() >= end {
            return Err(format!(
                "the user manager did not move podvm into {name} in {} s",
                BOUND.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
