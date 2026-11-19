//! TODO/enter.md T-2007: the first process of a sandbox whose kernel is
//! not the host's (gVisor's Sentry on the runsc tier): `podvm guest-init
//! --pid1`. It is PID 1 of the sandbox for a moment: it gives the files of
//! the tree the owners that the image meant (the extraction gave each file
//! to the caller, and the kernel of the sandbox holds each id), masks the
//! units that this kernel cannot serve, and becomes `/sbin/init` of the
//! image by `execve`, with the same pid.

use std::path::Path;

/// The first argument of `guest-init` for this mode.
pub const FLAG: &str = "--pid1";

/// What the mode takes: `--owners FILE` (the sidecar of the image) and
/// `--mask UNIT` (a unit that the init does not start), each optional.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub owners: Option<String>,
    pub masks: Vec<String>,
}

pub fn parse(args: &[String]) -> Result<Plan, String> {
    let mut p = Plan::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value = || it.next().cloned().ok_or(format!("{a} takes a value"));
        match a.as_str() {
            "--owners" => p.owners = Some(value()?),
            "--mask" => {
                let unit = value()?;
                let plain = !unit.is_empty()
                    && unit
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.@:\\".contains(&b));
                if !plain {
                    return Err(format!("--mask takes the name of a unit, not {unit:?}"));
                }
                p.masks.push(unit);
            }
            other => return Err(format!("{other:?} is no argument of {FLAG}")),
        }
    }
    Ok(p)
}

/// Do the steps and become the init. It returns only on an error: 125,
/// with one line.
pub fn main(args: &[String]) -> i32 {
    let fail = |why: String| {
        eprintln!("podvm: --podvm-init=image: {why} (TODO/enter.md T-2007)");
        125
    };
    let plan = match parse(args) {
        Ok(p) => p,
        Err(e) => return fail(e),
    };
    if let Some(file) = &plan.owners {
        // A sidecar that cannot be read changes no owner: each file then
        // belongs to root of the sandbox, and the line says so.
        match std::fs::read_to_string(file) {
            Ok(text) => {
                crate::userns::owners::apply_text(Path::new("/"), &text, u64::from(u32::MAX) - 1);
            }
            Err(e) => eprintln!(
                "podvm: --podvm-init=image: the owners of the image stay with root: {file}: {e}"
            ),
        }
    }
    if !plan.masks.is_empty() {
        let dir = Path::new("/run/systemd/system");
        if let Err(e) = std::fs::create_dir_all(dir) {
            return fail(format!("{}: {e}", dir.display()));
        }
        for unit in &plan.masks {
            let link = dir.join(unit);
            if let Err(e) = std::os::unix::fs::symlink("/dev/null", &link)
                && e.kind() != std::io::ErrorKind::AlreadyExists
            {
                return fail(format!("the mask of {unit}: {e}"));
            }
        }
    }
    super::mounts::machine_id(Path::new("/etc/machine-id"));
    super::join::drop_groups();
    use std::os::unix::process::CommandExt;
    let e = std::process::Command::new(super::INIT)
        .env_clear()
        .env("container", "podvm")
        .env("PATH", super::join::PATH)
        .exec();
    if e.kind() == std::io::ErrorKind::NotFound {
        return fail(format!(
            "the image has no {}: it carries no init",
            super::INIT
        ));
    }
    fail(format!("exec {}: {e}", super::INIT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_step_reads_its_arguments_and_names_a_wrong_one() {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(parse(&[]), Ok(Plan::default()));
        assert_eq!(
            parse(&v(&[
                "--owners",
                "/m",
                "--mask",
                "a.service",
                "--mask",
                "b@c.service"
            ])),
            Ok(Plan {
                owners: Some("/m".into()),
                masks: v(&["a.service", "b@c.service"]),
            })
        );
        for (args, word) in [
            (v(&["--mask", "../etc/passwd"]), "name of a unit"),
            (v(&["--mask", ""]), "name of a unit"),
            (v(&["--mask"]), "takes a value"),
            (v(&["--owners"]), "takes a value"),
            (v(&["--what"]), "no argument"),
        ] {
            let e = parse(&args).unwrap_err();
            assert!(e.contains(word), "{args:?}: {e}");
        }
    }
}
