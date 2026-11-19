//! TODO/enter.md T-2007: `run --podvm-init=image` on the runsc tier. The
//! init of the image (systemd) is PID 1 of a gVisor sandbox: the Sentry
//! gives it a cgroup2 tree of its own (`--in-sandbox-cgroup=v2`) and each
//! uid, so the host needs no delegated cgroup and no subordinate id range.
//! The program code runs on the CPU of the host; each system call goes to
//! the Sentry.
//!
//! The bundle starts `podvm guest-init --pid1` ([`crate::initns::sandbox`]),
//! which gives the files their owners and becomes `/sbin/init`. podvm then
//! asks `systemctl is-system-running` by `runsc exec` until the init is up,
//! runs the command by `runsc exec` with the user, the environment and the
//! working directory of the run, and ends the sandbox.
//!
//! The writes of the root go to an overlay in a file of the container's own
//! tree (`--overlay2=root:self`): a `chown` to an id that the host does not
//! map holds there, and the file goes with the tree.

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

use crate::incus::systemd::{step, Step, BOOT_BOUND};

use super::Job;

/// podvm and the sidecar of the image in the sandbox.
pub const PODVM: &str = "/.podvm/podvm";
pub const SIDECAR: &str = "/.podvm/meta.jsonl";
/// The unit that no sandbox kernel serves: the Sentry has no `/proc/sys`
/// that takes a write, and `systemd-sysctl` ends with exit 1 there.
pub const MASKED: &[&str] = &["systemd-sysctl.service"];
/// The capabilities of the init: each one but those that hold nothing in
/// a sandbox with the network of the host and no module or clock of its
/// own (CAP_NET_ADMIN, CAP_SYS_MODULE, CAP_SYS_RAWIO, CAP_SYS_TIME). A
/// unit with `ConditionCapability=` of one of them is skipped.
const CAPS: &[&str] = &[
    "CAP_CHOWN",
    "CAP_DAC_OVERRIDE",
    "CAP_DAC_READ_SEARCH",
    "CAP_FOWNER",
    "CAP_FSETID",
    "CAP_KILL",
    "CAP_SETGID",
    "CAP_SETUID",
    "CAP_SETPCAP",
    "CAP_LINUX_IMMUTABLE",
    "CAP_NET_BIND_SERVICE",
    "CAP_NET_BROADCAST",
    "CAP_NET_RAW",
    "CAP_IPC_LOCK",
    "CAP_IPC_OWNER",
    "CAP_SYS_CHROOT",
    "CAP_SYS_PTRACE",
    "CAP_SYS_PACCT",
    "CAP_SYS_ADMIN",
    "CAP_SYS_BOOT",
    "CAP_SYS_NICE",
    "CAP_SYS_RESOURCE",
    "CAP_SYS_TTY_CONFIG",
    "CAP_MKNOD",
    "CAP_LEASE",
    "CAP_AUDIT_WRITE",
    "CAP_AUDIT_CONTROL",
    "CAP_SETFCAP",
    "CAP_SYSLOG",
];
/// The bound of the stop of the init after the command, as `docker stop`.
const STOP_BOUND: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(20);

/// The shutdown signal that this process got, or 0.
static SIGNAL: AtomicI32 = AtomicI32::new(0);

unsafe extern "C" {
    fn signal(sig: i32, handler: usize) -> usize;
}

extern "C" fn note(sig: i32) {
    SIGNAL.store(sig, Ordering::Relaxed);
}

/// The OCI configuration of a bundle whose process is the init.
pub fn spec(job: &Job, podvm: &Path, sidecar: &Path) -> serde_json::Value {
    use serde_json::json;
    let mut s = super::spec(job);
    let mut args = vec![
        PODVM.to_string(),
        crate::initns::VERB.to_string(),
        crate::initns::sandbox::FLAG.to_string(),
        "--owners".to_string(),
        SIDECAR.to_string(),
    ];
    for unit in MASKED {
        args.extend(["--mask".to_string(), unit.to_string()]);
    }
    let caps: Vec<&str> = CAPS.to_vec();
    s["process"] = json!({
        "terminal": false,
        "user": {"uid": 0, "gid": 0},
        "args": args,
        "env": ["container=podvm"],
        "cwd": "/",
        "capabilities": {"bounding": caps, "effective": caps, "inheritable": caps,
                         "permitted": caps},
        "rlimits": [{"type": "RLIMIT_NOFILE", "hard": 1_048_576, "soft": 1024}],
    });
    // systemd mounts each of these itself where it finds no mount point,
    // with options that the Sentry refuses (`usrquota` of tmpfs, and
    // `memory_hugetlb_accounting` of cgroup2 from systemd 262).
    let own = json!([
        {"destination": "/dev/shm", "type": "tmpfs", "source": "shm",
         "options": ["nosuid", "nodev", "mode=1777"]},
        {"destination": "/dev/pts", "type": "devpts", "source": "devpts",
         "options": ["nosuid", "noexec", "newinstance", "ptmxmode=0666", "mode=0620"]},
        {"destination": "/sys/fs/cgroup", "type": "cgroup", "source": "cgroup",
         "options": ["rw", "nosuid", "noexec", "nodev"]},
        {"destination": "/run", "type": "tmpfs", "source": "tmpfs",
         "options": ["nosuid", "nodev", "mode=755"]},
        {"destination": "/tmp", "type": "tmpfs", "source": "tmpfs",
         "options": ["nosuid", "nodev", "mode=1777"]},
        {"destination": PODVM, "type": "bind", "source": podvm, "options": ["rbind", "ro"]},
        {"destination": SIDECAR, "type": "bind", "source": sidecar, "options": ["rbind", "ro"]},
    ]);
    if let (Some(mounts), Some(own)) = (s["mounts"].as_array_mut(), own.as_array()) {
        // After `/proc`, `/dev` and `/sys`, and before the volumes: a
        // volume at or under `/tmp` lies on the tmpfs.
        let at = mounts.len().min(3);
        for (i, m) in own.iter().enumerate() {
            mounts.insert(at + i, m.clone());
        }
    }
    s
}

/// The arguments of `runsc run` for the init: those of the tier, with the
/// cgroup2 of the Sentry, set-user-ID programs, and the overlay in a file.
pub fn args(state: &Path, bundle: &Path, id: &str) -> Vec<String> {
    let mut a: Vec<String> = super::args(state, bundle, id)
        .into_iter()
        .filter(|x| !x.starts_with("--overlay2="))
        .collect();
    let at = a.iter().position(|x| x == "run").unwrap_or(a.len());
    for (i, flag) in [
        "--overlay2=root:self",
        "--in-sandbox-cgroup=v2",
        "--allow-suid",
    ]
    .iter()
    .enumerate()
    {
        a.insert(at + i, flag.to_string());
    }
    a
}

/// The arguments of `runsc exec` for `argv` in the sandbox `id`.
pub fn exec_args(
    state: &Path,
    id: &str,
    user: Option<(u32, u32)>,
    cwd: &str,
    env: &[String],
    argv: &[String],
) -> Vec<String> {
    let mut a = vec![
        "--rootless".to_string(),
        format!("--root={}", state.display()),
        "exec".to_string(),
    ];
    if let Some((uid, gid)) = user {
        a.push(format!("--user={uid}:{gid}"));
    }
    a.push(format!("--cwd={}", if cwd.is_empty() { "/" } else { cwd }));
    for e in env {
        a.push(format!("--env={e}"));
    }
    a.push(id.to_string());
    a.extend(argv.iter().cloned());
    a
}

/// True where the tree has a `systemctl`: the init then has a state to
/// ask for.
fn has_systemctl(rootfs: &Path) -> bool {
    ["usr/bin", "bin", "usr/sbin", "sbin"]
        .iter()
        .any(|d| std::fs::symlink_metadata(rootfs.join(d).join("systemctl")).is_ok())
}

/// Wait for `child` within `bound`; `None` at the bound or at a shutdown
/// signal of podvm.
fn wait_bounded(child: &mut Child, bound: Duration) -> Option<i32> {
    let end = Instant::now() + bound;
    loop {
        if let Ok(Some(st)) = child.try_wait() {
            return Some(st.code().unwrap_or(125));
        }
        if Instant::now() >= end || SIGNAL.load(Ordering::Relaxed) != 0 {
            return None;
        }
        std::thread::sleep(POLL);
    }
}

fn console_tail(log: &Path) -> String {
    let text = std::fs::read(log).unwrap_or_default();
    let text = String::from_utf8_lossy(&text);
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(6)..].join("\n")
}

/// Boot the init in a sandbox, run the command of the job, and end the
/// sandbox. The exit code of the command.
pub fn drive(
    job: &Job,
    runsc: &Path,
    state: &Path,
    bundle: &Path,
    id: &str,
) -> Result<i32, String> {
    for sig in [1, 2, 15] {
        // SAFETY: the handler stores one integer.
        unsafe { signal(sig, note as extern "C" fn(i32) as usize) };
    }
    let log = bundle.join("console.log");
    let out = std::fs::File::create(&log).map_err(|e| format!("{}: {e}", log.display()))?;
    let err = out.try_clone().map_err(|e| e.to_string())?;
    let mut run = Command::new(runsc);
    run.args(args(state, bundle, id))
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err);
    // A SIGKILL of podvm ends `runsc run`, and `runsc run` gives its
    // sandbox the same rule (its attached mode): nothing of the run stays.
    // SAFETY: the closure makes one system call.
    unsafe {
        use std::os::unix::process::CommandExt;
        run.pre_exec(|| {
            unsafe extern "C" {
                fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
            }
            // PR_SET_PDEATHSIG, SIGKILL.
            prctl(1, 9, 0, 0, 0);
            Ok(())
        });
    }
    let mut sandbox = run
        .spawn()
        .map_err(|e| format!("{}: {e}", runsc.display()))?;
    let result = boot_then_run(job, runsc, state, id, &mut sandbox, &log);
    stop(runsc, state, id, &mut sandbox);
    match (result, SIGNAL.load(Ordering::Relaxed)) {
        (_, sig) if sig != 0 => Ok(128 + sig),
        (r, _) => r,
    }
}

fn boot_then_run(
    job: &Job,
    runsc: &Path,
    state: &Path,
    id: &str,
    sandbox: &mut Child,
    log: &Path,
) -> Result<i32, String> {
    let ended = |sandbox: &mut Child, log: &Path| {
        sandbox.try_wait().ok().flatten().map(|st| {
            format!(
                "the init of the image ended during its boot ({st}); the last lines of its \
                 console:\n{}",
                console_tail(log)
            )
        })
    };
    let ask = exec_args(
        state,
        id,
        None,
        "/",
        &[format!("PATH={}", crate::initns::join::PATH)],
        &["systemctl".to_string(), "is-system-running".to_string()],
    );
    let end = Instant::now() + BOOT_BOUND;
    let wait = has_systemctl(job.rootfs);
    loop {
        if let Some(why) = ended(sandbox, log) {
            return Err(why);
        }
        if SIGNAL.load(Ordering::Relaxed) != 0 {
            return Ok(125);
        }
        let o = Command::new(runsc)
            .args(&ask)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("runsc exec: {e}"))?;
        let state_word = String::from_utf8_lossy(&o.stdout).trim().to_string();
        let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
        // Before the sandbox runs, `runsc exec` names the state of the
        // container; before PID 1 has its bus, systemctl names the bus.
        let up = if wait {
            step(&state_word, &err) == Step::Run
        } else {
            !err.contains("cannot execute in container") && !err.contains("loading container")
        };
        if up {
            break;
        }
        if Instant::now() >= end {
            return Err(format!(
                "the init of the image did not finish its boot in {} s: systemctl \
                 is-system-running gives {state_word:?}: {err}",
                BOOT_BOUND.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let user = (job.user != (0, 0)).then_some(job.user);
    let mut cmd = Command::new(runsc);
    cmd.args(exec_args(
        state,
        id,
        user,
        &job.workdir,
        &job.env,
        &job.argv,
    ));
    if job.stdin.is_some() {
        cmd.stdin(Stdio::piped());
    }
    let mut child = cmd.spawn().map_err(|e| format!("runsc exec: {e}"))?;
    if let (Some(b), Some(mut w)) = (&job.stdin, child.stdin.take()) {
        use std::io::Write;
        if let Err(e) = w.write_all(b) {
            eprintln!("podvm run: the stdin of the command: {e}");
        }
    }
    loop {
        if let Some(code) = wait_bounded(&mut child, Duration::from_millis(200)) {
            return Ok(code);
        }
        if SIGNAL.load(Ordering::Relaxed) != 0 {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(125);
        }
        if let Some(why) = ended(sandbox, log) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(why);
        }
    }
}

/// End the sandbox: SIGRTMIN+4 to the init from inside (`runsc kill` takes
/// no real-time signal), until PID 1 is no longer the manager (systemd
/// stopped each unit and became `systemd-shutdown`, which does not end a
/// sandbox of the Sentry), within the bound; then SIGKILL of the sandbox.
fn stop(runsc: &Path, state: &Path, id: &str, sandbox: &mut Child) {
    if sandbox.try_wait().ok().flatten().is_none() && SIGNAL.load(Ordering::Relaxed) == 0 {
        let script = "kill -38 1 2>/dev/null; i=0; \
                      while [ \"$(cat /proc/1/comm 2>/dev/null)\" = systemd ] && [ $i -lt 100 ]; \
                      do sleep 0.1; i=$((i+1)); done";
        let a = exec_args(
            state,
            id,
            None,
            "/",
            &[format!("PATH={}", crate::initns::join::PATH)],
            &["sh".to_string(), "-c".to_string(), script.to_string()],
        );
        let asked = Command::new(runsc)
            .args(a)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(mut c) = asked
            && wait_bounded(&mut c, STOP_BOUND).is_none()
        {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
    let _ = Command::new(runsc)
        .args(["--rootless", &format!("--root={}", state.display())])
        .args(["kill", id, "KILL"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if wait_bounded(sandbox, STOP_BOUND).is_none() {
        let _ = sandbox.kill();
        let _ = sandbox.wait();
    }
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
