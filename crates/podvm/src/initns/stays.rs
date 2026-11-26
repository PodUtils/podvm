//! TODO/enter.md T-2002, T-2017: what the caller of `run -d` and `create`
//! with `--podvm-init=image` gets before a byte is fetched: a refusal by
//! name where the host lacks a part of the namespace rung, where the rung
//! of the run is another one, or where the run has a limit that the
//! supervisor of the init does not apply.

use super::detach::delegated;
use super::Host;

unsafe extern "C" {
    fn getuid() -> u32;
}

/// After the parse of `run -d` or `create` with `--podvm-init=image` and
/// no tier: the host must give each part of the namespace rung. No other
/// route keeps an init for a container that stays, so a missing part is a
/// refusal with its name, never a container with podvm's init.
pub fn check(verb: &str, o: &crate::run::Opts) -> Result<(), i32> {
    // TODO/enter.md T-2017: with no tier the init boots on the namespace
    // rung (a tier was set by `route` else), in the foreground too.
    if o.init_image && o.tier.is_none() {
        let asked = [
            o.confine.is_some(),
            !o.hardening.ulimits.is_empty(),
            o.hardening.no_new_privs,
            !o.devices.is_empty(),
        ];
        if let Some(flag) = unserved(asked) {
            eprintln!(
                "podvm {verb}: --podvm-init=image: the namespace rung does not apply {flag} to \
                 the init of the image or to its command, and podvm starts no run with a \
                 limit that it dropped (TODO/enter.md T-2017)"
            );
            return Err(podvm_image::error::EXIT_RUNTIME_ERROR);
        }
    }
    if !stays(o.init_image, o.tier.is_some(), o.detach, verb) {
        return Ok(());
    }
    // SAFETY: getuid cannot fail.
    let uid = unsafe { getuid() };
    let off = std::env::var(crate::userns::VAR).is_ok_and(|v| v == "0");
    let userns = uid != 0 && !off;
    let range = crate::userns::range_available();
    let host = Host {
        userns,
        range,
        cgroup: delegated(userns && range),
    };
    refused(verb, super::refusal(&host))
}

/// The first flag of `asked` (the confine layer, a `--ulimit`, the
/// no-new-privileges bit, a `--device`) that a run with the init of the
/// image on the namespace rung does not apply: its supervisor starts the
/// init and the command with no entry of pod-enter, which holds them
/// (measured 2026-11-20: each was dropped with no line).
pub fn unserved(asked: [bool; 4]) -> Option<&'static str> {
    const FLAGS: [&str; 4] = [
        "--podvm-confine",
        "--ulimit",
        "--security-opt no-new-privileges",
        "--device",
    ];
    asked.iter().position(|a| *a).map(|i| FLAGS[i])
}

/// True for a container that stays with the init of its image and no
/// tier: `run -d` and `create`.
pub fn stays(init_image: bool, tier: bool, detach: bool, verb: &str) -> bool {
    init_image && !tier && (detach || verb == "create")
}

/// Why the rung `rung` cannot keep the init of a container that stays, or
/// none: only the namespace rung has a PID namespace for it.
pub fn wrong_rung(stays: bool, rung: &str) -> Option<String> {
    (stays && rung != "namespace").then(|| {
        format!("the host gives this container the {rung} rung, which has no PID 1 of its own")
    })
}

/// After the selection of the rung: only the namespace rung holds the init.
pub fn check_rung(verb: &str, o: &crate::run::Opts, rung: &str) -> Result<(), i32> {
    let stays = stays(o.init_image, o.tier.is_some(), o.detach, verb);
    refused(verb, wrong_rung(stays, rung))
}

/// TODO/cli.md T-2087 (OR-B40-06): `--init` holds where the run has a PID
/// namespace, whose PID 1 is podvm's init; elsewhere it is refused by name.
pub fn check_init(verb: &str, init: bool, pid: bool, rung: &str) -> Result<(), i32> {
    if !init || pid {
        return Ok(());
    }
    eprintln!(
        "podvm {verb}: --init needs a PID namespace for its init, and the {rung} rung of this \
         host opens none; the launcher (or podvm, for a foreground run) is the child subreaper \
         of the payload's tree and kills each descendant at the end (TODO/supervise.md T-1708)"
    );
    Err(podvm_image::error::EXIT_RUNTIME_ERROR)
}

fn refused(verb: &str, why: Option<String>) -> Result<(), i32> {
    let Some(why) = why else {
        return Ok(());
    };
    eprintln!(
        "podvm {verb}: --podvm-init=image: {why}; a container that stays boots the init of \
         its image on the namespace rung, or with --podvm-tier=incus (TODO/enter.md T-2002)"
    );
    Err(podvm_image::error::EXIT_RUNTIME_ERROR)
}
