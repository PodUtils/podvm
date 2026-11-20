//! TODO/machine.md T-1750: `run -d --podvm-tier=machine`, a guest that
//! stays for `exec`. podvm runs itself again as the keeper of the guest
//! (the same argv, and `PODVM_MACHINE_KEEPER=NAME`, which makes the run a
//! foreground boot): the
//! keeper runs the foreground boot with an exec port, a QMP monitor and a
//! pid file ([`super::guest::exec::Keeper`]), its stdout in `serial.log`
//! of the machine, so that `logs` reads the payload. The record is the one
//! of the Windows machines (`podvm_windows::machines`, flavor `linux`), so
//! `ps`, `stop`, `rm` and `logs` serve it.
//!
//! ⛔ The guest has a lifetime: `--timeout`, else 24 h.

use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use podvm_image::error::{EXIT_FLAG_ERROR, EXIT_RUNTIME_ERROR};
use podvm_windows::machines::{self, Record};

use super::guest::exec::Keeper;

/// The variable that names the machine in the keeper.
pub const KEEPER_ENV: &str = "PODVM_MACHINE_KEEPER";

/// The time that `run -d` waits for the keeper to start the emulator.
const START_WAIT: Duration = Duration::from_secs(120);

/// The files of the keeper of the run, where this process is one.
pub fn keeper() -> Option<Keeper> {
    let name = std::env::var(KEEPER_ENV).ok()?;
    machines::valid_name(&name).then(|| {
        let root = machines::root();
        Keeper {
            exec: machines::exec_socket(&root, &name),
            qmp: machines::socket(&root, &name),
            pidfile: machines::dir_in(&root, &name).join("emulator.pid"),
        }
    })
}

/// `run -d --podvm-tier=machine`: make the record, start the keeper, and
/// print the id once the emulator runs (TODO/machine.md T-2042).
pub fn start(
    verb: &str,
    name: Option<&str>,
    image: &str,
    mem: Option<u64>,
    lifetime: Option<u64>,
) -> i32 {
    let root = machines::root();
    let name = match name {
        Some(n) if !machines::valid_name(n) => {
            eprintln!("podvm {verb}: {n:?} is no valid name ([a-zA-Z0-9][a-zA-Z0-9_.-]*)");
            return EXIT_FLAG_ERROR;
        }
        Some(n) => n.to_string(),
        None => match podvm_windows::agent::nonce() {
            Ok(n) => format!("vm-{}", &n[..6.min(n.len())]),
            Err(_) => return EXIT_RUNTIME_ERROR,
        },
    };
    let dir = machines::dir_in(&root, &name);
    if dir.exists() {
        eprintln!(
            "podvm {verb}: the name {name:?} is in use; `podvm rm {name}` removes the old one"
        );
        return EXIT_RUNTIME_ERROR;
    }
    // The keeper takes the caller's argv as it is: `-d` stays, and the
    // variable of the keeper makes the run a foreground boot.
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let r = Record {
        name: name.clone(),
        image: image.to_string(),
        base: String::new(),
        flavor: "linux".to_string(),
        created: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        lifetime: lifetime.unwrap_or(machines::DEFAULT_LIFETIME_SECS),
        memory_mib: mem.map(|m| m / (1 << 20)).unwrap_or(0),
        id: machines::new_id(&name),
        ..Default::default()
    };
    let r_id = r.id.clone();
    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|()| machines::write(&root, &r)) {
        eprintln!("podvm {verb}: {}: {e}", dir.display());
        return EXIT_RUNTIME_ERROR;
    }
    match spawn(&argv, &name, &dir).and_then(|pid| wait_up(&root, r, &dir, pid)) {
        Ok(()) => {
            println!("{}", r_id); // T-2042: the id, as docker prints it
            0
        }
        Err(why) => {
            eprintln!("podvm {verb}: {name}: {why}");
            let _ = std::fs::remove_dir_all(&dir);
            EXIT_RUNTIME_ERROR
        }
    }
}

/// Start the keeper in its own process group; its pid.
fn spawn(argv: &[String], name: &str, dir: &Path) -> Result<u32, String> {
    use std::os::unix::process::CommandExt;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let out = std::fs::File::create(dir.join("serial.log")).map_err(|e| e.to_string())?;
    let err = std::fs::File::create(dir.join("keeper.log")).map_err(|e| e.to_string())?;
    let child = std::process::Command::new(exe)
        .args(argv)
        .env(KEEPER_ENV, name)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .process_group(0)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(child.id())
}

/// Wait for the pid file of the keeper's boot, and write both pids into the
/// record.
fn wait_up(root: &Path, mut r: Record, dir: &Path, keeper: u32) -> Result<(), String> {
    let keeper_start = machines::start_of(keeper).unwrap_or(0);
    let t0 = Instant::now();
    while t0.elapsed() < START_WAIT {
        if let Ok(text) = std::fs::read_to_string(dir.join("emulator.pid")) {
            let mut it = text.split_whitespace();
            if let (Some(Ok(pid)), Some(Ok(start))) =
                (it.next().map(str::parse), it.next().map(str::parse))
            {
                r.keeper_pid = keeper;
                r.keeper_start = keeper_start;
                r.emulator_pid = pid;
                r.emulator_start = start;
                return machines::write(root, &r).map_err(|e| e.to_string());
            }
        }
        if !machines::alive(keeper, keeper_start) {
            let log = std::fs::read_to_string(dir.join("keeper.log")).unwrap_or_default();
            return Err(format!(
                "the keeper ended before the guest started: {}",
                log.lines().last().unwrap_or("no line")
            ));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(format!(
        "the guest did not start within {} s",
        START_WAIT.as_secs()
    ))
}

/// In the keeper, after the boot: write how the guest ended.
pub fn ended(code: i32) {
    let Ok(name) = std::env::var(KEEPER_ENV) else {
        return;
    };
    let root = machines::root();
    if let Some(mut r) = machines::read(&root, &name)
        && r.ended.is_empty()
    {
        r.ended = format!("the guest ended, exit code {code}");
        let _ = machines::write(&root, &r);
    }
    for f in [
        machines::exec_socket(&root, &name),
        machines::socket(&root, &name),
    ] {
        let _ = std::fs::remove_file(f);
    }
}
