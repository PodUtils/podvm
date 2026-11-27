//! TODO/enter.md T-2017: `--ulimit` and `--security-opt no-new-privileges`
//! of a run with the init of the image on the namespace rung.
//!
//! pod-enter sets both in its child before the exec of a payload. This
//! route has no entry of pod-enter: the supervisor sets them in the init
//! before its exec, so that systemd and each service that it starts get
//! them, and in the command of the run, which the supervisor starts
//! beside the init. A limit of a process passes to each of its children,
//! and the no-new-privileges bit cannot be cleared.
//!
//! The confine layer of `--podvm-confine` is not applied here: its seccomp
//! filter denies each mount call and each new namespace, and its Landlock
//! domain refuses each change of the mounts. systemd mounts file systems
//! as PID 1 and opens namespaces for the sandbox of a unit, so the boot
//! fails under the layer. A layer on the command only does not hold: the
//! command can ask the init to start a process with no layer
//! (`systemd-run`). [`super::stays::unserved`] refuses it by name.

use podvm_enter::Limit;

/// The argument of the supervisor for one limit.
pub const ULIMIT: &str = "--ulimit";
/// The argument of the supervisor for the no-new-privileges bit.
pub const NO_NEW_PRIVS: &str = "--no-new-privs";

/// The arguments of the supervisor for `limits` and the bit: each limit as
/// docker's `NAME=SOFT:HARD`, with the numbers of the kernel.
pub fn args(limits: &[Limit], no_new_privs: bool) -> Vec<String> {
    let mut a = Vec::with_capacity(limits.len() * 2 + 1);
    for l in limits {
        a.push(ULIMIT.to_string());
        a.push(format!("{}={}:{}", l.resource.word(), l.soft, l.hard));
    }
    if no_new_privs {
        a.push(NO_NEW_PRIVS.to_string());
    }
    a
}

/// One `--ulimit` of the supervisor, read as `run` reads it.
pub fn limit(v: &str) -> Result<Limit, String> {
    podvm_enter::ulimit(v).map_err(|e| format!("--ulimit {e}"))
}

/// Set each limit and then the bit in this process, before its exec. An
/// error is the step and the errno of the kernel.
pub fn apply(limits: &[Limit], no_new_privs: bool) -> Result<(), String> {
    for l in limits {
        if let Err(e) = pod_hostprobe::sys::set_rlimit(l.resource.number(), l.soft, l.hard) {
            return Err(format!(
                "--ulimit {}: setrlimit({}): {}",
                l.resource.word(),
                l.resource.name(),
                std::io::Error::from_raw_os_error(e.0)
            ));
        }
    }
    if no_new_privs && let Err(e) = pod_hostprobe::sys::set_no_new_privs() {
        return Err(format!(
            "--security-opt no-new-privileges: prctl(PR_SET_NO_NEW_PRIVS): {}",
            std::io::Error::from_raw_os_error(e.0)
        ));
    }
    Ok(())
}

/// [`apply`] in the `pre_exec` of a command: system calls only, and the
/// errno as the error.
pub fn apply_raw(limits: &[Limit], no_new_privs: bool) -> std::io::Result<()> {
    for l in limits {
        pod_hostprobe::sys::set_rlimit(l.resource.number(), l.soft, l.hard)
            .map_err(|e| std::io::Error::from_raw_os_error(e.0))?;
    }
    if no_new_privs {
        pod_hostprobe::sys::set_no_new_privs()
            .map_err(|e| std::io::Error::from_raw_os_error(e.0))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::CommandExt;

    /// The limits and the bit reach a child before its exec, and a child
    /// with none keeps the values of its parent; a limit over the hard
    /// value of the parent is a refusal with the errno, and no exec.
    #[test]
    fn a_child_gets_the_limits_and_the_bit_before_its_exec() {
        let (soft, hard) =
            pod_hostprobe::sys::prlimit(podvm_enter::Resource::Nofile.number()).unwrap();
        let over = if hard == podvm_enter::ulimit("nofile=-1").unwrap().hard {
            None
        } else {
            Some(format!("nofile={}:{}", hard, hard + 1))
        };
        let rows: Vec<(Vec<String>, bool, Option<String>)> = vec![
            (vec!["nofile=50:60".into()], true, Some("50 60 1".into())),
            (
                vec![],
                false,
                Some(format!("{} {} 0", show(soft), show(hard))),
            ),
            (
                vec!["nofile=40".into(), "core=0".into()],
                false,
                Some("40 40 0".into()),
            ),
            (over.into_iter().collect(), false, None),
        ];
        for (specs, nnp, want) in rows {
            if specs.is_empty() && want.is_none() {
                continue; // the hard value of this host is infinite
            }
            let limits: Vec<podvm_enter::Limit> =
                specs.iter().map(|s| super::limit(s).unwrap()).collect();
            let mut c = std::process::Command::new("/bin/sh");
            c.args([
                "-c",
                "echo $(ulimit -Sn) $(ulimit -Hn) $(awk '/NoNewPrivs/{print $2}' /proc/self/status)",
            ]);
            let l = limits.clone();
            // SAFETY: the closure makes system calls only.
            unsafe { c.pre_exec(move || super::apply_raw(&l, nnp)) };
            match (c.output(), want) {
                (Ok(o), Some(w)) => {
                    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), w, "{specs:?}")
                }
                (Err(e), None) => assert_eq!(e.raw_os_error(), Some(1), "{specs:?}"),
                (got, w) => panic!("{specs:?}: want {w:?}, got {got:?}"),
            }
        }
    }

    /// The word of `ulimit` for a value: `unlimited` for no limit.
    fn show(v: u64) -> String {
        if v == podvm_enter::ulimit("nofile=-1").unwrap().hard {
            "unlimited".into()
        } else {
            v.to_string()
        }
    }
}
