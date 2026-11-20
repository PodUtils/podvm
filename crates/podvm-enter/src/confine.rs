//! The confine layer of `run --podvm-confine=SPEC` (TODO/enter.md T-1658,
//! issue 170): Landlock, the seccomp deny filter of `pod-confine` and
//! `no_new_privs`, applied by `pod-enter` in the payload's child as the
//! last step before the exec (`Plan.confine`).
//!
//! SPEC is `read=P,P;write=P;exec=P;connect=PORT,PORT`, each part
//! optional, each path a guest path. `read` gives read access, `write`
//! read and write, `exec` read and execute. `connect` holds a TCP connect
//! to the listed ports (Landlock ABI 4); with no `connect`, TCP is free.
//! Each path that no rule names is denied. A path written `?/P` is
//! optional: it gets no rule where the image lacks it (a profile for many
//! images, as `errand`); a path without `?` that the image lacks is a
//! refusal by name.
//!
//! The spec adds the paths that a program needs to start: read and
//! execute of the system directories of the image that exist (`/bin`,
//! `/sbin`, `/usr`, `/lib`, `/lib64`, `/lib32`, and podvm's own `/.podvm`), read of `/proc`, and
//! the device nodes `/dev/null` (read and write), `/dev/zero`,
//! `/dev/random` and `/dev/urandom` (read), and the files of `/etc` that
//! the dynamic loader reads before `main` (`/etc/ld.so.cache` of glibc,
//! `/etc/ld-musl-ARCH.path` of musl; TODO/enter.md T-1764). Each other file
//! of `/etc` is not among them: a profile names each file that it reads.

pub use pod_confine::{Access, Spec};

/// The system directories that a program of the image runs from.
/// `/.podvm` is podvm's own directory of the rootfs (the probe of
/// `probe-inside`).
pub const SYSTEM_EXEC: &[&str] = &[
    "/bin", "/sbin", "/usr", "/lib", "/lib64", "/lib32", "/.podvm",
];
/// The paths that a program reads at its start.
/// Without `/etc/ld.so.cache`, glibc's loader searches only `/lib` and
/// `/usr/lib`, so python of `/usr/local/lib` did not start (T-1764).
pub const SYSTEM_READ: &[&str] = &[
    "/proc",
    "/dev/zero",
    "/dev/random",
    "/dev/urandom",
    "/etc/ld.so.cache",
    "/etc/ld-musl-x86_64.path",
    "/etc/ld-musl-aarch64.path",
];
/// The paths that a program writes at its start.
pub const SYSTEM_WRITE: &[&str] = &["/dev/null"];

/// What `--podvm-confine` asks for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ask {
    pub read: Vec<String>,
    pub write: Vec<String>,
    pub exec: Vec<String>,
    pub connect: Option<Vec<u16>>,
}

/// Parse SPEC. Err names the part that does not parse.
pub fn parse(spec: &str) -> Result<Ask, String> {
    let mut a = Ask::default();
    for part in spec.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let Some((key, value)) = part.split_once('=') else {
            return Err(format!("{part:?} is not KEY=VALUE"));
        };
        let items: Vec<&str> = value
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .collect();
        match key.trim() {
            "read" | "write" | "exec" => {
                let list = match key.trim() {
                    "read" => &mut a.read,
                    "write" => &mut a.write,
                    _ => &mut a.exec,
                };
                for p in items {
                    if !p.trim_start_matches('?').starts_with('/') {
                        // TODO/cli.md T-1829: `,` where the rules part with `;`.
                        let sep = if p.contains('=') {
                            "; the rules part with `;`"
                        } else {
                            ""
                        };
                        return Err(format!("the path {p:?} of {key} is not absolute{sep}"));
                    }
                    list.push(p.to_string());
                }
            }
            "connect" => {
                let mut ports = Vec::new();
                for p in items {
                    match p.parse::<u16>() {
                        Ok(n) if n > 0 => ports.push(n),
                        _ => return Err(format!("the port {p:?} of connect is not 1 to 65535")),
                    }
                }
                a.connect = Some(ports);
            }
            other => {
                return Err(format!("{other:?} is not read, write, exec or connect"));
            }
        }
    }
    Ok(a)
}

/// The SPEC of `ask`, the inverse of [`parse`].
pub fn render(ask: &Ask) -> String {
    let mut parts = Vec::new();
    for (k, v) in [
        ("read", &ask.read),
        ("write", &ask.write),
        ("exec", &ask.exec),
    ] {
        if !v.is_empty() {
            parts.push(format!("{k}={}", v.join(",")));
        }
    }
    if let Some(c) = &ask.connect {
        let ports: Vec<String> = c.iter().map(u16::to_string).collect();
        parts.push(format!("connect={}", ports.join(",")));
    }
    parts.join(";")
}

/// The `pod-confine` spec of `ask`, with guest paths. `exists` tells
/// whether a guest path exists in the image: a system path that the image
/// lacks gets no rule, and a path of the ask always gets one (a missing
/// one is a refusal of `pod-enter` by name).
pub fn spec(ask: &Ask, exists: &dyn Fn(&str) -> bool) -> Spec {
    let mut s = Spec::new();
    let rx = Access::READ | Access::EXECUTE;
    let rw = Access::READ | Access::WRITE;
    for p in SYSTEM_EXEC.iter().filter(|p| exists(p)) {
        s = s.allow(*p, rx);
    }
    for p in SYSTEM_READ.iter().filter(|p| exists(p)) {
        s = s.allow(*p, Access::READ);
    }
    for p in SYSTEM_WRITE.iter().filter(|p| exists(p)) {
        s = s.allow(*p, rw);
    }
    // `?/P` is optional: no rule where the image lacks P.
    let wanted = |p: &String| match p.strip_prefix('?') {
        Some(o) => exists(o).then(|| o.to_string()),
        None => Some(p.clone()),
    };
    for (list, access) in [(&ask.read, Access::READ), (&ask.write, rw), (&ask.exec, rx)] {
        for p in list.iter().filter_map(wanted) {
            s = s.allow(p, access);
        }
    }
    s.connect = ask.connect.clone();
    s
}

/// The Landlock ABI of this kernel, or None where it has none.
pub fn abi() -> Option<u32> {
    pod_confine::probe()
}

/// The lowest ABI of the layer, and the lowest of a TCP rule.
pub const MIN_ABI: u32 = pod_confine::MIN_ABI;
pub const NET_ABI: u32 = pod_confine::NET_ABI;

/// [`spec`] for the image tree at `rootfs` (a host path): a system path
/// gets its rule where the tree has it.
pub fn spec_in(ask: &Ask, rootfs: &std::path::Path) -> Spec {
    let exists =
        |guest: &str| std::fs::symlink_metadata(rootfs.join(guest.trim_start_matches('/'))).is_ok();
    spec(ask, &exists)
}

/// The host paths that a payload of the loader family (no chroot) reads
/// beside its image: the host's `/proc` and device nodes, which it sees
/// as they are.
pub const HOST_READ: &[&str] = &["/proc", "/dev/zero", "/dev/random", "/dev/urandom"];
pub const HOST_WRITE: &[&str] = &["/dev/null"];

/// The host path of the guest path `guest`: under the volume that serves
/// it, else under the rootfs (the table of the interposer, read
/// lexically).
pub fn host_of(maps: &crate::userland::Maps, guest: &str) -> String {
    match maps.volume_at(guest) {
        Some(v) => {
            let rest = guest
                .trim_start_matches('/')
                .split('/')
                .skip(v.guest.split('/').filter(|c| !c.is_empty()).count())
                .collect::<Vec<_>>()
                .join("/");
            if rest.is_empty() {
                v.host.clone()
            } else {
                format!("{}/{rest}", v.host.trim_end_matches('/'))
            }
        }
        None => format!("{}{guest}", maps.rootfs().trim_end_matches('/')),
    }
}

/// How many links one guest path may name before the path is refused: the
/// limit of the kernel's own resolution.
const MAX_LINKS: u32 = 40;

/// The host path of the guest path `guest`, with each link resolved inside
/// the tree that serves it. A link target that starts with `/` resolves
/// from the root of the image (or of the volume), as the interposer
/// resolves it, and `..` stops at that root: the host's own `/bin/sh` is
/// not the `/bin/sh` of the image. `None` where the image lacks the path.
pub fn host_resolved(maps: &crate::userland::Maps, guest: &str) -> Option<String> {
    let (base, mut todo): (String, std::collections::VecDeque<String>) = match maps.volume_at(guest)
    {
        Some(v) => {
            let parts = v.guest.split('/').filter(|c| !c.is_empty()).count();
            let rest = guest
                .trim_start_matches('/')
                .split('/')
                .skip(parts)
                .map(str::to_string)
                .collect();
            (v.host.trim_end_matches('/').to_string(), rest)
        }
        None => {
            let rest = guest
                .trim_start_matches('/')
                .split('/')
                .map(str::to_string)
                .collect();
            (maps.rootfs().trim_end_matches('/').to_string(), rest)
        }
    };
    let mut done: Vec<String> = Vec::new();
    let mut links = 0u32;
    while let Some(c) = todo.pop_front() {
        match c.as_str() {
            "" | "." => continue,
            ".." => {
                done.pop();
                continue;
            }
            _ => done.push(c),
        }
        let host = format!("{}/{}", base, done.join("/"));
        let md = std::fs::symlink_metadata(&host).ok()?;
        if !md.file_type().is_symlink() {
            continue;
        }
        links += 1;
        if links > MAX_LINKS {
            return None;
        }
        let target = std::fs::read_link(&host).ok()?;
        let text = target.to_string_lossy().into_owned();
        done.pop();
        if text.starts_with('/') {
            done.clear();
        }
        let mut next: std::collections::VecDeque<String> =
            text.split('/').map(str::to_string).collect();
        while let Some(c) = todo.pop_front() {
            next.push_back(c);
        }
        todo = next;
    }
    // The root of the tree where the path named no component: the host
    // side of a volume pair can carry a trailing `/`.
    let host = if done.is_empty() {
        base.trim_end_matches('/').to_string()
    } else {
        format!("{}/{}", base.trim_end_matches('/'), done.join("/"))
    };
    std::fs::symlink_metadata(&host).ok()?;
    Some(host)
}

/// TODO/enter.md T-1887 (PodUtils/podvm#364, the patch of Nemo-010).
///
/// The spec of `ask` with host paths, for the loader family: each guest
/// path through `maps` with its links resolved inside the tree, and the
/// host's `/proc` and device nodes.
///
/// TODO/enter.md T-1888: a required path that does not resolve inside the
/// tree that serves it is refused with the guest path that the caller
/// wrote. A link of the image to a path that only the host has took a
/// rule on the host's inode before.
///
/// TODO/enter.md T-2014: a rule on a guest path that holds a volume gives
/// the same access to the host path of the volume, as a rule of the
/// namespace rung holds the mounts under its path (`read=/` reads `/sc` of
/// `-v S:/sc`).
pub fn host_spec(ask: &Ask, maps: &crate::userland::Maps) -> Result<Spec, String> {
    let exists = |guest: &str| host_resolved(maps, guest).is_some();
    let mut s = spec(ask, &exists);
    let under: Vec<(String, Access)> = s
        .paths
        .iter()
        .flat_map(|r| {
            volumes_under(maps, &r.path)
                .filter_map(|v| host_resolved(maps, &v.guest))
                .map(move |h| (h, r.access))
        })
        .collect();
    for r in &mut s.paths {
        r.path = host_resolved(maps, &r.path).ok_or_else(|| {
            let lexical = host_of(maps, &r.path);
            if std::fs::symlink_metadata(&lexical).is_ok() {
                format!(
                    "the path {:?} of the spec is a link of the image that leaves the tree \
                     that serves it, so the image does not hold its target (TODO/enter.md T-1888)",
                    r.path
                )
            } else {
                format!("the path {:?} of the spec is not in the image", r.path)
            }
        })?;
    }
    for p in HOST_READ
        .iter()
        .filter(|p| std::path::Path::new(p).exists())
    {
        s = s.allow(*p, Access::READ);
    }
    for p in HOST_WRITE
        .iter()
        .filter(|p| std::path::Path::new(p).exists())
    {
        s = s.allow(*p, Access::READ | Access::WRITE);
    }
    for (host, access) in under {
        s = s.allow(host, access);
    }
    Ok(s)
}

/// TODO/enter.md T-2014: each volume whose guest path is under the guest
/// path `guest` (at a component boundary, not the path itself), read
/// lexically as the interposer matches: `.` and `..` fold, and no link of
/// the image is followed. A volume at `guest` itself serves the rule
/// already ([`host_resolved`]).
fn volumes_under<'a>(
    maps: &'a crate::userland::Maps,
    guest: &str,
) -> impl Iterator<Item = &'a crate::userland::Volume> {
    let mut g: Vec<&str> = Vec::new();
    for c in guest.split('/').filter(|c| !c.is_empty() && *c != ".") {
        if c == ".." {
            g.pop();
        } else {
            g.push(c);
        }
    }
    let g: Vec<String> = g.into_iter().map(str::to_string).collect();
    maps.volumes().iter().filter(move |v| {
        let p: Vec<&str> = v.guest.split('/').filter(|c| !c.is_empty()).collect();
        p.len() > g.len() && p.iter().zip(&g).all(|(a, b)| a == b)
    })
}

/// Whether a rung can carry the layer: a rung with a chroot, where the
/// paths of the spec are guest paths. Elsewhere `run` refuses by name and
/// never runs unconfined (the Decision of T-1658).
pub fn rung_carries(rung: podvm_probe::select::Rung) -> bool {
    matches!(
        rung,
        podvm_probe::select::Rung::Namespace | podvm_probe::select::Rung::Chroot
    )
}

#[cfg(test)]
mod tests;

/// The program that a dynamic loader runs: the file of the interposer and
/// of the handler of the syscall tier, included as it is (TODO/enter.md
/// T-1984).
#[path = "../../podvm-interpose/src/exec/ldso.rs"]
mod ldso;

/// TODO/enter.md T-2013: the rule `exec=` for the first program of an
/// entry with no chroot, as the kernel would apply it on the namespace
/// rung. Each argv of `argvs` (the caller's command, and for a `#!` script
/// the argv of the kernel, with its interpreter first) names a guest
/// program, resolved through the table of the run (`PATH` for a name with
/// no `/`, the working directory for a relative one). A program outside
/// each execute path is [`pod_enter::Error::CannotInvoke`] (126). Where the
/// program is a dynamic loader, each argument that the loader can take as
/// its program takes the same rule ([`ldso::each_program`]); a program of
/// the loader with no `/` is a refusal, as the loader searches it as a
/// library. A program that does not resolve is left to the spawn, which
/// names it (127). `Ok` with no confinement.
pub fn check_first(
    plan: &pod_enter::Plan,
    maps: &crate::userland::Maps,
    argvs: &[&[String]],
) -> pod_enter::Result<()> {
    if plan.confine.is_none() {
        return Ok(());
    }
    let guest = |p: &str| -> String {
        if p.starts_with('/') {
            p.to_string()
        } else {
            format!("{}/{p}", plan.working_dir.trim_end_matches('/'))
        }
    };
    for argv in argvs {
        let Some(first) = argv.first() else { continue };
        let candidates: Vec<String> = if first.contains('/') {
            vec![guest(first)]
        } else {
            plan.path_dirs
                .iter()
                .map(|d| format!("{}/{first}", d.trim_end_matches('/')))
                .collect()
        };
        let Some(host) = candidates.iter().find_map(|g| host_resolved(maps, g)) else {
            continue;
        };
        let refuse = |g: &str| {
            pod_enter::Error::CannotInvoke(format!(
                "{g}: --podvm-confine allows no exec of it: it is outside each path of \
                 exec= (EACCES)"
            ))
        };
        if pod_enter::check_exec(plan, &host).is_err() {
            return Err(refuse(first));
        }
        let real = std::fs::canonicalize(&host)
            .map(|r| r.to_string_lossy().into_owned())
            .unwrap_or(host);
        if !ldso::is_loader(real.as_bytes()) {
            continue;
        }
        let mut refused: Option<pod_enter::Error> = None;
        let held = ldso::each_program(argv[1..].iter().map(|a| a.as_bytes()), |p| {
            let p = String::from_utf8_lossy(p);
            match host_resolved(maps, &guest(&p)) {
                Some(h) if pod_enter::check_exec(plan, &h).is_ok() => true,
                Some(_) => {
                    refused = Some(pod_enter::Error::CannotInvoke(format!(
                        "{first} {p}: --podvm-confine allows no exec of the program of the \
                         dynamic loader: it is outside each path of exec= (EACCES)"
                    )));
                    false
                }
                None => {
                    refused = Some(pod_enter::Error::CannotInvoke(format!(
                        "{first} {p}: the program of the dynamic loader does not resolve in \
                         the image, so the confinement cannot judge it (EACCES)"
                    )));
                    false
                }
            }
        });
        if !held {
            return Err(refused.unwrap_or_else(|| {
                pod_enter::Error::CannotInvoke(format!(
                    "{first}: the dynamic loader would search its program as a library, \
                     which the confinement cannot judge (EACCES)"
                ))
            }));
        }
    }
    Ok(())
}
