//! TODO/machine.md T-2008: one run from a saved state of a guest, or with
//! the save of one.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use podvm_windows::plan::Accel;

use super::super::job::Job;
use super::super::{arch, init, vol, Request};
use super::save::Saved;
use super::store::{files, mark_bad, plan, touch, Files, Plan};
use super::{fingerprint, refusal, wanted, Asks, Shape, Transport, SLOTS, VAR};

/// The job of a run for a guest of a state.
pub fn job(r: &Request, nonce: &str, vols: &[vol::Vol]) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut seed = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut seed))
        .map_err(|e| format!("/dev/urandom: {e}"))?;
    let time_ns = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("the clock of the host: {e}"))?
        .as_nanos();
    Job {
        script: init::job(nonce, &r.argv, &r.env, &r.workdir, vols),
        stdin: r.stdin.clone(),
        seed,
        time_ns: u64::try_from(time_ns).map_err(|_| "the clock of the host".to_string())?,
    }
    .frame()
}

/// What the drive of the emulator does for a guest of a state.
pub enum Mode<'a> {
    /// The guest booted now: save its state at the marker of the held
    /// guest, then give the job.
    Save {
        qmp: PathBuf,
        files: &'a Files,
        fingerprint: &'a str,
        mem_mib: u64,
        transport: Transport,
        unique: &'a str,
    },
    /// The guest comes from the state: give the job when the load is
    /// complete and the guest runs.
    Load { qmp: PathBuf },
}

pub struct Warm<'a> {
    pub mode: Mode<'a>,
    /// The nonce of the markers before the job ([`Files::nonce`]).
    pub nonce: &'a str,
    /// The frame of the job, taken when it is sent.
    pub frame: Option<Vec<u8>>,
    /// The save, where the drive made one.
    pub saved: Option<Result<Saved, String>>,
}

/// How the drive of the emulator ended.
pub enum Driven {
    /// The run ended with this exit code.
    Code(i32),
    /// The guest of a state did not reach the command, for this reason:
    /// the caller boots a guest with no state.
    Cold(String),
}

/// What [`run`] gives the boot of a guest.
pub enum Outcome {
    /// The run ended with this exit code.
    Done(i32),
    /// Boot as a run with no state.
    Boot,
}

/// What the boot of a guest has when it asks for a state.
pub struct Ctx<'a> {
    pub accel: Accel,
    pub qemu: &'a Path,
    pub kernel: &'a Path,
    /// The base archive of the image.
    pub base: &'a Path,
    /// The modules of the network and of the volumes, for the guest.
    pub modules: &'a [(String, PathBuf)],
    /// True where the kernel is the pin, with podvm and the 9p modules.
    pub pinned_kernel: bool,
}

/// The paths of one emulator line.
pub(super) struct Paths<'a> {
    initrd: &'a Path,
    err: &'a Path,
    qmp: Option<&'a Path>,
    /// The directory of each slot, and whether it is read-only.
    slots: Vec<(PathBuf, bool)>,
    incoming: Option<String>,
}

/// The emulator line of a guest of a state: the line of each guest, with
/// [`SLOTS`] 9p devices.
pub(super) fn line(r: &Request, accel: Accel, mem: u64, kernel: &Path, p: &Paths) -> Vec<String> {
    let mut extra = super::super::stderr::argv(p.err);
    for (i, (dir, ro)) in p.slots.iter().enumerate() {
        extra.push("-virtfs".to_string());
        extra.push(vol::virtfs(
            dir,
            &vol::Vol {
                tag: format!("podvm{i}"),
                guest: String::new(),
                ro: *ro,
            },
        ));
    }
    extra.extend(r.qemu_args.iter().cloned());
    if let Some(q) = p.qmp {
        extra.push("-qmp".to_string());
        extra.push(format!("unix:{},server=on,wait=off", q.display()));
    }
    if let Some(uri) = &p.incoming {
        extra.push("-incoming".to_string());
        extra.push(uri.clone());
    }
    let mut argv = super::super::qemu_argv(accel, mem, kernel, p.initrd, &extra);
    arch::adapt(&mut argv, r.arch);
    arch::err_port(&mut argv, r.arch);
    if r.net_none {
        super::super::no_nic(&mut argv);
    }
    super::super::with_cpus(&mut argv, r.cpus);
    argv
}

/// Try the run from a saved state, or with the save of one.
pub fn run(r: &Request, c: &Ctx) -> Outcome {
    let say = |what: &str| {
        if r.loud {
            eprintln!(
                "podvm {}: machine tier: {what} (TODO/machine.md T-2008)",
                r.verb
            );
        }
    };
    let init_image = r.env.iter().any(|e| e == crate::initns::MARK);
    if !init_image {
        return Outcome::Boot;
    }
    let first = match wanted(std::env::var(VAR).ok().as_deref()) {
        Ok(Some(t)) => t,
        Ok(None) => return Outcome::Boot,
        Err(e) => {
            eprintln!("podvm {}: machine tier: {e}", r.verb);
            return Outcome::Done(podvm_image::error::EXIT_FLAG_ERROR);
        }
    };
    if let Some(why) = refusal(&Asks::of(r, init_image, c.pinned_kernel)) {
        say(&format!("the guest boots: {why}"));
        return Outcome::Boot;
    }
    // The fingerprint: the script and the line with fixed names.
    let fixed = Paths {
        initrd: Path::new("INITRD"),
        err: Path::new("ERR"),
        qmp: None,
        slots: (0..SLOTS)
            .map(|i| (PathBuf::from(format!("SLOT{i}")), true))
            .collect(),
        incoming: None,
    };
    let shape = Shape {
        digest: r.digest,
        accel: c.accel,
        qemu: c.qemu,
        kernel: c.kernel,
        mem: r.mem,
        line: &line(r, c.accel, 0, Path::new("KERNEL"), &fixed),
        script: &init::template("NONCE", !r.net_none),
    };
    let print = match fingerprint(&shape) {
        Ok(p) => p,
        Err(e) => {
            say(&format!("the guest boots: no fingerprint of a state: {e}"));
            return Outcome::Boot;
        }
    };
    let files = files(r.store_root, r.digest, &print);
    let nonce = match init::nonce() {
        Ok(n) => n,
        Err(e) => {
            say(&format!("the guest boots: {e}"));
            return Outcome::Boot;
        }
    };
    let vols: Vec<vol::Vol> = vol::vols(
        &r.volumes
            .iter()
            .map(|(_, g, ro)| (g.clone(), *ro))
            .collect::<Vec<_>>(),
    );
    let frame = match job(r, &nonce, &vols) {
        Ok(f) => f,
        Err(e) => {
            say(&format!("the guest boots: {e}"));
            return Outcome::Boot;
        }
    };
    // The memory of the guest of a state: the base archive, and the run
    // archive with podvm and the modules.
    let base_bytes = std::fs::metadata(c.base).map(|m| m.len()).unwrap_or(0);
    let most = r
        .mem
        .unwrap_or_else(|| super::super::mem::estimate(base_bytes + (64 << 20)));
    let plan = plan(&files, &print, most);
    if let Plan::Boot(why) = &plan {
        say(&format!("the guest boots: {why}"));
        return Outcome::Boot;
    }
    super::super::rundir::sweep(&r.store_root.join("machine"));
    let dir = r.store_root.join("machine").join(format!(
        "run-{}-{}",
        std::process::id(),
        nonce.chars().take(8).collect::<String>()
    ));
    let empty = dir.join("slot");
    if let Err(e) = std::fs::create_dir_all(&empty) {
        say(&format!("the guest boots: {}: {e}", empty.display()));
        return Outcome::Boot;
    }
    let sock = super::super::stderr::socket_path(&nonce);
    let qmp = sock.with_extension("qmp");
    let _ = std::fs::remove_file(&sock);
    let _ = std::fs::remove_file(&qmp);
    let initrd = dir.join("initrd.cpio");
    let mut slots: Vec<(PathBuf, bool)> = r
        .volumes
        .iter()
        .map(|(host, _, ro)| (host.clone(), *ro))
        .collect();
    slots.resize(SLOTS, (empty.clone(), true));
    let started = Instant::now();
    let (argv, mut warm) = match &plan {
        Plan::Load(meta) => {
            // The guest of the state has its root in its memory: the
            // emulator needs a file for `-initrd`, and no byte of it.
            if let Err(e) = std::fs::write(&initrd, [0u8; 512]) {
                let _ = std::fs::remove_dir_all(&dir);
                say(&format!("the guest boots: {}: {e}", initrd.display()));
                return Outcome::Boot;
            }
            let paths = Paths {
                initrd: &initrd,
                err: &sock,
                qmp: Some(&qmp),
                slots,
                incoming: Some(meta.transport.load_uri(&files.state.to_string_lossy())),
            };
            let argv = line(r, c.accel, meta.mem_mib << 20, c.kernel, &paths);
            let warm = Warm {
                mode: Mode::Load { qmp: qmp.clone() },
                nonce: &files.nonce,
                frame: Some(frame),
                saved: None,
            };
            (argv, warm)
        }
        Plan::Save { .. } => {
            let script = init::template(&files.nonce, !r.net_none);
            let made = super::super::archive::tools(c.modules, Vec::new())
                .and_then(|tools| {
                    std::fs::copy(c.base, &initrd)
                        .map_err(|e| format!("{}: {e}", initrd.display()))?;
                    super::super::run_archive(&script, tools.as_ref(), &[], None)
                        .map_err(|e| format!("the archive of the guest: {e}"))
                })
                .and_then(|extra| {
                    use std::io::Write;
                    std::fs::OpenOptions::new()
                        .append(true)
                        .open(&initrd)
                        .and_then(|mut f| f.write_all(&extra))
                        .map_err(|e| format!("{}: {e}", initrd.display()))
                });
            if let Err(e) = made {
                let _ = std::fs::remove_dir_all(&dir);
                say(&format!("the guest boots: {e}"));
                return Outcome::Boot;
            }
            let mem = super::super::mem::size(r.verb, r.mem, &initrd, true);
            let paths = Paths {
                initrd: &initrd,
                err: &sock,
                qmp: Some(&qmp),
                slots,
                incoming: None,
            };
            let argv = line(r, c.accel, mem, c.kernel, &paths);
            let warm = Warm {
                mode: Mode::Save {
                    qmp: qmp.clone(),
                    files: &files,
                    fingerprint: &print,
                    mem_mib: mem.div_ceil(1 << 20),
                    transport: first,
                    unique: &nonce,
                },
                nonce: &files.nonce,
                frame: Some(frame),
                saved: None,
            };
            (argv, warm)
        }
        Plan::Boot(_) => unreachable!("the plan to boot returned before"),
    };
    say(match &plan {
        Plan::Load(_) => {
            "the guest starts from the saved state of this image and guest shape; its clock \
             and its random pool are the ones of this run, and its machine-id, its boot id and \
             its service processes are the ones of the save"
        }
        _ => "the guest boots, and podvm saves its state for the next run of this image",
    });
    let driven = super::super::drive::drive(r, c.qemu, &argv, &nonce, &dir, &sock, Some(&mut warm));
    let _ = std::fs::remove_file(&sock);
    let _ = std::fs::remove_file(&qmp);
    let _ = std::fs::remove_dir_all(&dir);
    let loaded = matches!(plan, Plan::Load(_));
    match (&warm.saved, loaded) {
        (Some(Ok(s)), _) => say(&format!(
            "the state of the guest is saved: {} MiB in {:.1} s, {}",
            s.bytes.div_ceil(1 << 20),
            s.took.as_secs_f64(),
            files.state.display()
        )),
        // A save that failed is a fault of the store or of the emulator:
        // one line, also with no --verbose, and one count in the marker.
        (Some(Err(e)), _) => {
            eprintln!(
                "podvm {}: machine tier: the state of the guest is not saved, and the next run \
                 boots: {e} (TODO/machine.md T-2008)",
                r.verb
            );
            mark_bad(&files, e);
        }
        (None, true) => touch(&files),
        (None, false) => {}
    }
    match driven {
        Driven::Code(code) => Outcome::Done(code),
        Driven::Cold(why) => {
            eprintln!(
                "podvm {}: machine tier: the guest that podvm {} did not reach the command \
                 after {:.1} s ({why}); podvm boots a guest with no state (TODO/machine.md T-2008)",
                r.verb,
                if loaded {
                    "started from a saved state"
                } else {
                    "booted to save its state"
                },
                started.elapsed().as_secs_f64()
            );
            mark_bad(&files, &why);
            Outcome::Boot
        }
    }
}

/// Send the frame of a job to the serial line of the guest (the stdin of
/// the emulator), from a thread: a frame can be larger than a pipe holds.
/// The thread gives the pipe back, so that it stays open until the end.
///
/// `after_load` is the monitor of an emulator that loads a state: the
/// frame goes when the guest runs. Bytes that come before the end of the
/// load go into the serial port of the emulator and are lost with it. A
/// monitor that does not answer does not hold the job: the frame goes
/// then, and its lead of newlines takes the loss.
pub fn send_job(
    stdin: Option<std::process::ChildStdin>,
    frame: Option<Vec<u8>>,
    after_load: Option<std::path::PathBuf>,
) -> Option<std::thread::JoinHandle<std::process::ChildStdin>> {
    let (mut stdin, frame) = (stdin?, frame?);
    Some(std::thread::spawn(move || {
        // An emulator that ended (a state that does not load) closed the
        // pipe: the write gives EPIPE to this thread, and the default
        // action of SIGPIPE must not end podvm before its boot of a guest
        // (measured: exit 141). Blocked in this thread only, as the tee of
        // stdout does (PodUtils/podvm#534).
        let mut old = 0u64;
        let _ = podvm_probe::sys::sigblock(1 << (13 - 1), &mut old);
        if let Some(qmp) = after_load {
            let _ = super::save::running(&qmp);
        }
        // A failed write is an emulator that ended: the console says so.
        stdin
            .write_all(&frame)
            .and_then(|()| stdin.flush())
            .unwrap_or(());
        stdin
    }))
}
