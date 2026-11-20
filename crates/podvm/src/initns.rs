//! TODO/enter.md T-2002: `run --podvm-init=image` on a rung of the host and
//! in a guest of the machine tier. The init of the image (systemd) is PID 1
//! of a PID namespace, the command of the run joins it after the boot, and
//! the end of the command ends the init.
//!
//! Two callers of one supervisor ([`boot::supervise`], the verb
//! `guest-init`):
//!
//! * the namespace rung, where a foreground `run` of a caller with no root
//!   is in podvm's user namespace ([`crate::userns`]). It needs two things
//!   of the host ([`decide`]): the subordinate id range of the caller
//!   (`newuidmap`, `newgidmap`, `/etc/subuid`, `/etc/subgid`), so that a
//!   unit with `User=` gets its uid from the kernel, and a delegated
//!   cgroup2 subtree, without which systemd ends at "Failed to create
//!   /init.scope control group";
//! * a guest of the machine tier, where podvm is root with a kernel of its
//!   own: each host that runs QEMU (KVM, else TCG) has this route, also a
//!   host that denies each namespace (the userland rung). [`route`] takes
//!   it where the host lacks a part of the first.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

pub mod boot;
pub mod cgroup;
pub mod hold;
pub mod join;
pub mod mounts;
pub mod sandbox;
pub mod scope;

/// The init of an image, as incus and each container manager start it.
pub const INIT: &str = "/sbin/init";
/// The mark of the run in the environment of the command.
pub const MARK: &str = "PODVM_INIT=image";
/// The verb of the supervisor.
pub const VERB: &str = "guest-init";
/// The variable that sends the console of the init to stderr (`--verbose`).
pub const LOG_VAR: &str = "PODVM_INIT_LOG";
/// The variable of a run whose tree and volumes keep nothing (`--rm`, and
/// each volume read-only): the init ends with SIGKILL (T-2010).
pub const DISCARD_VAR: &str = "PODVM_INIT_DISCARD";

/// True where [`decide`] found each part of the namespace rung.
static EARLY: AtomicBool = AtomicBool::new(false);
/// The delegated directory of [`decide`].
static DELEGATED: OnceLock<PathBuf> = OnceLock::new();
/// Why [`decide`] left the namespace rung.
static WHY: OnceLock<String> = OnceLock::new();

/// True for the arguments of `run --podvm-init=image` (either form).
pub fn asked(argv: &[String]) -> bool {
    argv.get(1).map(String::as_str) == Some("run")
        && argv[2..].iter().enumerate().any(|(i, a)| {
            a == "--podvm-init=image"
                || (a == "--podvm-init" && argv.get(i + 3).map(String::as_str) == Some("image"))
        })
}

/// What the host gives the namespace rung for an init of the image.
pub struct Host {
    /// A foreground `run` of a caller that is not uid 0, with the user
    /// namespace not turned off ([`crate::userns::wanted`]).
    pub userns: bool,
    /// The subordinate id range and its two tools.
    pub range: bool,
    /// The delegated cgroup2 directory, or the reason for none.
    pub cgroup: Result<PathBuf, String>,
}

/// The reason for which the namespace rung cannot boot the init, or none.
pub fn refusal(h: &Host) -> Option<String> {
    if !h.userns {
        return Some(
            "the namespace rung boots an init of the image for a foreground run of a \
             caller that is not uid 0, with PODVM_USERNS not 0"
                .to_string(),
        );
    }
    if !h.range {
        return Some(
            "the host gives the caller no subordinate id range (newuidmap, newgidmap, and a \
             range in /etc/subuid and /etc/subgid), so a unit with User= cannot get its uid"
                .to_string(),
        );
    }
    h.cgroup.as_ref().err().map(|why| {
        format!(
            "the host delegates no cgroup2 subtree to the caller ({why}), and systemd as \
             PID 1 needs one"
        )
    })
}

/// In `main`, before the user namespace: decide where the init of the
/// image can boot. True where the caller asks and the namespace rung
/// cannot hold it: podvm then enters no user namespace, and [`route`]
/// takes the machine tier.
pub fn decide(argv: &[String], uid: u32) -> bool {
    if !asked(argv) || argv[2..].iter().any(|a| a.starts_with("--podvm-tier")) {
        return false;
    }
    let var = std::env::var(crate::userns::VAR).ok();
    let userns = crate::userns::wanted(argv, uid, var.as_deref());
    let range = crate::userns::range_available();
    let mut cgroup = cgroup::delegated();
    // TODO/enter.md T-2004: where the rest holds, the user manager of
    // systemd can give the missing subtree.
    if let (true, true, Err(first)) = (userns, range, &cgroup) {
        cgroup = match scope::ask() {
            Ok(()) => cgroup::delegated(),
            Err(why) => Err(format!("{first}; {why}")),
        };
    }
    let host = Host {
        userns,
        range,
        cgroup,
    };
    match refusal(&host) {
        Some(why) => {
            let _ = WHY.set(why);
            true
        }
        None => {
            if let Ok(dir) = host.cgroup {
                let _ = DELEGATED.set(dir);
            }
            EARLY.store(true, Ordering::Relaxed);
            false
        }
    }
}

/// True where [`decide`] chose the namespace rung: the user namespace of
/// the run then takes the subordinate range with no variable.
pub fn early() -> bool {
    EARLY.load(Ordering::Relaxed)
}

/// True where this process can boot the init on the namespace rung: the
/// decision, the user namespace entered, and the range in its maps.
pub fn native() -> bool {
    early() && crate::userns::active() && crate::userns::owners::holds(1, 1)
}

/// The vCPUs of a guest that boots an init of the image, where the caller
/// names none: the units of a boot start in parallel, and under TCG one
/// vCPU boots systemd of Arch Linux in 37 s and four in 12 s (experiment
/// 465). At most four, and at most the CPUs of the host.
pub fn guest_cpus(host: usize) -> u32 {
    host.clamp(1, 4) as u32
}

/// After the parse: a `run --podvm-init=image` with no tier that the
/// namespace rung cannot hold takes the machine tier, with one line that
/// says why.
pub fn route(verb: &str, mut o: crate::run::Opts) -> crate::run::Opts {
    if verb != "run" || !o.init_image || o.detach {
        return o;
    }
    if o.tier.is_none() && !native() {
        let why = WHY.get().cloned().unwrap_or_else(|| {
            "the host refuses the user namespace of the run or its id range".to_string()
        });
        // TODO/enter.md T-2007: gVisor before a guest. Its kernel gives the
        // init a cgroup2 tree and each uid with no help of the host, and
        // the code of the image runs on the CPU of the host.
        let sandbox = match crate::run::runsc_unserved(verb, &o) {
            Some(what) => Err(format!("the runsc tier {what}")),
            None => crate::runsc::binary()
                .and_then(|_| crate::runsc::user_namespaces())
                .map_err(|e| format!("no runsc tier: {e}")),
        };
        match sandbox {
            Ok(()) => {
                eprintln!(
                    "podvm run: --podvm-init=image: {why}; the run takes the runsc tier, where \
                     the init of the image is PID 1 of a gVisor sandbox (TODO/enter.md T-2007)"
                );
                o.tier = Some(crate::tier::Want::Runsc);
            }
            Err(no) => {
                eprintln!(
                    "podvm run: --podvm-init=image: {why}; {no}; the run takes the machine \
                     tier, where the init of the image is PID 1 of a PID namespace of the \
                     guest (TODO/enter.md T-2002)"
                );
                o.tier = Some(crate::tier::Want::Machine);
            }
        }
    }
    if o.tier == Some(crate::tier::Want::Machine) && o.cpus.is_none() {
        let host = std::thread::available_parallelism().map_or(1, |n| n.get());
        o.cpus = Some(guest_cpus(host));
    }
    o
}

/// The mark of an init of the image in the environment of a run, and its
/// console on stderr under `--verbose`. A value of the caller is removed
/// first: only the flag sets the mark.
pub fn mark_env(env: &mut Vec<String>, init_image: bool, verbose: bool, discard: bool) {
    env.retain(|e| !e.starts_with("PODVM_INIT"));
    if init_image {
        env.push(MARK.to_string());
        if verbose {
            env.push(format!("{LOG_VAR}=1"));
        }
        if discard {
            env.push(format!("{DISCARD_VAR}=1"));
        }
    }
}

/// True where nothing of a run can stay: `--rm`, and each volume
/// read-only.
pub fn discards(rm: bool, volumes: &[podvm_enter::userland::Volume], ro: &[String]) -> bool {
    rm && volumes.iter().all(|v| ro.contains(&v.guest))
}

/// True where the prepared foreground run boots the init here.
pub fn takes(env: &[String]) -> bool {
    native() && env.iter().any(|e| e == MARK)
}

/// The arguments of the supervisor for a run on the namespace rung.
pub fn host_args(
    rootfs: &str,
    base: &Path,
    cwd: &str,
    ids: Option<(u32, u32)>,
    binds: &[podvm_enter::VolumeBind],
    argv: &[String],
) -> Vec<String> {
    let mut a = vec![
        VERB.to_string(),
        "--root".into(),
        rootfs.to_string(),
        "--cgroup".into(),
        base.to_string_lossy().into_owned(),
        "--cwd".into(),
        cwd.to_string(),
        "--tmp".into(),
    ];
    if let Some((u, g)) = ids.filter(|i| *i != (0, 0)) {
        a.extend(["--ids".to_string(), format!("{u}:{g}")]);
    }
    for b in binds {
        let ro = if b.read_only { ":ro" } else { "" };
        a.extend(["--bind".to_string(), format!("{}:{}{ro}", b.guest, b.host)]);
    }
    a.push("--".into());
    a.extend(argv.iter().cloned());
    a
}

/// Read the arguments of `guest-init`; an error is the word that is wrong.
pub fn parse(args: &[String]) -> Result<boot::Spec, String> {
    let mut s = boot::Spec {
        root: PathBuf::from("/"),
        cwd: "/".to_string(),
        ..Default::default()
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{a} takes a value"));
        match a.as_str() {
            "--root" => s.root = PathBuf::from(value()?),
            "--cwd" => s.cwd = value()?,
            "--cgroup" => {
                let v = value()?;
                s.cgroup = match v.as_str() {
                    "auto" => cgroup::own_base(),
                    _ => Some(PathBuf::from(v)),
                };
            }
            "--ids" => {
                let v = value()?;
                let ids = v
                    .split_once(':')
                    .and_then(|(u, g)| Some((u.parse().ok()?, g.parse().ok()?)));
                s.ids = Some(ids.ok_or(format!("--ids takes UID:GID, not {v:?}"))?);
            }
            "--tmp" => {
                s.tmp.get_or_insert_with(Vec::new);
            }
            "--bind" => {
                let v = value()?;
                let b = mounts::Bind::parse(&v)
                    .ok_or(format!("--bind takes /tmp/GUEST:/HOST[:ro], not {v:?}"))?;
                s.tmp.get_or_insert_with(Vec::new).push(b);
            }
            "--hold" => s.hold = Some(PathBuf::from(value()?)),
            "--attach" => s.attach = Some(PathBuf::from(value()?)),
            "--verbose" => s.verbose = true,
            "--discard" => s.discard = true,
            "--" => {
                s.argv = it.cloned().collect();
                break;
            }
            other => return Err(format!("{other:?} is no argument of {VERB}")),
        }
    }
    if s.hold.is_some() && (s.attach.is_some() || !s.argv.is_empty()) {
        return Err("--hold takes no command and no --attach".to_string());
    }
    if s.argv.is_empty() && s.hold.is_none() {
        return Err("no command after --".to_string());
    }
    Ok(s)
}

/// `podvm guest-init ...`: internal, in the user namespace of a run or in
/// a guest.
pub fn main(args: &[String]) -> i32 {
    if args.first().map(String::as_str) == Some(sandbox::FLAG) {
        return sandbox::main(&args[1..]);
    }
    match parse(args) {
        Ok(mut spec) => {
            spec.verbose |= std::env::var(LOG_VAR).is_ok_and(|v| v == "1");
            spec.discard |= std::env::var(DISCARD_VAR).is_ok_and(|v| v == "1");
            match (spec.hold.clone(), spec.attach.clone()) {
                (Some(file), _) => hold::hold(&spec, &file),
                (None, Some(file)) => hold::attach(&spec, &file),
                (None, None) => boot::supervise(&spec),
            }
        }
        Err(e) => {
            eprintln!("podvm {VERB}: {e}");
            125
        }
    }
}

/// On the namespace rung: start the supervisor for the prepared run. It
/// gives the pid of the supervisor and the base of its cgroups, which
/// [`end`] removes after the wait.
pub fn spawn(rootfs: &str, plan: &podvm_enter::Plan) -> Result<(i64, PathBuf), String> {
    use std::os::unix::process::CommandExt;
    let delegated = DELEGATED
        .get()
        .ok_or("no delegated cgroup2 directory was found")?;
    // A random name: the pid of this process is 2 in each run (the PID
    // namespace of the run), and two runs share the delegated directory.
    let nonce = crate::machine::guest::init::nonce()?;
    cgroup::sweep(delegated);
    let base = delegated.join(format!("{}{nonce}", cgroup::BASE_PREFIX));
    let ids = plan
        .env
        .iter()
        .find_map(|e| e.strip_prefix("PODVM_IDENTITY="))
        .and_then(|v| v.split_once(':'))
        .and_then(|(u, g)| Some((u.parse().ok()?, g.parse().ok()?)));
    let me = std::env::current_exe().map_err(|e| format!("the path of podvm: {e}"))?;
    let args = host_args(
        rootfs,
        &base,
        &plan.working_dir,
        ids,
        &plan.volumes,
        &plan.argv,
    );
    let mut c = std::process::Command::new(me);
    c.args(args).env_clear();
    for e in &plan.env {
        if let Some((k, v)) = e.split_once('=') {
            c.env(k, v);
        }
    }
    let pass = plan.fds.pass.clone();
    // SAFETY: the closure makes system calls only.
    unsafe {
        c.pre_exec(move || {
            unsafe extern "C" {
                fn dup2(old: i32, new: i32) -> i32;
                fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
            }
            // PR_SET_PDEATHSIG, SIGKILL: the supervisor ends with podvm.
            prctl(1, 9, 0, 0, 0);
            for (child, host) in &pass {
                if (0..3).contains(child) && dup2(*host as i32, *child as i32) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let child = c
        .spawn()
        .map_err(|e| format!("the start of the supervisor: {e}"))?;
    let pid = i64::from(child.id());
    // The wait of pod-enter reaps the supervisor; the handle of std must not.
    std::mem::forget(child);
    Ok((pid, base))
}

/// After the wait: remove the cgroups of the run.
pub fn end(base: &Path) {
    cgroup::remove(base);
}

#[cfg(test)]
mod tests;
