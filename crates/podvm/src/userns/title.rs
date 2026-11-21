//! The command lines of podvm's processes in the PID namespace of a run
//! (TODO/enter.md T-2054, OQ-08 of the sixth outside pass;
//! `podvm_enter::title`).

use std::sync::atomic::{AtomicI32, Ordering};

/// The command line of pid 1 of the PID namespace of a run, and of the
/// init and the relays of pod-enter there (TODO/enter.md T-2054).
pub const TITLE_INIT: &str = "podvm-init";
/// The command line of pid 2, which runs the rest of podvm.
pub const TITLE_RUN: &str = "podvm-run";
/// The errno of a refused title of pid 2, 0 where it took (T-2054).
pub(super) static HIDDEN: AtomicI32 = AtomicI32::new(0);

/// The line of `--verbose` where the payload can read podvm's command line
/// and environment in `/proc/2` (TODO/enter.md T-2054), or `None`.
pub fn shown_line() -> Option<String> {
    let e = HIDDEN.load(Ordering::Relaxed);
    (e != 0).then(|| {
        format!(
            "podvm: the payload can read the command line and the environment of podvm \
             in /proc/2: prctl(PR_SET_MM_MAP) failed with errno {e} (TODO/enter.md T-2054)\n"
        )
    })
}
