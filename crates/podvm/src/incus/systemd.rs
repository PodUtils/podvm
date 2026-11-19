//! PodUtils/podrunner#313 ask 1 (TODO/machine.md T-1993): the init of the
//! image (systemd) as PID 1 of an incus instance that stays.
//!
//! With `--podvm-init=image` the instance has no `lxc.init.cmd`, so the
//! init of the image boots. podvm then waits for `systemctl
//! is-system-running --wait` by the exec API, within [`BOOT_BOUND`], and
//! starts `guest-hold --run --detach` by the exec API as one more process:
//! it runs the command of `/.podvm/cmd` with its log and its exit code as
//! with podvm's own init, relays the channel, and asks PID 1 to power the
//! instance off when the command ends.

use std::time::Duration;

use pod_incus::{Client, Exec};

/// The bound of the boot of the init of the image.
pub const BOOT_BOUND: Duration = Duration::from_secs(180);

/// The states of `systemctl is-system-running` that let the command run.
fn ready(state: &str) -> bool {
    matches!(state, "running" | "degraded")
}

/// Wait for the init of the image, then start the command. An error names
/// what happened: the state of systemd, or the bound.
pub async fn boot_then_run(c: &Client, name: &str) -> Result<(), String> {
    let wait = Exec::new(["systemctl", "is-system-running", "--wait"]);
    let out = tokio::time::timeout(BOOT_BOUND, c.exec_recorded(name, &wait))
        .await
        .map_err(|_| {
            format!(
                "--podvm-init=image: the init of the image did not finish its boot in {} s",
                BOOT_BOUND.as_secs()
            )
        })?
        .map_err(|e| format!("--podvm-init=image: systemctl is-system-running: {e}"))?;
    let state = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !ready(&state) {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!(
            "--podvm-init=image: the init of the image is not up: systemctl \
             is-system-running gives {:?} (exit {}){}; an image with no systemd \
             has no such init",
            state,
            out.exit_code,
            if err.is_empty() {
                String::new()
            } else {
                format!(": {err}")
            }
        ));
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

#[cfg(test)]
mod tests {
    #[test]
    fn running_and_degraded_let_the_command_run() {
        for (s, want) in [
            ("running", true),
            ("degraded", true),
            ("starting", false),
            ("maintenance", false),
            ("offline", false),
            ("", false),
        ] {
            assert_eq!(super::ready(s), want, "{s}");
        }
    }
}
