//! PodUtils/podrunner#313 ask 1 (TODO/machine.md T-1993): the init of the
//! image (systemd) as PID 1 of an incus instance that stays.
//!
//! With `--podvm-init=image` the instance has no `lxc.init.cmd`, so the
//! init of the image boots. podvm then waits for `systemctl
//! is-system-running --wait` by the exec API, again until PID 1 has its
//! bus, within [`BOOT_BOUND`], and
//! starts `guest-hold --run --detach` by the exec API as one more process:
//! it runs the command of `/.podvm/cmd` with its log and its exit code as
//! with podvm's own init, relays the channel, and asks PID 1 to power the
//! instance off when the command ends.

use std::time::Duration;

use pod_incus::{Client, Exec};

/// The bound of the boot of the init of the image.
pub const BOOT_BOUND: Duration = Duration::from_secs(180);

/// What podvm does after one `systemctl is-system-running --wait`.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    /// The init is up: start the command.
    Run,
    /// The init is not up yet: ask again. Before PID 1 opens its bus,
    /// systemctl gives no state and "Failed to connect to bus" (experiment
    /// 434 clause 19), and `--wait` does not wait for the bus.
    Again,
    /// The init stays down: the error.
    Fail,
}

fn step(state: &str, stderr: &str) -> Step {
    match state {
        "running" | "degraded" => Step::Run,
        "initializing" | "starting" => Step::Again,
        "" if stderr.contains("Failed to connect to bus") => Step::Again,
        _ => Step::Fail,
    }
}

/// Wait for the init of the image, then start the command. An error names
/// what happened: the state of systemd, or the bound and the name of PID 1.
pub async fn boot_then_run(c: &Client, name: &str) -> Result<(), String> {
    let end = tokio::time::Instant::now() + BOOT_BOUND;
    let wait = Exec::new(["systemctl", "is-system-running", "--wait"]);
    let mut last = String::from("no answer");
    loop {
        let left = end.saturating_duration_since(tokio::time::Instant::now());
        let out = match tokio::time::timeout(left, c.exec_recorded(name, &wait)).await {
            Err(_) => None,
            Ok(r) => Some(
                r.map_err(|e| format!("--podvm-init=image: systemctl is-system-running: {e}"))?,
            ),
        };
        if let Some(out) = out {
            let state = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            last = format!(
                "systemctl is-system-running gives {state:?} (exit {}){}",
                out.exit_code,
                if err.is_empty() {
                    String::new()
                } else {
                    format!(": {err}")
                }
            );
            match step(&state, &err) {
                Step::Run => break,
                Step::Fail => {
                    return Err(format!(
                        "--podvm-init=image: the init of the image is not up: {last}"
                    ))
                }
                Step::Again => {}
            }
        }
        if tokio::time::Instant::now() >= end {
            return Err(format!(
                "--podvm-init=image: the init of the image did not finish its boot in {} s: {last}; \
                 PID 1 is {}; an image with no systemd has no such init",
                BOOT_BOUND.as_secs(),
                pid1(c, name).await
            ));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    let run = Exec::new(["/.podvm/podvm", "guest-hold", "--run", "--detach"]);
    let r = c
        .exec_recorded(name, &run)
        .await
        .map_err(|e| format!("--podvm-init=image: the start of the command: {e}"))?;
    if r.exit_code != 0 {
        return Err(format!(
            "--podvm-init=image: the start of the command gave exit {}: {}",
            r.exit_code,
            String::from_utf8_lossy(&r.stderr).trim()
        ));
    }
    Ok(())
}

/// The name of PID 1 of the instance, for the error of the bound.
async fn pid1(c: &Client, name: &str) -> String {
    let comm = Exec::new(["cat", "/proc/1/comm"]);
    match c.exec_recorded(name, &comm).await {
        Ok(o) if o.exit_code == 0 => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Ok(o) => format!("- (cat gives exit {})", o.exit_code),
        Err(e) => format!("- ({e})"),
    }
}

#[cfg(test)]
mod tests {
    use super::{step, Step};

    #[test]
    fn the_state_of_systemd_gives_the_step() {
        let bus = "Failed to connect to bus: No such file or directory";
        for (state, err, want) in [
            ("running", "", Step::Run),
            ("degraded", "", Step::Run),
            ("starting", "", Step::Again),
            ("initializing", "", Step::Again),
            ("", bus, Step::Again),
            ("", "", Step::Fail),
            ("maintenance", "", Step::Fail),
            ("offline", "", Step::Fail),
            ("stopping", "", Step::Fail),
        ] {
            assert_eq!(step(state, err), want, "{state:?} {err:?}");
        }
    }
}
