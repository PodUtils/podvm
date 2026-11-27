//! TODO/machine.md T-1870: `start` and `rm` of a machine, out of
//! `machine.rs` for the limit of 500 lines of a source file.

use std::time::{Duration, Instant};

use podvm_image::error::EXIT_RUNTIME_ERROR;
use podvm_windows::machines;

use super::{find, start_keeper, stop_record, STOP_GRACE};

/// `start` of `want` where a machine has that name: a running one changes
/// nothing; a stopped one boots again from its own disk. `None` for no
/// machine.
pub(crate) fn start_if(verb: &str, want: &str) -> Option<Result<String, i32>> {
    let mut r = find(want)?;
    if r.running() {
        return Some(Ok(r.name));
    }
    if r.flavor == "linux" {
        eprintln!(
            "podvm {verb}: {want} is a guest of the machine tier, whose disk is the image: \
             `podvm rm {want}` and `podvm run -d` start a new one (TODO/machine.md T-1750)"
        );
        return Some(Err(EXIT_RUNTIME_ERROR));
    }
    if machines::alive(r.keeper_pid, r.keeper_start) {
        eprintln!("podvm {verb}: the keeper of {want} still runs; wait for it to end");
        return Some(Err(EXIT_RUNTIME_ERROR));
    }
    let started = (|| {
        crate::windows::plan::provide(verb, r.flavor != "dos")?;
        let accel = crate::windows::plan::accelerator()?;
        r.ended.clear();
        r.keeper_pid = 0;
        r.keeper_start = 0;
        r.emulator_pid = 0;
        r.emulator_start = 0;
        let root = machines::root();
        machines::write(&root, &r).map_err(|e| {
            eprintln!("podvm {verb}: {want}: {e}");
            EXIT_RUNTIME_ERROR
        })?;
        start_keeper(&r.name, accel, &machines::dir_in(&root, &r.name)).map_err(|why| {
            eprintln!("podvm {verb}: {want}: {why}");
            EXIT_RUNTIME_ERROR
        })
    })();
    Some(started.map(|()| r.name.clone()))
}

/// `podvm rm [-f] NAME` of a machine.
pub(crate) fn rm(name: &str, force: bool) -> i32 {
    let code = remove("rm", name, force);
    if code == 0 {
        println!("{name}");
    }
    code
}

/// Remove the machine `name` (stop it first with `force`), with each
/// report on stderr under `verb`.
pub(super) fn remove(verb: &str, name: &str, force: bool) -> i32 {
    let root = machines::root();
    let Some(r) = machines::read(&root, name) else {
        return EXIT_RUNTIME_ERROR;
    };
    if r.running() {
        if !force {
            eprintln!(
                "podvm {verb}: the machine {name} runs; `podvm stop {name}` first, or `podvm rm -f {name}`"
            );
            return EXIT_RUNTIME_ERROR;
        }
        let _ = stop_record(&root, name);
    }
    // A keeper that saw its emulator end writes the end and exits.
    let t0 = Instant::now();
    while machines::alive(r.keeper_pid, r.keeper_start) && t0.elapsed() < STOP_GRACE {
        std::thread::sleep(Duration::from_millis(100));
    }
    if r.running() || machines::alive(r.keeper_pid, r.keeper_start) {
        eprintln!("podvm {verb}: the emulator of {name} did not end; nothing was removed");
        return EXIT_RUNTIME_ERROR;
    }
    match std::fs::remove_dir_all(machines::dir_in(&root, name)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("podvm {verb}: {name}: {e}");
            EXIT_RUNTIME_ERROR
        }
    }
}

/// SIGTERM to the keeper of the guest `r` of `run -d`, and the wait of
/// `grace` for its end. True where the keeper ended with the guest
/// and wrote the end of the record; false where it runs on (the caller
/// then ends the emulator) or no keeper of it runs.
pub(super) fn sigterm_keeper(r: &machines::Record, grace: Duration) -> bool {
    if !machines::alive(r.keeper_pid, r.keeper_start) {
        return false;
    }
    let _ = podvm_probe::sys::kill(i64::from(r.keeper_pid), 15);
    let t0 = Instant::now();
    while t0.elapsed() < grace {
        if !r.running() && !machines::alive(r.keeper_pid, r.keeper_start) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TODO/machine.md T-1994: `stop` of a guest of `run -d` gives its
    /// keeper SIGTERM and waits for it: a keeper that ends with the guest
    /// is a stop with the exit code of the command; one that runs on past
    /// the grace, or no keeper, is not (the caller then ends the emulator).
    #[test]
    fn stop_gives_the_keeper_sigterm_and_waits_for_its_end() {
        let keeper = |script: &str| {
            let c = std::process::Command::new("sh")
                .args(["-c", script])
                .spawn()
                .unwrap();
            std::thread::sleep(Duration::from_millis(200));
            let pid = c.id();
            let r = machines::Record {
                keeper_pid: pid,
                keeper_start: machines::start_of(pid).unwrap(),
                ..Default::default()
            };
            (c, r)
        };
        let grace = Duration::from_secs(2);
        for (script, ends) in [("exec sleep 30", true), ("trap '' TERM; sleep 5", false)] {
            let (mut c, r) = keeper(script);
            assert_eq!(sigterm_keeper(&r, grace), ends, "{script}");
            let _ = c.kill();
            let _ = c.wait();
        }
        assert!(
            !sigterm_keeper(&machines::Record::default(), grace),
            "no keeper"
        );
    }
}
