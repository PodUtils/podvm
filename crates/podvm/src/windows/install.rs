//! TODO/machine.md T-1867: `podvm pull windows:NAME` for a setup ISO of
//! the catalog. The driver boots the ISO with the answer stick
//! (`podvm_windows::media`) on a blank sparse disk, sends Enter through QMP
//! to pass "Press any key to boot from CD or DVD", watches the guest from
//! outside (`podvm_windows::install::phase_of`), and converts the disk into
//! the golden image of the catalog, with no backing file. The ISO stays in
//! the store and is never written.
//!
//! ⛔ One emulator, and it never outlives the driver: [`Guest`] stops it
//! on each return. The password of the guest goes into the stick and into
//! a file beside the golden image (mode 0600), never into an argv or a
//! log.

use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant};

use podvm_image::error::EXIT_RUNTIME_ERROR;
use podvm_windows::answer::{self, Answer, Target};
use podvm_windows::catalog::Entry;
use podvm_windows::install::{self as inst, Phase};
use podvm_windows::plan::{Medium, MediumKind};
use podvm_windows::Plan;

/// The golden image of a catalog name: beside its ISO in the store.
pub(crate) fn golden_path(e: &Entry) -> PathBuf {
    podvm_windows::catalog::store_dir().join(format!("windows-{}.golden.qcow2", e.name))
}

/// TODO/machine.md T-1871: the meta file of the golden image of `e`: the
/// date of the install and the days of its evaluation.
pub(crate) fn meta_path(e: &Entry) -> PathBuf {
    golden_path(e).with_extension("meta")
}

/// The file of the password of the golden image of `e`.
pub(crate) fn password_path(e: &Entry) -> PathBuf {
    password_path_of(&golden_path(e))
}

/// The emulator of an install, stopped when the value is dropped.
struct Guest<'a> {
    plan: &'a Plan,
    child: Option<Child>,
}

impl Guest<'_> {
    fn child(&mut self) -> &mut Child {
        self.child
            .as_mut()
            .expect("the child stays until the value ends")
    }

    /// True when the emulator exited.
    fn exited(&mut self) -> bool {
        matches!(self.child().try_wait(), Ok(Some(_)))
    }
}

impl Drop for Guest<'_> {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take()
            && !matches!(c.try_wait(), Ok(Some(_)))
        {
            let _ = podvm_windows::emulator::stop_emulator(self.plan, &mut c);
        }
    }
}

/// Print `why` for `name`, and give the runtime exit code.
fn fail(verb: &str, name: &str, why: impl std::fmt::Display) -> i32 {
    eprintln!("podvm {verb}: windows:{name}: {why}");
    EXIT_RUNTIME_ERROR
}

/// The checks before the download of the ISO of `e`: an answer file for
/// it, the accelerator, and room for the ISO and the install. A golden
/// image that exists passes.
pub(crate) fn preflight(verb: &str, e: &Entry) -> Result<(), i32> {
    if golden_path(e).is_file() {
        return Ok(());
    }
    target_of(verb, e)?;
    let accel = super::plan::accelerator()?;
    let asked_tcg = std::env::var(inst::ACCEL_VAR).is_ok_and(|v| v == "tcg");
    inst::admit(e.name, accel, asked_tcg).map_err(|why| {
        eprintln!("podvm {verb}: {why}");
        EXIT_RUNTIME_ERROR
    })?;
    let dir = podvm_windows::catalog::store_dir();
    // TODO/machine.md T-1868: the virtio-win CD comes with the ISO.
    let tools = &podvm_windows::catalog::VIRTIO_WIN;
    let iso = [e, tools]
        .iter()
        .filter(|m| !dir.join(m.file).is_file())
        .map(|m| m.size)
        .sum::<u64>();
    let fsize = super::plan::ceiling()?;
    // The store can be absent before the first pull: the disk is read at
    // its nearest parent that exists.
    let free = dir
        .ancestors()
        .find(|p| p.is_dir())
        .and_then(|p| podvm_image::space::read(&p.to_string_lossy()).ok())
        .map(|h| h.free.bytes);
    super::pull::room_for(inst::ROOM_BYTES + iso, free, fsize).map_err(|why| {
        fail(
            verb,
            e.name,
            format!("the install needs room (estimate): {why} (TODO/machine.md T-1862)"),
        )
    })
}

/// The target of the answer file of `e`, or the refusal.
fn target_of(verb: &str, e: &Entry) -> Result<Target, i32> {
    Target::of(e.name).ok_or_else(|| {
        fail(
            verb,
            e.name,
            "podvm has no answer file for this setup ISO yet (TODO/machine.md T-1867)",
        )
    })
}

/// Install the setup ISO `iso` of `e` into its golden image, or give the
/// golden image that exists. [`preflight`] ran first.
pub(crate) fn install(verb: &str, e: &Entry, iso: &Path, quiet: bool) -> Result<PathBuf, i32> {
    let golden = golden_path(e);
    if golden.is_file() {
        return Ok(golden);
    }
    let target = target_of(verb, e)?;
    let mut plan = super::plan::build(&super::root_bus::install_args()?)?;
    // The root disk goes into the store, where `preflight` counted its room.
    let work =
        super::install_dir::WorkDir::install(e.name).map_err(|why| fail(verb, e.name, why))?;
    plan.root = work.0.join("root.qcow2");
    let password = match prepare(verb, e.name, target, iso, &mut plan) {
        Ok(p) => p,
        Err(code) => {
            podvm_windows::cleanup(&plan);
            return Err(code);
        }
    };
    let _scratch = podvm_windows::RunGuard::arm(&plan);
    let done = boot_and_convert(verb, e.name, &plan, &password, &golden, quiet).and_then(|()| {
        // TODO/machine.md T-1871: the start of the evaluation, for the doctor.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let text = inst::meta_text(e.name, now, inst::eval_days(target));
        std::fs::write(meta_path(e), text).map_err(|why| fail(verb, e.name, why))
    });
    if done.is_err() {
        keep_failed(&plan);
        let _ = std::fs::remove_file(&golden);
        let _ = std::fs::remove_file(password_path(e));
        let _ = std::fs::remove_file(meta_path(e));
    }
    done.map(|()| golden)
}

/// TODO/machine.md T-1867: `PODVM_WINDOWS_KEEP_FAILED=DIR` keeps the root
/// disk of a failed install as `DIR/root.qcow2`, for the logs of Setup
/// (`$WINDOWS.~BT/Sources/Panther`), and the log stick as
/// `DIR/pelogs.img`, for the logs of WinPE (`podvm_windows::pelogs`).
/// The emulator stopped before this.
fn keep_failed(plan: &Plan) {
    let Some(dir) = std::env::var_os("PODVM_WINDOWS_KEEP_FAILED").map(PathBuf::from) else {
        return;
    };
    let logs = podvm_windows::pelogs::path_beside(&plan.root);
    let to = dir.join(podvm_windows::pelogs::IMAGE);
    match std::fs::create_dir_all(&dir).and_then(|()| std::fs::copy(&logs, &to)) {
        Ok(_) => eprintln!(
            "podvm: the log stick of the failed install is {}",
            to.display()
        ),
        Err(e) => eprintln!("podvm: PODVM_WINDOWS_KEEP_FAILED: {}: {e}", to.display()),
    }
    let to = dir.join("root.qcow2");
    let kept = std::fs::create_dir_all(&dir).and_then(|()| {
        std::fs::rename(&plan.root, &to).or_else(|_| std::fs::copy(&plan.root, &to).map(|_| ()))
    });
    match kept {
        Ok(()) => eprintln!(
            "podvm: the root disk of the failed install is {}",
            to.display()
        ),
        Err(e) => eprintln!("podvm: PODVM_WINDOWS_KEEP_FAILED: {}: {e}", to.display()),
    }
}

/// The blank root disk, the empty mailbox, the answer stick and the
/// devices of the install; gives the password of the guest.
fn prepare(
    verb: &str,
    name: &str,
    target: Target,
    iso: &Path,
    plan: &mut Plan,
) -> Result<String, i32> {
    let qemu_img = super::plan::qemu_img();
    let size = inst::DISK_BYTES.to_string();
    let create: Vec<String> = [
        qemu_img.to_string_lossy().as_ref(),
        "create",
        "-q",
        "-f",
        "qcow2",
        plan.root.to_string_lossy().as_ref(),
        size.as_str(),
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    podvm_windows::emulator::run_steps(&create).map_err(|why| fail(verb, name, why))?;
    // TODO/machine.md T-1867: the first logon installs the mailbox agent
    // from this volume, so that `podvm run windows:NAME` runs commands.
    let mailbox = agent_mailbox().map_err(|why| fail(verb, name, why))?;
    std::fs::write(&plan.mailbox, mailbox.image()).map_err(|why| fail(verb, name, why))?;
    let password = answer::password().map_err(|why| fail(verb, name, why))?;
    let dir = plan.root.parent().unwrap_or(Path::new(".")).to_path_buf();
    let stick = podvm_windows::media::write(
        &Answer {
            target,
            locale: "en-US".into(),
            user: "podvm".into(),
            password: password.clone(),
            computer: format!("podvm-{name}"),
        },
        &dir.join("answer.img"),
    )
    .map_err(|why| fail(verb, name, why))?;
    // TODO/machine.md T-1867: WinPE copies its logs to this stick.
    let logs = podvm_windows::pelogs::write(&podvm_windows::pelogs::path_beside(&plan.root))
        .map_err(|why| fail(verb, name, why))?;
    plan.options = podvm_windows::options::desktop(plan.accel, Vec::new());
    // TODO/machine.md T-1867: `PODVM_WINDOWS_HYPERV=0` drops the Hyper-V
    // flags, to tell a stall of nested KVM from one of the emulated disk.
    if std::env::var("PODVM_WINDOWS_HYPERV").as_deref() == Ok("0") {
        plan.options.hyperv = false;
    }
    // TODO/machine.md T-1868: the virtio-win CD of the guest tools, and
    // the channel of the agent, so that the virtio-serial driver binds.
    let tools = super::pull::fetched(verb, &podvm_windows::catalog::VIRTIO_WIN)?;
    plan.options.channels = vec![podvm_windows::qga::channel(&dir.join("qga.sock"))];
    plan.options.media = vec![
        Medium {
            kind: MediumKind::Cdrom,
            path: iso.to_path_buf(),
            readonly: true,
            boot: true,
        },
        podvm_windows::guest_tools::medium(&tools),
        stick,
        logs,
    ];
    plan.options.check().map_err(|why| fail(verb, name, why))?;
    Ok(password)
}

/// Boot the install, watch it to its end, and convert the disk into
/// `golden`.
fn boot_and_convert(
    verb: &str,
    name: &str,
    plan: &Plan,
    password: &str,
    golden: &Path,
    quiet: bool,
) -> Result<(), i32> {
    let qemu_img = super::plan::qemu_img();
    let dir = plan.root.parent().unwrap_or(Path::new(".")).to_path_buf();
    let log =
        std::fs::File::create(dir.join("emulator.log")).map_err(|why| fail(verb, name, why))?;
    let child = podvm_windows::emulator::spawn_emulator(
        plan,
        &super::root_bus::root_bus(podvm_windows::argv(plan)),
        log.into(),
    )
    .map_err(|why| fail(verb, name, why))?;
    let mut guest = Guest {
        plan,
        child: Some(child),
    };
    let start = Instant::now();
    let say = |phase: Phase, written: u64| {
        if !quiet {
            let l = inst::phase_line(verb, name, phase, start.elapsed(), written);
            eprintln!("{l}");
        }
    };
    say(Phase::Booting, 0);
    press_keys(&mut guest);
    watch(verb, name, &mut guest, start, &say)?;
    // The answer file shuts the guest down after the marker; the writes
    // are on the disk only when the emulator exited.
    let off = Instant::now();
    while off.elapsed() < inst::POWER_OFF && !guest.exited() {
        std::thread::sleep(Duration::from_millis(500));
    }
    if !guest.exited() {
        eprintln!(
            "podvm {verb}: windows:{name}: the guest did not power off within {}s; it is stopped",
            inst::POWER_OFF.as_secs()
        );
    }
    drop(guest);
    super::settle::after_install(verb, name, plan, quiet).map_err(|why| fail(verb, name, why))?;
    let part = golden.with_extension("part");
    let _ = std::fs::remove_file(&part);
    podvm_windows::emulator::run_steps(&podvm_windows::convert_argv(&qemu_img, &plan.root, &part))
        .map_err(|why| fail(verb, name, why))?;
    write_secret(&password_path_of(golden), password).map_err(|why| fail(verb, name, why))?;
    std::fs::rename(&part, golden).map_err(|why| fail(verb, name, why))?;
    if !quiet {
        eprintln!(
            "podvm {verb}: windows:{name}: installed in {}; the password of the user podvm is in {}",
            inst::elapsed(start.elapsed()),
            password_path_of(golden).display()
        );
    }
    Ok(())
}

/// The mailbox of an install: the marker, the agent and its installer,
/// as for the provisioning boot of Validation OS.
fn agent_mailbox() -> podvm_windows::Result<podvm_windows::Fat16> {
    use podvm_windows::agent;
    let s = podvm_windows::settings();
    let mut m = podvm_windows::Fat16::with_oem(&s.oem)?;
    m.put(agent::MARKER, &s.marker)?;
    m.put(agent::AGENT_NAME, agent::AGENT_CMD.as_bytes())?;
    m.put(agent::SETUP_NAME, agent::SETUP_CMD.as_bytes())?;
    // TODO/machine.md T-1868: the script of the virtio-win guest tools.
    let tools = podvm_windows::guest_tools::SCRIPT_TEXT;
    m.put(podvm_windows::guest_tools::SCRIPT, tools.as_bytes())?;
    Ok(m)
}

/// A screen dump of the guest into `PODVM_WINDOWS_SHOTS` where it names a
/// directory: the evidence of an install that hangs. Named by the elapsed
/// time and the phase; a failure of the dump changes nothing.
fn shot(plan: &Plan, start: Instant, phase: Phase) {
    let Some(dir) = std::env::var_os("PODVM_WINDOWS_SHOTS").map(PathBuf::from) else {
        return;
    };
    let file = dir.join(format!(
        "{:05}s-{}.ppm",
        start.elapsed().as_secs(),
        phase.words().replace(' ', "-")
    ));
    if let Ok(mut m) = podvm_windows::qmp::Monitor::connect(&plan.monitor, Duration::from_secs(1)) {
        let _ = std::fs::create_dir_all(&dir);
        let _ = m.screendump(&file);
    }
}

/// The last lines of the error stream of the emulator, as `; the
/// emulator said: ...`, or nothing.
fn log_tail(plan: &Plan) -> String {
    let dir = plan.root.parent().unwrap_or(Path::new("."));
    let bytes = std::fs::read(dir.join("emulator.log")).unwrap_or_default();
    let text = String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(2048)..]);
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return String::new();
    }
    let tail = lines[lines.len().saturating_sub(3)..].join(" | ");
    format!("; the emulator said: {tail}")
}

/// [`password_path`] of a golden image path.
fn password_path_of(golden: &Path) -> PathBuf {
    golden.with_extension("password")
}

/// Write `text` to `path` with mode 0600.
fn write_secret(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let _ = std::fs::remove_file(path);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(text.as_bytes())?;
    f.write_all(b"\n")
}

/// Send Enter each `KEY_EVERY` for `KEYS_FOR`, for the prompt of the
/// firmware of the ISO. A monitor that does not answer yet is no failure.
fn press_keys(guest: &mut Guest<'_>) {
    let start = Instant::now();
    let mut monitor = None;
    while start.elapsed() < inst::KEYS_FOR && !guest.exited() {
        if monitor.is_none() {
            monitor =
                podvm_windows::qmp::Monitor::connect(&guest.plan.monitor, inst::KEY_EVERY).ok();
        }
        if let Some(m) = monitor.as_mut()
            && m.keys(&["ret"]).is_err()
        {
            monitor = None;
        }
        std::thread::sleep(inst::KEY_EVERY);
    }
}

/// Watch the guest until the serial line holds the `READY` marker. An
/// emulator that exits first, and the deadline, are failures.
fn watch(
    verb: &str,
    name: &str,
    guest: &mut Guest<'_>,
    start: Instant,
    say: &dyn Fn(Phase, u64),
) -> Result<(), i32> {
    use std::os::unix::fs::MetadataExt;
    // `PODVM_WINDOWS_INSTALL_DEADLINE` (seconds) bounds an experiment.
    let deadline = crate::number::env(verb, &crate::number::INSTALL_DEADLINE)?
        .map(Duration::from_secs)
        .unwrap_or_else(|| inst::deadline(guest.plan.accel));
    let mut phase = Phase::Booting;
    let mut last_line = Instant::now();
    loop {
        let serial = std::fs::read(&guest.plan.serial).unwrap_or_default();
        let written = std::fs::metadata(&guest.plan.root)
            .map(|m| m.blocks() * 512)
            .unwrap_or(0);
        let now = inst::phase_of(&serial, written);
        if now > phase || last_line.elapsed() >= inst::PROGRESS_EVERY {
            phase = phase.max(now);
            say(phase, written);
            last_line = Instant::now();
            shot(guest.plan, start, phase);
        }
        if phase == Phase::Ready {
            return Ok(());
        }
        if guest.exited() {
            return Err(fail(
                verb,
                name,
                format!(
                    "the emulator exited in the phase \"{}\", before the install ended{}",
                    phase.words(),
                    log_tail(guest.plan)
                ),
            ));
        }
        if start.elapsed() >= deadline {
            shot(guest.plan, start, phase);
            return Err(fail(
                verb,
                name,
                format!(
                    "the install did not end within {} (phase \"{}\"); the emulator is stopped",
                    inst::elapsed(deadline),
                    phase.words()
                ),
            ));
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}
