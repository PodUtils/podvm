//! TODO/enter.md T-2054 item 1: a foreground entry of the namespace rung
//! where podvm opened no PID namespace of its own (uid 0, or
//! `PODVM_USERNS=0`) takes pod-enter's (`Plan::pid_namespace`), as `run -d`
//! does since T-1930. The payload then sees only the processes of its run,
//! not those of the host or the argv of podvm; the init of pod-enter
//! (`podvm-init`) reaps each orphan at once; and the end of the init ends
//! each process of the namespace, so the payload gets no descriptor of the
//! image lock (T-0204).

use super::*;

/// The words of pod-enter's refusal of a PID namespace on an entry that
/// has none: its fallback to chroot (`namespace_entry`) and `ns_pid::refused`.
pub(super) const REFUSAL: &str = "the PID namespace needs the namespace rung";

/// True where the foreground entry of `p` takes pod-enter's PID namespace:
/// the namespace rung, no forced launch rung, no loader family, no init of
/// the image (T-2002, its own PID namespace), and no PID namespace of
/// podvm's own (T-1909).
pub(super) fn wanted(p: &crate::lifecycle::Prepared) -> bool {
    p.rung == podvm_probe::select::Rung::Namespace.word()
        && p.ladder.is_none()
        && p.userland.is_none()
        && !crate::userns::active()
        && !crate::initns::takes(&p.env)
}

/// True for pod-enter's refusal of the PID namespace of a plan.
pub(super) fn refused(e: &podvm_enter::Error) -> bool {
    e.to_string().contains(REFUSAL)
}

/// The entry of the namespace rung. Where pod-enter falls back to chroot
/// (an image with no `/tmp`, a mount that the host refused after the
/// probe), it refuses a plan with a PID namespace: the run then hands the
/// image lock to the payload and enters again with none, as before
/// T-2054, and pod-enter's fallback line names the cause.
pub(super) fn spawn(
    root: &RootDir,
    plan: &mut Plan,
    held: &podvm_image::store::Lock,
    err: &mut dyn Write,
) -> podvm_enter::Result<podvm_enter::Child> {
    match podvm_enter::spawn_namespace(root, plan, err) {
        Err(e) if plan.pid_namespace && refused(&e) => {
            plan.pid_namespace = false;
            plan.relay_title = None;
            held.hand_to_payload().map_err(|h| {
                podvm_enter::runtime(format!("the entry with no PID namespace: {h}"))
            })?;
            podvm_enter::spawn_namespace(root, plan, err)
        }
        r => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The second path of [`spawn`] reads the words of pod-enter's refusal:
    /// a root with no `/tmp` falls back to chroot, which refuses a plan
    /// with a PID namespace before any fork. Each other error is not it.
    #[test]
    fn the_refusal_of_a_pid_namespace_is_known_by_its_words() {
        let dir = std::env::temp_dir().join(format!("podvm-pidns-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let root = RootDir::open(dir.to_str().unwrap()).unwrap();
        let plan = Plan {
            argv: vec!["true".to_string()],
            env: Vec::new(),
            working_dir: "/".to_string(),
            fds: podvm_enter::Fds::default(),
            banner: String::new(),
            path_dirs: vec!["/bin".to_string()],
            names: podvm_enter::NAMES,
            limits: Vec::new(),
            no_new_privs: false,
            die_with_parent: false,
            confine: None,
            hostname: None,
            volumes: Vec::new(),
            userns_ids: None,
            pid_namespace: true,
            relay_title: None,
        };
        let mut out = Vec::new();
        let e = podvm_enter::spawn_namespace(&root, &plan, &mut out).unwrap_err();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(refused(&e), "{e}");
        assert!(out.is_empty(), "{}", String::from_utf8_lossy(&out));
        for other in [
            podvm_enter::runtime("the PID namespace of the run"),
            podvm_enter::Error::NotFound("sh".into()),
        ] {
            assert!(!refused(&other), "{other}");
        }
    }
}
