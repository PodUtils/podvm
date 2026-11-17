//! TODO/enter.md T-1657. The syscall tier: `run --podvm-tier=syscall` runs
//! a payload on the native-static rung (T-1652; a dynamic one with the
//! image's interpreter, T-1731), and the handler
//! answers more than the paths: `uname` gives a fixed release that names
//! the tier, `/tmp` is a directory of the run with a size cap, and the
//! links of `/proc/self` name guest paths. A `-v HOST:GUEST` volume writes
//! through to the host.
//!
//! It is the gVisor view without a user namespace, mount or ptrace. The
//! handler shares the payload's address space, so the tier is a
//! compatibility layer and not a boundary, and the banner says so.

use super::*;

/// The rung word of the syscall tier.
pub const WORD: &str = "syscall";

/// The cap of `/tmp` where `--tmpfs /tmp:size=` gives none: the default of
/// issue 169's recipes (`tmp 256m`).
pub const TMP_CAP: u64 = 256 << 20;

/// The entry of the syscall tier, or a refusal by name (exit 125).
pub fn decide(
    verb: &str,
    maps: &podvm_enter::userland::Maps,
    argv: &[String],
    working_dir: &str,
    env: &[String],
    cap: u64,
) -> Result<Userland, i32> {
    let refuse = |why: String| {
        eprintln!("podvm {verb}: --podvm-tier=syscall: {why} (TODO/enter.md T-1657)");
        Err(podvm_image::error::EXIT_RUNTIME_ERROR)
    };
    // TODO/gate.md T-1948, PodUtils/podvm#508: the tier takes the interposer
    // out (T-1731), and the handler answers no `getuid`, so a `-u` that the
    // interposer fakes would be a claim with nothing behind it.
    if env.iter().any(|e| {
        e.split_once('=')
            .is_some_and(|(k, v)| k == crate::interpose::IDENTITY_VAR && !v.is_empty())
    }) {
        return refuse(
            "-u is refused: the payload runs as the caller on this tier, and the tier \
             fakes no identity; run without -u, or with no --podvm-tier"
                .to_string(),
        );
    }
    if let Err(why) = podvm_dispatch::available() {
        return refuse(format!(
            "the tier runs on the native-static rung, and {why}"
        ));
    }
    let path_dirs = podvm_enter::Plan::path_from(env);
    let f = match script::follow(maps, argv, &path_dirs, working_dir) {
        Ok(f) => f,
        Err((code, note, _)) => {
            eprintln!("podvm {verb}: --podvm-tier=syscall: {note} (TODO/enter.md T-1657)");
            return Err(code);
        }
    };
    // TODO/enter.md T-1731: a dynamic program brings its interpreter from
    // the image, which the loader maps beside it.
    if let Err(why) = podvm_dispatch::host_program(std::path::Path::new(&f.resolved)) {
        return refuse(format!(
            "the tier runs an x86_64 program of the image, and {}: {why}. \
             --podvm-tier=emulate runs a program of another machine",
            f.resolved
        ));
    }
    let mut u = native::decide(&f, maps, "").map_err(|why| {
        eprintln!("podvm {verb}: --podvm-tier=syscall: {why} (TODO/enter.md T-1657)");
        podvm_image::error::EXIT_RUNTIME_ERROR
    })?;
    u.syscall = true;
    u.banner = format!(
        "entering on the syscall tier: the payload (and the image's interpreter of a \
         dynamic one) runs in a podvm process of its own, and Syscall User Dispatch gives each of its system calls to podvm's \
         handler, which resolves each path in the image with \
         openat2(RESOLVE_IN_ROOT), answers uname with {}, gives /tmp as a directory \
         of the run capped at {} MiB, and names guest paths in /proc/self. The handler \
         shares the payload's address space: it is a compatibility layer, not a \
         boundary (TODO/enter.md T-1657)",
        podvm_dispatch::TIER_RELEASE,
        cap >> 20
    );
    Ok(u)
}

/// The variables of the tier in the payload's environment: the tier's own
/// (the cap of /tmp, and the port of the broker that holds the payload),
/// and the proxy variables that name the broker (TODO/net.md T-1665).
pub fn tier_env(
    env: &mut Vec<String>,
    syscall: bool,
    cap: u64,
    broker: Option<&crate::broker::Started>,
) {
    let tier = podvm_dispatch::TIER_VAR;
    let names = podvm_dispatch::BROKER_VAR;
    env.retain(|e| {
        let k = e.split('=').next().unwrap_or("");
        k != tier && k != names
    });
    if let Some(b) = broker {
        crate::broker::replace_proxy_env(env, b.env("127.0.0.1", b.port));
    }
    if syscall {
        match broker {
            Some(b) => {
                env.push(format!("{tier}={cap}:-:{}", b.port));
                env.push(b.dispatch_env());
            }
            None => env.push(format!("{tier}={cap}")),
        }
    }
}

/// TODO/enter.md T-1935: the cap of `/tmp` of a container whose run took
/// the syscall tier, from the variable of the tier in its record; `None`
/// for each other container.
pub fn cap_of(record_env: &[String]) -> Option<u64> {
    let v = record_env
        .iter()
        .find_map(|e| e.strip_prefix(podvm_dispatch::TIER_VAR)?.strip_prefix('='))?;
    v.split(':').next()?.parse().ok()
}

/// TODO/enter.md T-1935: the entry of `exec`. In a container whose run took
/// the syscall tier, the command enters the same tier (a raw system call of
/// a dynamic program sees the image there, not the host); else the shared
/// entry decision of `run`.
#[allow(clippy::too_many_arguments)]
pub fn exec_entry(
    record_env: &[String],
    maps: &podvm_enter::userland::Maps,
    argv: &[String],
    working_dir: &str,
    env: &mut Vec<String>,
    support_native: bool,
    findings: &podvm_probe::Findings,
    unsafe_host_paths: bool,
) -> Result<Option<Userland>, i32> {
    if let Some(cap) = cap_of(record_env) {
        tier_env(env, true, cap, None);
        return decide("exec", maps, argv, working_dir, env, cap).map(Some);
    }
    super::decide_entry(
        "exec",
        maps,
        argv,
        working_dir,
        env,
        support_native,
        findings,
        unsafe_host_paths,
        None,
    )
}

#[cfg(test)]
#[test]
fn the_cap_comes_from_the_record_of_the_tier() {
    let v = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(
        cap_of(&v(&["A=1", "PODVM_DISPATCH_TIER=268435456"])),
        Some(268435456)
    );
    assert_eq!(cap_of(&v(&["PODVM_DISPATCH_TIER=1024:-:3128"])), Some(1024));
    assert_eq!(cap_of(&v(&["PODVM_DISPATCH_TIERX=1"])), None);
    assert_eq!(cap_of(&v(&["A=1"])), None);
}
