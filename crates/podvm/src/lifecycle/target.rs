//! TODO/machine.md T-2042 (OQ-03 of the sixth outside pass): a detached
//! guest of the machine tier (and a detached Windows or DOS machine) for
//! each verb that takes a container.
//!
//! The keeper of such a guest keeps a record of its own
//! (`podvm_windows::machines`), not a record of `podvm_supervise::table`.
//! Before, `ps` and `ps -q` listed it, `stop`, `rm -f`, `logs` and `exec`
//! found it, and `inspect` ("no such image"), `kill` and `wait` ("no such
//! container") and the filters and `--format` of `ps` did not.
//!
//! Now each verb finds a name in the records of both kinds through one
//! lookup, `crate::windows::machine::find` (by the name, or by the id or a
//! prefix of 12 hex digits of it, `machines::find_in`), and reads a machine
//! through one view, [`view`]: the record of the table that the machine
//! would have. `inspect` and `ps` render the view with the fields of a
//! container; `kill` gives the signal to the keeper, which forwards it to
//! the command in the guest (T-2041), or ends the emulator ([`route`]);
//! `wait` waits for the keeper and gives the code of the guest ([`code_of`]).

use std::time::{Duration, Instant};

use podvm_image::error::{EXIT_CLI_ERROR, EXIT_RUNTIME_ERROR};
use podvm_supervise::table::{Container, State};
use podvm_windows::machines::{self, Record};

/// The word of the end of a guest that `kill` ended with SIGKILL.
const KILLED: &str = "killed by podvm kill (SIGKILL), exit code 137";

/// The exit code in the end of a record (`ended`), where it has one: "the
/// guest ended, exit code N" of the keeper, or the line of [`KILLED`].
pub(crate) fn code_of(ended: &str) -> Option<i32> {
    ended
        .rsplit_once("exit code ")
        .and_then(|(_, n)| n.trim().parse::<i32>().ok())
}

/// The record of the container table that the machine `r` would have, for
/// the fields of `inspect` and `ps`. `running` is `r.running()`.
pub(crate) fn view(r: &Record, running: bool) -> Container {
    let at = |s: u64| podvm_image::clock::iso8601(std::time::UNIX_EPOCH + Duration::from_secs(s));
    let code = (!running).then(|| code_of(&r.ended)).flatten();
    Container {
        id: r.id_or_name().to_string(),
        name: r.name.clone(),
        argv: vec![format!("({} guest)", r.flavor)],
        env: Vec::new(),
        working_dir: String::new(),
        created_at: at(r.created),
        started_at: Some(at(r.created)),
        finished_at: None,
        state: if running {
            State::Running
        } else {
            State::Exited
        },
        pid: running.then_some(i64::from(r.emulator_pid)),
        launcher_pid: running.then_some(i64::from(r.keeper_pid)),
        exit_code: code,
        noticed: None,
        wall_ms: Some(r.lifetime.saturating_mul(1000)),
        end: None,
        rm: false,
        of: podvm_supervise::table::Detail {
            image: r.image.clone(),
            rung: "machine".to_string(),
            ..Default::default()
        },
    }
}

/// The views of the machines for `ps`: each one with `all`, else the ones
/// that run.
pub(crate) fn views(all: bool) -> Vec<Container> {
    machines::list(&machines::root())
        .into_iter()
        .filter_map(|r| {
            let running = r.running();
            (all || running).then(|| view(&r, running))
        })
        .collect()
}

/// The fields of `inspect` of the machine `r`: those of a container, with
/// the log of the guest and no tree of its own.
pub(crate) fn fields(
    s: &podvm_image::Store,
    r: &Record,
) -> (Container, Vec<(&'static str, String)>) {
    let c = view(r, r.running());
    let log = machines::dir_in(&machines::root(), &r.name).join("serial.log");
    let mut f = super::container_fields(s, &c);
    for (k, v) in f.iter_mut() {
        match *k {
            "LogPath" => *v = log.display().to_string(),
            "RootfsPath" => *v = "-".to_string(),
            "Contains" => {
                *v = "a guest of its own kernel: its keeper ends the emulator".to_string()
            }
            _ => {}
        }
    }
    (c, f)
}

/// What `kill` does with a machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    /// The keeper takes the signal and gives it to the command in the
    /// guest (a Linux guest, a signal of `signals::SET`).
    Forward,
    /// SIGKILL: the emulator ends, and the record says so.
    KillEmulator,
    /// The machine does not run.
    NotRunning,
    /// No route for this signal to this guest.
    Refuse,
}

/// The route of `sig` to a machine of `flavor`.
pub(crate) fn route(flavor: &str, running: bool, sig: i32) -> Route {
    let forwarded = crate::machine::guest::signals::SET
        .iter()
        .any(|(s, _)| *s == sig);
    match (running, sig) {
        (false, _) => Route::NotRunning,
        (true, 9) => Route::KillEmulator,
        (true, _) if flavor == "linux" && forwarded => Route::Forward,
        _ => Route::Refuse,
    }
}

/// `kill -s SIG NAME` of the machine `r`.
pub(crate) fn kill(r: &Record, sig: i32) -> Result<(), i32> {
    let keeper = machines::alive(r.keeper_pid, r.keeper_start);
    match route(&r.flavor, r.running(), sig) {
        Route::Forward if keeper => {
            podvm_probe::sys::kill(i64::from(r.keeper_pid), i64::from(sig)).map_err(|e| {
                eprintln!(
                    "podvm kill: {}: the keeper refused signal {sig}: {}",
                    r.name,
                    e.name()
                );
                EXIT_RUNTIME_ERROR
            })?;
            Ok(())
        }
        Route::KillEmulator | Route::Forward => {
            let root = machines::root();
            if let Some(mut now) = machines::read(&root, &r.name) {
                now.ended = KILLED.to_string();
                if let Err(e) = machines::write(&root, &now) {
                    eprintln!("podvm kill: {}: the record: {e}", r.name);
                }
            }
            // The keeper started the emulator in a process group of its own.
            let _ = podvm_probe::sys::kill(-i64::from(r.emulator_pid), 9);
            Ok(())
        }
        Route::NotRunning => {
            eprintln!("podvm kill: {} is not running: {}", r.name, r.status());
            Err(EXIT_CLI_ERROR)
        }
        Route::Refuse => {
            eprintln!(
                "podvm kill: signal {sig} has no route to {} ({} guest): a Linux guest of the \
                 machine tier takes SIGHUP, SIGINT, SIGQUIT, SIGUSR1, SIGUSR2, SIGTERM, SIGALRM \
                 and SIGPWR, and SIGKILL ends each guest (TODO/machine.md T-2042)",
                r.name, r.flavor
            );
            Err(EXIT_CLI_ERROR)
        }
    }
}

/// `wait NAME` of the machine `r`: the exit code of the guest, once its
/// keeper and its emulator ended, within `bound_ms`.
pub(crate) fn wait(r: &Record, bound_ms: u64) -> Result<i32, i32> {
    let root = machines::root();
    let t0 = Instant::now();
    let mut now = r.clone();
    while now.running() || machines::alive(now.keeper_pid, now.keeper_start) {
        if t0.elapsed() >= Duration::from_millis(bound_ms) {
            eprintln!(
                "podvm wait: {} still runs after {} s (TODO/supervise.md T-0604)",
                r.name,
                bound_ms / 1000
            );
            return Err(EXIT_RUNTIME_ERROR);
        }
        std::thread::sleep(Duration::from_millis(200));
        now = machines::read(&root, &r.name).unwrap_or(now);
    }
    code_of(&now.ended).ok_or_else(|| {
        eprintln!(
            "podvm wait: {} has no exit code podvm measured: {} (TODO/supervise.md T-0604)",
            r.name,
            now.status()
        );
        EXIT_RUNTIME_ERROR
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-2042: the route of `kill` for each class: a forwarded signal to a
    /// Linux guest, SIGKILL to each guest, a guest that does not run, and a
    /// signal with no route.
    #[test]
    fn kill_has_one_route_for_each_kind_of_guest_and_signal() {
        let cases = [
            ("linux", true, 15, Route::Forward),
            ("linux", true, 2, Route::Forward),
            ("linux", true, 1, Route::Forward),
            ("linux", true, 30, Route::Forward),
            ("linux", true, 9, Route::KillEmulator),
            ("linux", true, 19, Route::Refuse),
            ("linux", false, 15, Route::NotRunning),
            ("linux", false, 9, Route::NotRunning),
            ("desktop", true, 9, Route::KillEmulator),
            ("desktop", true, 15, Route::Refuse),
            ("dos", true, 2, Route::Refuse),
        ];
        for (flavor, running, sig, want) in cases {
            assert_eq!(
                route(flavor, running, sig),
                want,
                "{flavor} {running} {sig}"
            );
        }
    }

    /// T-2042: the view of a machine is a record of the table: the id, the
    /// name, the state, the pids while it runs, and the code of its end
    /// only after it, never a guessed zero.
    #[test]
    fn a_machine_reads_as_a_container_of_the_table() {
        let r = Record {
            name: "m".into(),
            id: "ab".repeat(32),
            image: "alpine:3.20".into(),
            flavor: "linux".into(),
            keeper_pid: 10,
            emulator_pid: 11,
            ended: "the guest ended, exit code 4".into(),
            ..Default::default()
        };
        let up = view(&r, true);
        assert_eq!((up.id.len(), up.name.as_str()), (64, "m"));
        assert_eq!(
            (up.state, up.pid, up.launcher_pid),
            (State::Running, Some(11), Some(10))
        );
        assert_eq!((up.exit_code, up.status()), (None, "Up".to_string()));
        assert_eq!(
            (up.image.as_str(), up.rung.as_str()),
            ("alpine:3.20", "machine")
        );
        let down = view(&r, false);
        assert_eq!(
            (down.state, down.exit_code, down.pid),
            (State::Exited, Some(4), None)
        );
        for (ended, code) in [
            ("the guest ended, exit code 0", Some(0)),
            ("the guest ended, exit code 143", Some(143)),
            (KILLED, Some(137)),
            ("stopped by podvm stop (SIGKILL)", None),
            ("", None),
        ] {
            assert_eq!(code_of(ended), code, "{ended:?}");
        }
        let old = Record {
            name: "old".into(),
            ..Default::default()
        };
        assert_eq!(view(&old, false).id, "old");
    }
}
