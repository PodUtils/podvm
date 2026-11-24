//! The incus back end, TODO/machine.md T-1718, in the form of
//! `docs/incus.md`: podvm drives an incusd over its REST API on a unix
//! socket. `--podvm-tier=incus` selects it for `run`.

pub(crate) mod attach; // TODO/machine.md T-2080
pub(crate) mod config;
pub(crate) mod cut; // TODO/machine.md T-2082, T-2085
pub(crate) mod exec; // TODO/machine.md T-2080
pub(crate) mod guest_exec; // TODO/machine.md T-2080
pub(crate) mod gzip; // TODO/machine.md T-2085
pub(crate) mod held; // TODO/machine.md T-2078
pub(crate) mod image;
pub(crate) mod import; // TODO/machine.md T-2081
pub(crate) mod inspect;
pub(crate) mod logs; // TODO/cli.md T-2046
pub(crate) mod net;
pub(crate) mod oom; // TODO/machine.md T-2077
pub(crate) mod run;
pub(crate) mod space; // TODO/machine.md T-2078
pub(crate) mod systemd;
pub(crate) mod usage; // TODO/machine.md T-2076
pub(crate) mod verbs;
mod xattrs; // TODO/machine.md T-1999

#[cfg(test)]
mod attach_tests; // TODO/machine.md T-2080
#[cfg(test)]
mod held_tests; // TODO/machine.md T-2078
#[cfg(test)]
mod import_tests; // TODO/machine.md T-2085
#[cfg(test)]
mod oom_tests; // TODO/machine.md T-2077
#[cfg(test)]
mod run_tests; // TODO/machine.md T-2080
#[cfg(test)]
mod space_tests; // TODO/machine.md T-2078
#[cfg(test)]
mod tests;
#[cfg(test)]
mod usage_tests;
#[cfg(test)]
mod verbs_tests; // TODO/machine.md T-1846 // TODO/machine.md T-2076

/// The verbs that `--podvm-tier=incus` serves on an instance that stays
/// (TODO/machine.md T-1722): their exit, or `None` for another verb.
pub fn intercept(verb: Option<&str>, rest: &[String]) -> Option<i32> {
    // TODO/cli.md T-1811: another tier on these verbs is a usage error by
    // name; it reached the parser, whose "a bug in podvm" text was wrong.
    if let (Some(v @ ("ps" | "start" | "stop" | "rm" | "logs" | "inspect")), Some(t)) =
        (verb, other_tier(rest))
    {
        eprintln!(
            "podvm {v}: --podvm-tier={t}: {v} takes --podvm-tier=incus only, for an \
             instance of incus that stays (TODO/machine.md T-1722); the containers of \
             the other tiers are named with no --podvm-tier (TODO/cli.md T-1811)"
        );
        return Some(podvm_image::error::EXIT_FLAG_ERROR);
    }
    let a = verbs::wants(rest)?;
    match verb? {
        v @ ("ps" | "start" | "stop" | "rm") => Some(verbs::dispatch(v, &a)),
        "logs" => Some(logs::logs(&a)),
        "inspect" => Some(inspect::inspect(&a)),
        _ => None,
    }
}

/// The value of a `--podvm-tier` in `args` that is not `incus`.
pub(crate) fn other_tier(args: &[String]) -> Option<String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let t = match a.strip_prefix("--podvm-tier=") {
            Some(t) => t.to_string(),
            None if a == "--podvm-tier" => it.next().cloned().unwrap_or_default(),
            None => continue,
        };
        if t != "incus" {
            return Some(t);
        }
    }
    None
}

/// The command of an instance that stays (TODO/machine.md T-1722), as
/// `run -d` and `create` push it to [`CMD`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    pub argv: Vec<String>,
    pub env: Vec<String>,
    pub cwd: String,
}

impl Cmd {
    /// The JSON of the file.
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::json!({"argv": self.argv, "env": self.env, "cwd": self.cwd})
            .to_string()
            .into_bytes()
    }

    /// The command of the JSON of the file.
    pub fn parse(b: &[u8]) -> Result<Cmd, String> {
        let v: serde_json::Value = serde_json::from_slice(b).map_err(|e| e.to_string())?;
        let list = |k: &str| -> Vec<String> {
            v.get(k)
                .and_then(serde_json::Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        Ok(Cmd {
            argv: list("argv"),
            env: list("env"),
            cwd: v
                .get("cwd")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("/")
                .to_string(),
        })
    }
}

/// The files of the command in the instance.
pub const CMD: &str = "/.podvm/cmd";
pub const LOG: &str = "/.podvm/log";
pub const EXIT: &str = "/.podvm/exit";
/// The stdin of the command, mode 0600, where the run had one (T-1777).
pub const STDIN: &str = "/.podvm/stdin";

/// The exit code of a wait status.
fn code_of(status: i32) -> i32 {
    if status & 0x7f == 0 {
        (status >> 8) & 0xff
    } else {
        128 + (status & 0x7f)
    }
}

/// `podvm guest-hold [--run]`: the init of a podvm instance. As PID 1 it
/// reaps each orphan, and it ends on SIGTERM, SIGINT or SIGPWR (incus stops
/// an instance with SIGPWR); a PID 1 with no handler would ignore each.
///
/// With `--run` (TODO/machine.md T-1722) it first starts the command of
/// [`CMD`] with stdout and stderr appended to [`LOG`], passes a stop signal
/// to it as SIGTERM, and ends with the command's exit code, which it writes
/// to [`EXIT`]: the instance stops when its command ends, as a docker
/// container does. As PID 1 (TODO/machine.md T-2076) it first ends each
/// other process of the instance and writes the usage of the instance's
/// cgroup to [`usage::USAGE`], which `inspect` reads after the stop.
pub fn hold(args: &[String]) -> i32 {
    unsafe extern "C" {
        fn signal(sig: i32, handler: usize) -> usize;
        fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
        fn pause() -> i32;
        fn kill(pid: i32, sig: i32) -> i32;
        fn _exit(code: i32) -> !;
    }
    static CHILD: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
    extern "C" fn end(_: i32) {
        // T-2076: the end of the instance runs; a stop signal changes nothing.
        if ENDING.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let child = CHILD.load(std::sync::atomic::Ordering::Relaxed);
        if child > 0 {
            // SAFETY: kill is async-signal-safe; the loop reaps the child.
            unsafe { kill(child, 15) };
            return;
        }
        // SAFETY: _exit is async-signal-safe.
        unsafe { _exit(0) }
    }
    // TODO/machine.md T-1993: `--detach` (with `--podvm-init=image`): the
    // exec that started this process returns at once, and a child in a
    // session of its own holds the command; systemd is PID 1.
    if args.iter().any(|a| a == "--detach") {
        unsafe extern "C" {
            fn fork() -> i32;
            fn setsid() -> i32;
        }
        // SAFETY: one thread so far; the child goes on alone.
        match unsafe { fork() } {
            0 => {
                // SAFETY: a plain system call in the child.
                unsafe { setsid() };
                if let Ok(null) = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open("/dev/null")
                {
                    use std::os::fd::AsRawFd;
                    unsafe extern "C" {
                        fn dup2(old: i32, new: i32) -> i32;
                    }
                    for fd in 0..3 {
                        // SAFETY: the standard descriptors of this child.
                        unsafe { dup2(null.as_raw_fd(), fd) };
                    }
                }
            }
            pid if pid > 0 => return 0,
            _ => return 127,
        }
    }
    // TODO/machine.md T-1779: an init brings `lo` up. An instance with no
    // NIC (`--podvm-net none`) has nothing else that does. The channel
    // listens on 127.0.0.1 before the command starts.
    let _ = crate::machine::guest::net::lo_up();
    net::listen();
    // TODO/machine.md T-2076: as PID 1 with a command, the files of the
    // cgroup and the root are taken before the command can change them,
    // and the exit and the usage of a run before go.
    let pid1 = std::process::id() == 1;
    let mut end_of = None;
    let mut guard = None;
    if args.iter().any(|a| a == "--run") {
        if pid1 {
            let _ = std::fs::remove_file(EXIT);
            let _ = std::fs::remove_file(usage::USAGE);
            end_of = Some((usage::Anchor::take(), usage::Meter::open()));
            // TODO/machine.md T-2077: the OOM killer of the instance takes
            // each process of the command before PID 1, which writes the
            // files of the end.
            let (g, why) = oom::Guard::take(std::path::Path::new(oom::SELF));
            if let Some(why) = why {
                // The console of the instance, not the log of the command.
                eprintln!(
                    "podvm guest-hold: {why}; the command gets oom_score_adj {} \
                     (TODO/machine.md T-2077)",
                    g.child()
                );
            }
            guard = Some(g);
        }
        match start_cmd(guard) {
            Ok(pid) => CHILD.store(pid, std::sync::atomic::Ordering::Relaxed),
            Err(e) => {
                let _ = std::fs::write(LOG, format!("podvm guest-hold: {e}\n"));
                match end_of.as_mut() {
                    Some((anchor, meter)) => finish(127, anchor.as_ref(), meter),
                    None => {
                        let _ = std::fs::write(EXIT, "127\n");
                    }
                }
                return 127;
            }
        }
    }
    // SAFETY: plain system calls; the handler only signals or exits.
    unsafe {
        for sig in [2, 15, 30] {
            signal(sig, end as extern "C" fn(i32) as usize);
        }
        loop {
            let mut status = 0;
            let pid = waitpid(-1, &mut status, 0);
            if pid < 0 {
                pause();
                continue;
            }
            if pid == CHILD.load(std::sync::atomic::Ordering::Relaxed) {
                let code = code_of(status);
                if let Some((anchor, meter)) = end_of.as_mut() {
                    finish(code, anchor.as_ref(), meter);
                    return code;
                }
                let _ = std::fs::write(EXIT, format!("{code}\n"));
                // TODO/machine.md T-1993: where the init of the image is
                // PID 1, the instance stops when its command ends, as with
                // podvm's own init: SIGRTMIN+4 asks systemd to power off.
                if std::process::id() != 1 {
                    kill(1, 34 + 4);
                }
                return code;
            }
        }
    }
}

/// Set at the end of an instance: the handler of a stop signal does nothing.
static ENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// TODO/machine.md T-2076: the end of an instance whose PID 1 is the hold,
/// after its command ended with `code`. Each other process of the instance
/// ends first, so that none of them can write after the hold: then the
/// exit code goes to [`EXIT`] and the usage to [`usage::USAGE`].
fn finish(code: i32, anchor: Option<&usage::Anchor>, meter: &mut usage::Meter) {
    ENDING.store(true, std::sync::atomic::Ordering::Relaxed);
    let ended = usage::end_tree(usage::END_WAIT);
    let mut m = meter.read();
    if !ended {
        m.why.push(format!(
            "a process of the instance was still there {} s after SIGKILL",
            usage::END_WAIT.as_secs()
        ));
    }
    let root = match anchor {
        Some(a) => a.restore(),
        None => Err("the init could not open its root at the start".to_string()),
    };
    if usage::seal(EXIT, format!("{code}\n").as_bytes()).is_err() {
        let _ = std::fs::write(EXIT, format!("{code}\n"));
    }
    match root {
        // The root of the instance: the measure is the one that a reader
        // finds after the stop.
        Ok(()) => {
            let _ = usage::seal(usage::USAGE, &m.to_json());
        }
        // The root may be one that the command made: no usage file, and a
        // reader gives no value.
        Err(_) => {
            let _ = std::fs::remove_file(usage::USAGE);
        }
    }
}

/// Start the command of [`CMD`]; its pid. With `guard`, the command gets
/// the `oom_score_adj` of [`oom::Guard::child`] before its `exec`.
fn start_cmd(guard: Option<oom::Guard>) -> Result<i32, String> {
    let text = std::fs::read(CMD).map_err(|e| format!("{CMD}: {e}"))?;
    let cmd = Cmd::parse(&text).map_err(|e| format!("{CMD}: {e}"))?;
    let (prog, rest) = cmd.argv.split_first().ok_or("the command is empty")?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG)
        .map_err(|e| format!("{LOG}: {e}"))?;
    let err = log.try_clone().map_err(|e| e.to_string())?;
    // TODO/machine.md T-1777: the stdin that `run -d -i` pushed, if any. The
    // file goes once the command holds it: the credentials of a caller do
    // not stay in the instance.
    let stdin = match std::fs::File::open(STDIN) {
        Ok(f) => std::process::Stdio::from(f),
        Err(_) => std::process::Stdio::null(),
    };
    let mut command = std::process::Command::new(prog);
    if let Some(g) = guard {
        let (text, must) = (g.child_text(), g.must_set());
        // SAFETY: the closure makes three system calls and allocates
        // nothing (`oom::set_in_child`).
        unsafe {
            use std::os::unix::process::CommandExt;
            command.pre_exec(move || oom::set_in_child(&text, must));
        }
    }
    let child = command
        .args(rest)
        .env_clear()
        .envs(cmd.env.iter().filter_map(|e| e.split_once('=')))
        .current_dir(if cmd.cwd.is_empty() { "/" } else { &cmd.cwd })
        .stdin(stdin)
        .stdout(log)
        .stderr(err)
        .spawn()
        .map_err(|e| match guard {
            Some(g) if g.must_set() => format!(
                "{prog}: {e} (before its exec the command sets its oom_score_adj to {})",
                g.child()
            ),
            _ => format!("{prog}: {e}"),
        })?;
    let _ = std::fs::remove_file(STDIN);
    Ok(child.id() as i32)
}
