//! TODO/enter.md T-2017, clause 4: each flag of `run` that limits the
//! payload (`--ulimit`, no-new-privileges, `--podvm-confine`, `--device`,
//! a volume on a mount of the init) against each route of a rootfs rung:
//! taken, or refused with its line. On the init of the image, "taken" is
//! the round trip through the arguments of the supervisor (`guest-init`),
//! which applies them (`initns::harden`, `initns::mounts::late`). The
//! guest tiers have their own table (`tier_options_tests`); the init of
//! the image never boots on the userland rung (`initns::route` takes a
//! guest tier there), so the userland column is a run with no init.

use super::*;

fn v(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

/// What a route does with a flag: it takes it, or it refuses it with a
/// line that holds the text.
#[derive(Clone, Copy, Debug)]
enum Out {
    Taken,
    Refused(&'static str),
}
use Out::{Refused, Taken};

/// The routes of a rootfs rung.
#[derive(Clone, Copy, Debug)]
enum Route {
    /// pod-enter's entry on the namespace rung, with no init of the image.
    Namespace,
    /// pod-enter's entry on the userland rung, with no init of the image.
    Userland,
    /// `--podvm-init=image` on the namespace rung: the supervisor.
    Init,
}

const RO: &str = "read-only bind does not refuse a write";
const CONFINE: &str = "--podvm-confine is refused";
const WRITE_ONLY: &str = "served by the interposer";

/// Each flag, and what the namespace rung, the userland rung and the init
/// of the image do with it.
const TABLE: &[(&[&str], Out, Out, Out)] = &[
    (&["--ulimit", "nofile=123"], Taken, Taken, Taken),
    (&["--ulimit", "nproc=64:unlimited"], Taken, Taken, Taken),
    (
        &["--security-opt", "no-new-privileges"],
        Taken,
        Taken,
        Taken,
    ),
    (&["--cap-drop", "ALL"], Taken, Taken, Taken),
    (
        &["--podvm-confine", "read=/usr"],
        Taken,
        Taken,
        Refused(CONFINE),
    ),
    (&["--device", "/dev/null:/dev/xnull"], Taken, Taken, Taken),
    (
        &["--device", "/dev/null:/dev/xnull:rw"],
        Taken,
        Taken,
        Taken,
    ),
    (&["--device", "/dev/null"], Taken, Taken, Taken),
    (
        &["--device", "/dev/null:/dev/xnull:r"],
        Refused(RO),
        Taken,
        Refused(RO),
    ),
    (
        &["--device", "/dev/null:/dev/xnull:w"],
        Taken,
        Taken,
        Refused(WRITE_ONLY),
    ),
    (&["-v", "/tmp:/run/podvm-x"], Taken, Taken, Taken),
    (&["-v", "/tmp:/tmp/podvm-x:ro"], Taken, Taken, Taken),
];

/// The route's answer for the options `o`: the refusal of the checks of
/// `prepare` that read the flag, else the round trip of the supervisor.
fn answer(mut o: Opts, route: Route) -> Result<(), String> {
    let init = matches!(route, Route::Init);
    if init && let Some(why) = crate::initns::stays::unserved(o.confine.is_some(), &o.devices) {
        return Err(why);
    }
    devices::fold(&mut o);
    let rung = match route {
        Route::Userland => "userland",
        Route::Namespace | Route::Init => "namespace",
    };
    if let Some(why) = devices::read_only_node(&o, rung) {
        return Err(why);
    }
    if !init {
        return Ok(());
    }
    // The foreground plan of the init route: its limits, its bit, and the
    // volumes that the supervisor binds after the mounts of the init.
    let late: Vec<podvm_enter::VolumeBind> = o
        .volumes
        .iter()
        .filter(|vol| crate::initns::mounts::late(&vol.guest))
        .map(|vol| podvm_enter::VolumeBind {
            host: vol.host.clone(),
            guest: vol.guest.clone(),
            read_only: o.ro_volumes.contains(&vol.guest),
            host_base: None,
        })
        .collect();
    let plan = podvm_enter::Plan {
        argv: v(&["true"]),
        env: Vec::new(),
        working_dir: "/".into(),
        fds: podvm_enter::Fds::default(),
        banner: String::new(),
        path_dirs: Vec::new(),
        names: podvm_enter::NAMES,
        limits: o.hardening.limits(),
        no_new_privs: o.hardening.no_new_privs,
        die_with_parent: true,
        confine: None,
        hostname: None,
        volumes: late.clone(),
        userns_ids: None,
        pid_namespace: false,
        relay_title: None,
        tmpfs: Vec::new(),
    };
    let base = std::path::Path::new("/c");
    let args = crate::initns::host_args("/r", base, None, &[], &plan);
    let spec = crate::initns::parse(&args[1..])?;
    let binds = spec.tmp.unwrap_or_default();
    let carried = spec.limits == plan.limits
        && spec.no_new_privs == plan.no_new_privs
        && late.iter().all(|l| {
            binds
                .iter()
                .any(|b| b.guest == l.guest && b.host == l.host && b.read_only == l.read_only)
        });
    // Each device and each volume of a run reaches the init: on the tree,
    // or after the mounts of the init.
    let on_tree = o.volumes.len() - late.len();
    if !carried || binds.len() + on_tree != o.volumes.len() {
        return Err(format!("the supervisor did not get each flag: {args:?}"));
    }
    Ok(())
}

#[test]
fn each_flag_that_limits_a_run_is_taken_or_refused_on_each_route() {
    for (flags, namespace, userland, init) in TABLE {
        for (route, want) in [
            (Route::Namespace, namespace),
            (Route::Userland, userland),
            (Route::Init, init),
        ] {
            let mut a = flags.to_vec();
            if matches!(route, Route::Init) {
                a.push("--podvm-init=image");
            }
            a.extend_from_slice(&["img", "true"]);
            let o = parse::parse("run", &v(&a)).unwrap_or_else(|c| panic!("{a:?}: parse {c}"));
            // The flag asks for something: a row that parses to nothing
            // would pass each route with no check.
            let asked = !o.hardening.ulimits.is_empty()
                || o.hardening.no_new_privs
                || o.confine.is_some()
                || !o.devices.is_empty()
                || !o.volumes.is_empty();
            assert!(asked, "{flags:?} asks for nothing");
            match (want, answer(o, route)) {
                (Taken, Ok(())) => {}
                (Refused(text), Err(line)) if line.contains(text) => {}
                (w, got) => panic!("{flags:?} on {route:?}: want {w:?}, got {got:?}"),
            }
        }
    }
}
