//! TODO/machine.md T-1870: a detached Windows or DOS guest. `run -d
//! windows:NAME` makes a machine (`podvm_windows::machines`), and `ps`,
//! `stop`, `rm` and `logs` know machines beside containers.
//!
//! ⛔ The emulator never runs without an owner. `run -d` starts a keeper
//! (`podvm windows keeper NAME ACCEL`, its own process group) and returns
//! when the keeper wrote the pid of the emulator into the record. The
//! keeper waits for the emulator, stops it at the end of its lifetime
//! (`--timeout`, else 24 h), and writes how it ended. `stop` and `rm -f`
//! end the emulator and wait for the keeper to finish.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use podvm_image::error::{EXIT_CLI_ERROR, EXIT_FLAG_ERROR, EXIT_RUNTIME_ERROR};
use podvm_windows::catalog::Kind;
use podvm_windows::machines::{self, Record};
use podvm_windows::{Accel, Plan};

/// The time that `run -d` waits for the keeper to start the emulator.
const START_WAIT: Duration = Duration::from_secs(20);
/// The time that `stop` gives the guest to power off after ACPI, before
/// QMP `quit` (docker gives 10 s).
const STOP_GRACE: Duration = Duration::from_secs(10);

/// The plan of the machine in `dir`: each per-machine file is in it.
fn plan_of(r: &Record, dir: &Path, accel: Accel) -> Plan {
    let (code, _) = super::plan::firmware();
    let options = match r.flavor.as_str() {
        "desktop" => {
            let forwards = r.forwards.split(',');
            let mut o = podvm_windows::options::desktop(
                accel,
                forwards
                    .filter_map(crate::run::publish::forward_of)
                    .collect(),
            );
            // TODO/machine.md T-1868: the channel of the QEMU Guest Agent.
            let qga = machines::qga_socket(&machines::root(), &r.name);
            o.channels.push(podvm_windows::qga::channel(&qga));
            o
        }
        _ => podvm_windows::options::mailbox(),
    };
    Plan {
        emulator: super::plan::emulator(),
        accel,
        share: super::plan::share(),
        firmware_code: code,
        firmware_vars: dir.join("vars.fd"),
        root: dir.join("disk.qcow2"),
        mailbox: dir.join("mailbox.img"),
        serial: dir.join("serial.log"),
        monitor: machines::socket(&machines::root(), &r.name),
        memory_mib: r.memory_mib,
        cpus: 2,
        emu_args: Vec::new(),
        options,
    }
}

/// The base image of a catalog name, and the flavor of its boot line.
fn base_of(verb: &str, image: &str) -> Result<(PathBuf, &'static str), i32> {
    let Some(name) = podvm_windows::catalog::name_of(image) else {
        eprintln!(
            "podvm {verb}: run -d of a Windows guest takes a name of the catalog \
             (windows:dos, windows:validationos, windows:2025); {image} is none"
        );
        return Err(EXIT_FLAG_ERROR);
    };
    let e = podvm_windows::catalog::resolve(name).map_err(|why| {
        eprintln!("podvm {verb}: {why}");
        EXIT_CLI_ERROR
    })?;
    match e.kind {
        Kind::DosZip => Ok((super::pull::dos_base(verb)?, "dos")),
        Kind::ValidationOs => Ok((super::named::vos_image(verb, e)?, "mailbox")),
        Kind::Iso => Ok((super::named::golden_image(verb, e)?, "desktop")),
        Kind::Drivers => Err(super::named::no_guest(verb, e)),
    }
}

/// A free name: `win-` and six hex digits of the nonce.
fn fresh_name(root: &Path) -> Result<String, i32> {
    for _ in 0..8 {
        let n = podvm_windows::agent::nonce().map_err(|_| EXIT_RUNTIME_ERROR)?;
        let name = format!("win-{}", &n[..6.min(n.len())]);
        if !root.join(&name).exists() {
            return Ok(name);
        }
    }
    Err(EXIT_RUNTIME_ERROR)
}

/// The mailbox of a machine: empty for DOS; for Windows the marker and
/// `KEEP`, so that the agent of the image polls it for a command.
fn mailbox(flavor: &str) -> podvm_windows::Result<podvm_windows::Fat16> {
    use podvm_windows::agent;
    let s = podvm_windows::settings();
    let mut m = podvm_windows::Fat16::with_oem(&s.oem)?;
    if flavor != "dos" {
        m.put(agent::MARKER, &s.marker)?;
        m.put(agent::KEEP, b"")?;
    }
    Ok(m)
}

/// `run -d windows:NAME`: make the machine, start its keeper, print its
/// id, as docker prints the id of a container (TODO/machine.md T-2042).
pub(crate) fn run_detached(
    verb: &str,
    name: Option<&str>,
    image: &str,
    mem: Option<u64>,
    lifetime: Option<u64>,
    publish: &[String],
) -> i32 {
    match create(verb, name, image, mem, lifetime, publish) {
        Ok(name) => {
            println!(
                "{}",
                find(&name).map_or(name, |r| r.id_or_name().to_string())
            );
            0
        }
        Err(c) => c,
    }
}

/// Make the machine and start its keeper; give its name. Each refusal is
/// on stderr (TODO/machine.md T-1933: a foreground run uses it too, and its
/// stdout is the output of the command only).
fn create(
    verb: &str,
    name: Option<&str>,
    image: &str,
    mem: Option<u64>,
    lifetime: Option<u64>,
    publish: &[String],
) -> Result<String, i32> {
    let root = machines::root();
    let name = match name {
        Some(n) if !machines::valid_name(n) => {
            eprintln!("podvm {verb}: {n:?} is no valid name ([a-zA-Z0-9][a-zA-Z0-9_.-]*)");
            return Err(EXIT_FLAG_ERROR);
        }
        Some(n) => n.to_string(),
        None => fresh_name(&root)?,
    };
    let dir = machines::dir_in(&root, &name);
    if dir.exists() {
        eprintln!(
            "podvm {verb}: the name {name:?} is in use; `podvm rm {name}` removes the old one"
        );
        return Err(EXIT_RUNTIME_ERROR);
    }
    // The kind of the name decides the network, before any fetch.
    let desktop = podvm_windows::catalog::name_of(image)
        .and_then(|n| podvm_windows::catalog::resolve(n).ok())
        .is_some_and(|e| e.kind == Kind::Iso);
    if !publish.is_empty() && !desktop {
        eprintln!(
            "podvm {verb}: -p needs a guest with a network: {image} has none; a setup ISO \
             of the catalog (windows:2025, 11, 10) has one (TODO/machine.md T-1869)"
        );
        return Err(EXIT_FLAG_ERROR);
    }
    let (base, flavor) = base_of(verb, image)?;
    super::plan::provide(verb, flavor != "dos")?;
    let accel = super::plan::accelerator()?;
    let r = Record {
        name: name.clone(),
        image: image.to_string(),
        base: base.display().to_string(),
        flavor: flavor.to_string(),
        created: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        lifetime: lifetime.unwrap_or(machines::DEFAULT_LIFETIME_SECS),
        memory_mib: mem
            .map(|m| m / (1 << 20))
            .unwrap_or(if flavor == "dos" { 512 } else { 4096 }),
        forwards: publish.join(","),
        id: machines::new_id(&name),
        ..Default::default()
    };
    if let Err(why) = make(&r, &dir, accel) {
        let _ = std::fs::remove_dir_all(&dir);
        eprintln!("podvm {verb}: {name}: {why}");
        return Err(EXIT_RUNTIME_ERROR);
    }
    match start_keeper(&name, accel, &dir) {
        Ok(()) => Ok(name),
        Err(why) => {
            eprintln!("podvm {verb}: {name}: {why}");
            let _ = stop_record(&root, &name);
            let _ = std::fs::remove_dir_all(&dir);
            Err(EXIT_RUNTIME_ERROR)
        }
    }
}

/// The directory of a new machine: the overlay, the mailbox, the UEFI
/// variables and the record.
fn make(r: &Record, dir: &Path, accel: Accel) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let plan = plan_of(r, dir, accel);
    let base = std::fs::canonicalize(&r.base).unwrap_or_else(|_| PathBuf::from(&r.base));
    podvm_windows::emulator::run_steps(&podvm_windows::overlay_argv(
        &super::plan::qemu_img(),
        &base,
        &plan.root,
    ))
    .map_err(|e| e.to_string())?;
    let m = mailbox(&r.flavor).map_err(|e| e.to_string())?;
    std::fs::write(&plan.mailbox, m.image()).map_err(|e| e.to_string())?;
    if r.flavor != "dos" {
        let (_, vars) = super::plan::firmware();
        std::fs::copy(&vars, &plan.firmware_vars)
            .map_err(|e| format!("{}: {e}", vars.display()))?;
    }
    machines::write(&machines::root(), r).map_err(|e| e.to_string())
}

/// Start the keeper in its own process group, and wait until it wrote the
/// pid of the emulator.
fn start_keeper(name: &str, accel: Accel, dir: &Path) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let log = std::fs::File::create(dir.join("keeper.log")).map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;
    std::process::Command::new(exe)
        .args(["windows", "keeper", name, accel.word()])
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(err)
        .process_group(0)
        .spawn()
        .map_err(|e| e.to_string())?;
    let t0 = Instant::now();
    let root = machines::root();
    while t0.elapsed() < START_WAIT {
        if let Some(r) = machines::read(&root, name) {
            if r.running() {
                return Ok(());
            }
            if !r.ended.is_empty() {
                return Err(format!("the emulator ended at once: {}", r.ended));
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let tail = std::fs::read_to_string(dir.join("keeper.log")).unwrap_or_default();
    Err(format!(
        "the keeper started no emulator within {} s: {}",
        START_WAIT.as_secs(),
        tail.lines().last().unwrap_or("no line")
    ))
}

/// `podvm windows keeper NAME ACCEL`: own the emulator of the machine.
pub(crate) fn keeper(args: &[String]) -> i32 {
    let (Some(name), Some(word)) = (args.first(), args.get(1)) else {
        eprintln!("podvm windows keeper: NAME and ACCEL are needed");
        return EXIT_FLAG_ERROR;
    };
    let accel = if word == "kvm" {
        Accel::Kvm
    } else {
        Accel::Tcg
    };
    let root = machines::root();
    let Some(mut r) = machines::read(&root, name) else {
        eprintln!("podvm windows keeper: no machine {name:?}");
        return EXIT_RUNTIME_ERROR;
    };
    let dir = machines::dir_in(&root, name);
    let plan = plan_of(&r, &dir, accel);
    // TODO/machine.md T-1899: the screen on a VNC socket, for `windows view`.
    let vnc = machines::vnc_socket(&root, name);
    let _ = std::fs::remove_file(&vnc);
    let qga = machines::qga_socket(&root, name); // TODO/machine.md T-1868
    let _ = std::fs::remove_file(&qga);
    let argv = if r.flavor == "dos" {
        podvm_windows::dos::argv(
            accel,
            &plan.root,
            &plan.mailbox,
            &plan.serial,
            &plan.monitor,
            plan.memory_mib,
            &["-vnc".to_string(), format!("unix:{}", vnc.display())],
        )
    } else {
        let mut plan = plan.clone();
        plan.options.vnc = Some(vnc.clone());
        podvm_windows::argv(&plan)
    };
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("emulator.log"))
        .map(Stdio::from)
        .unwrap_or_else(|_| Stdio::null());
    let mut child = match podvm_windows::emulator::spawn_emulator(&plan, &argv, log) {
        Ok(c) => c,
        Err(e) => {
            r.ended = format!("the emulator did not start: {e}");
            let _ = machines::write(&root, &r);
            return EXIT_RUNTIME_ERROR;
        }
    };
    r.keeper_pid = std::process::id();
    r.keeper_start = machines::start_of(r.keeper_pid).unwrap_or(0);
    r.emulator_pid = child.id();
    r.emulator_start = machines::start_of(child.id()).unwrap_or(0);
    let _ = machines::write(&root, &r);
    let t0 = Instant::now();
    let lifetime = Duration::from_secs(r.lifetime);
    let ended = loop {
        match child.try_wait() {
            Ok(Some(s)) => break format!("the emulator ended, {s}"),
            Ok(None) => {}
            Err(e) => break format!("the wait for the emulator failed: {e}"),
        }
        if t0.elapsed() >= lifetime {
            let stop = podvm_windows::emulator::stop_emulator(&plan, &mut child);
            break format!("its lifetime of {} s ended; {stop}", r.lifetime);
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    // A `stop` may have written its own end first: that one stays.
    let mut now = machines::read(&root, name).unwrap_or(r);
    if now.ended.is_empty() {
        now.ended = ended;
    }
    let _ = machines::write(&root, &now);
    let _ = std::fs::remove_file(&plan.monitor);
    let _ = std::fs::remove_file(&vnc);
    let _ = std::fs::remove_file(&qga);
    0
}

/// The record of `name`, when a machine has that name.
pub(crate) fn find(name: &str) -> Option<Record> {
    machines::find_in(&machines::root(), name) // T-2042: or its id
}

/// End the emulator of `name`: `guest-shutdown` through the agent of a
/// full Windows guest (T-1868), then ACPI power-off, then QMP `quit`, then
/// SIGKILL to its process group; then wait for the keeper. Gives the end
/// that it wrote, or `None` where the machine did not run.
fn stop_record(root: &Path, name: &str) -> Option<String> {
    let r = machines::read(root, name)?;
    if !r.running() {
        return None;
    }
    let sock = machines::socket(root, name);
    let mut how = "SIGKILL";
    if r.flavor == "desktop" && agent::graceful(root, &r) {
        how = "guest-shutdown";
    }
    // TODO/machine.md T-1994: a guest of `run -d` gets SIGTERM, as `docker
    // stop` gives one: the keeper forwards it to the command in the guest
    // (T-2041), and the exit code of the command ends the record.
    let termed = r.flavor == "linux" && machines::alive(r.keeper_pid, r.keeper_start);
    if termed && life::sigterm_keeper(&r, STOP_GRACE) {
        return machines::read(root, name).map(|now| now.ended);
    }
    let qmp = r
        .running()
        .then(|| podvm_windows::qmp::Monitor::connect(&sock, Duration::from_secs(2)));
    if let Some(Ok(mut m)) = qmp {
        let _ = m.call("system_powerdown");
        let t0 = Instant::now();
        // A Linux guest boots with `acpi=off` and takes no power button.
        let grace = if r.flavor == "linux" {
            Duration::ZERO
        } else {
            STOP_GRACE
        };
        while t0.elapsed() < grace && r.running() {
            std::thread::sleep(Duration::from_millis(250));
        }
        how = "ACPI power-off";
        if r.running() {
            m.quit();
            std::thread::sleep(Duration::from_millis(500));
            how = "QMP quit";
        }
    }
    if r.running() {
        let _ = podvm_probe::sys::kill(-i64::from(r.emulator_pid), 9);
        how = "SIGKILL";
    }
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(10)
        && (r.running() || machines::alive(r.keeper_pid, r.keeper_start))
    {
        std::thread::sleep(Duration::from_millis(100));
    }
    let end = if termed {
        // As `docker stop` after its grace: the exit code of a SIGKILL.
        format!(
            "stopped by podvm stop ({how}) after {} s of SIGTERM, exit code 137",
            STOP_GRACE.as_secs()
        )
    } else if r.flavor == "linux" {
        // No keeper got the SIGTERM: say so, with the code of a SIGKILL.
        format!("stopped by podvm stop ({how}) with no keeper, exit code 137")
    } else {
        format!("stopped by podvm stop ({how})")
    };
    if let Some(mut now) = machines::read(root, name) {
        now.ended = end.clone();
        let _ = machines::write(root, &now);
    }
    Some(end)
}

/// `podvm stop NAME` of a machine.
pub(crate) fn stop(name: &str) -> i32 {
    let _ = stop_record(&machines::root(), name);
    println!("{name}");
    0
}

/// `rm` of `want` where a machine has that name; `None` for no machine.
pub(crate) fn rm_if(want: &str, force: bool) -> Option<i32> {
    find(want).map(|r| rm(&r.name, force))
}

/// `run -d windows:NAME`, where the run is detached and names the catalog.
/// Each other run with `-p` is refused by name (TODO/machine.md T-1869).
pub(crate) fn detach_if(verb: &str, o: &crate::run::Opts) -> Option<i32> {
    if verb == "run" && o.detach && super::named(o.image.as_deref()) {
        return Some(run_detached(
            verb,
            o.name.as_deref(),
            o.image.as_deref().unwrap_or_default(),
            o.mem,
            o.timeout,
            &o.publish,
        ));
    }
    (!o.publish.is_empty()).then(|| {
        eprintln!(
            "podvm {verb}: -p publishes a port of a detached Windows guest only (run -d -p \
             HOST:GUEST windows:NAME); this payload shares the network of this machine, so a \
             port it opens is already a port of this machine (TODO/machine.md T-1869)"
        );
        EXIT_FLAG_ERROR
    })
}

mod agent; // TODO/machine.md T-1868
pub(crate) use agent::{cp_if, exec};
mod life; // TODO/machine.md T-1870
pub(crate) use life::{rm, start_if};
// TODO/machine.md T-1933: a foreground run of a golden image on the agent.
pub(crate) mod once;
// TODO/machine.md T-1869, T-1870: the verbs that read a machine.
mod view;
pub(crate) use view::{logs, port, screenshot};
mod viewer; // TODO/machine.md T-1899
pub(crate) use viewer::view;
