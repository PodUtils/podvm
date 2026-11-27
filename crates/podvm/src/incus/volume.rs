//! `podvm volume rm --podvm-tier=incus HOST...`, TODO/machine.md T-2103
//! (PodUtils/podrunner#371).
//!
//! An instance writes in a writable volume as the host ids of its map
//! ([`super::volmap`]), so the host user cannot remove a directory that a
//! user of the image made with mode 0755 or 0700. The removal keeps the map:
//! for each map that the record of HOST (or of a directory under or above
//! it) holds, podvm creates a short instance with that map
//! (`security.idmap.isolated`, `security.idmap.base`, `security.idmap.size`;
//! incusd takes a set base as it is), no NIC, HOST as its one disk device and
//! podvm as its one file. podvm in the instance removes each entry of the
//! volume that the root of that map can remove (`guest-volume-rm`), and
//! the instance goes. Then podvm, as the host user, removes HOST and what
//! stays in it. No id of the host user goes into a map, and no instance is
//! privileged.
//!
//! Each removal stays under HOST: HOST is a directory of the caller that is
//! not a link, not `/`, not the home directory or a directory above it, and
//! has no mount point at or under it. The walk opens each directory with
//! `O_NOFOLLOW` and removes a link as a link (`pod-extract`'s `remove_at`);
//! the disk device of the instance is not recursive, so it shows no mount
//! under HOST.

use std::path::{Path, PathBuf};

use pod_incus::{Client, Exec, FileWrite, NewInstance, Payload, Source};
use podvm_image::error::{EXIT_FLAG_ERROR, EXIT_RUNTIME_ERROR};

use super::volmap::{self, Map};

const VERB: &str = "volume rm";
const USAGE: &str = "\
usage: podvm volume rm --podvm-tier=incus HOST...

  Remove the host directory HOST of a volume of the incus tier, with what
  the instances wrote in it as the ids of their maps. A short instance with
  each recorded map removes what its root can; podvm then removes the rest
  as the caller. HOST is a directory of the caller, not a link, not /, not
  the home directory (TODO/machine.md T-2103).
";
/// The guest verb of the removal in the instance.
pub const GUEST_VERB: &str = "guest-volume-rm";
/// The path of the volume in the instance of the removal.
const MOUNT: &str = "/podvm-volume";
/// The most bytes of `/proc/self/mountinfo` that the check reads.
const MOUNTINFO_MAX: u64 = 16 << 20;
/// The bound of each silence of incusd: the removal of a large tree is one
/// operation with no answer before its end.
const REMOVAL_BOUND: std::time::Duration = std::time::Duration::from_secs(1800);

fn bad(why: impl std::fmt::Display) -> i32 {
    eprintln!("podvm {VERB}: {why} (TODO/machine.md T-2103)");
    eprint!("{USAGE}");
    EXIT_FLAG_ERROR
}

/// `podvm volume ...`: the group, with `rm` its one subverb.
pub fn group(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("rm" | "remove") => dispatch(&args[1..]),
        Some("-h" | "--help") | None => {
            print!("{USAGE}");
            0
        }
        Some(other) => bad(format!(
            "{other:?} is not a subverb: podvm keeps no named volumes, and `volume rm` \
             of a host directory on the incus tier is the one subverb"
        )),
    }
}

/// The host paths of `args`, or the exit of a usage error; `Ok(None)` for
/// `--help`.
fn parse(args: &[String]) -> Result<Option<Vec<String>>, i32> {
    let (options, operands) = args
        .iter()
        .position(|a| a == "--")
        .map_or((args, &[][..]), |i| (&args[..i], &args[i + 1..]));
    let expanded = crate::parity::expand(VERB, options)
        .map_err(|flag| crate::parity::refuse_member(VERB, &flag, USAGE))?;
    let (mut incus, mut hosts) = (false, Vec::new());
    let mut it = expanded.iter().map(String::as_str);
    while let Some(arg) = it.next() {
        if !arg.starts_with('-') {
            hosts.push(arg.to_string());
            continue;
        }
        crate::parity::admit(VERB, arg, USAGE)?;
        match (arg, arg.strip_prefix("--podvm-tier=")) {
            ("-h" | "--help", _) => return Ok(None),
            (_, Some("incus")) => incus = true,
            ("--podvm-tier", _) if it.next() == Some("incus") => incus = true,
            ("--podvm-tier", _) | (_, Some(_)) => {
                return Err(bad(
                    "--podvm-tier takes incus only: the other tiers write in a \
                     volume as the host user, who removes it with no podvm",
                ))
            }
            _ => return Err(crate::parity::no_arm(VERB, arg)),
        }
    }
    hosts.extend(operands.iter().cloned());
    if !incus {
        return Err(bad("requires --podvm-tier=incus"));
    }
    if hosts.is_empty() {
        return Err(bad("name at least one host directory"));
    }
    Ok(Some(hosts))
}

unsafe extern "C" {
    fn geteuid() -> u32;
}

/// The canonical path of `arg`, or the reason that podvm does not remove
/// it. `home` is the canonical home directory, where there is one.
pub fn check_host(arg: &str, euid: u32, home: Option<&Path>) -> Result<PathBuf, String> {
    use std::os::unix::fs::MetadataExt;
    let md = std::fs::symlink_metadata(arg).map_err(|e| format!("{arg:?}: {e}"))?;
    if md.file_type().is_symlink() {
        return Err(format!("{arg:?} is a link: name the directory itself"));
    }
    if !md.is_dir() {
        return Err(format!("{arg:?} is not a directory"));
    }
    let host = std::fs::canonicalize(arg).map_err(|e| format!("{arg:?}: {e}"))?;
    if host.parent().is_none() {
        return Err("/ is the root of the host".to_string());
    }
    if home.is_some_and(|h| h.starts_with(&host)) {
        return Err(format!(
            "{} is the home directory or a directory above it",
            host.display()
        ));
    }
    if md.uid() != euid {
        return Err(format!(
            "{} belongs to uid {}, and the caller is uid {euid}: podvm removes the \
             caller's own directory only",
            host.display(),
            md.uid()
        ));
    }
    Ok(host)
}

/// The mount points of `mountinfo` (its fifth field, with its octal
/// escapes) at or under `host`.
pub fn mounts_under(mountinfo: &str, host: &Path) -> Vec<PathBuf> {
    mountinfo
        .lines()
        .filter_map(|l| l.split(' ').nth(4))
        .map(unescape)
        .filter(|m| m.starts_with(host))
        .collect()
}

/// A path of `mountinfo`: `\NNN` is the byte of octal NNN.
fn unescape(s: &str) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let oct = b.get(i + 1..i + 4).and_then(|d| {
            let t = std::str::from_utf8(d).ok()?;
            u8::from_str_radix(t, 8).ok()
        });
        match (b[i], oct) {
            (b'\\', Some(v)) => {
                out.push(v);
                i += 4;
            }
            (c, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    PathBuf::from(std::ffi::OsString::from_vec(out))
}

/// The refusal where a mount point is at or under `host`, or where the
/// table of mounts does not read.
fn no_mount_under(host: &Path) -> Result<(), String> {
    use std::io::Read;
    let mut s = String::new();
    std::fs::File::open("/proc/self/mountinfo")
        .and_then(|f| f.take(MOUNTINFO_MAX).read_to_string(&mut s))
        .map_err(|e| format!("/proc/self/mountinfo: {e}: podvm does not remove a tree whose mounts it cannot see"))?;
    match mounts_under(&s, host).first() {
        Some(m) => Err(format!(
            "{} is a mount point at or under {}: podvm removes no other file system",
            m.display(),
            host.display()
        )),
        None => Ok(()),
    }
}

pub(crate) fn dispatch(args: &[String]) -> i32 {
    let hosts = match parse(args) {
        Ok(Some(h)) => h,
        Ok(None) => {
            print!("{USAGE}");
            return 0;
        }
        Err(code) => return code,
    };
    let store = match podvm_image::open_store() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("podvm {VERB}: {e}");
            return e.exit_code();
        }
    };
    // SAFETY: geteuid has no failure.
    let euid = unsafe { geteuid() };
    let home = std::env::var_os("HOME").and_then(|h| std::fs::canonicalize(h).ok());
    let mut code = 0;
    for arg in &hosts {
        let host = match check_host(arg, euid, home.as_deref()).and_then(|h| {
            no_mount_under(&h)?;
            Ok(h)
        }) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("podvm {VERB}: {e} (TODO/machine.md T-2103)");
                code = EXIT_FLAG_ERROR;
                continue;
            }
        };
        match remove(&store, &host) {
            Ok(()) => println!("{}", host.display()),
            Err(e) => {
                eprintln!(
                    "podvm {VERB}: {}: {e} (TODO/machine.md T-2103)",
                    host.display()
                );
                code = EXIT_RUNTIME_ERROR;
            }
        }
    }
    code
}

/// Remove `host`: an instance for each recorded map, then the host user.
fn remove(store: &podvm_image::Store, host: &Path) -> Result<(), String> {
    let records = volmap::related(store.root(), host)?;
    let maps = volmap::maps(&records);
    let mut said = Vec::new();
    if !maps.is_empty() {
        let me = std::fs::read("/proc/self/exe").map_err(|e| format!("/proc/self/exe: {e}"))?;
        let r = super::verbs::block_on(VERB, async {
            match super::verbs::connect_within(REMOVAL_BOUND).await {
                Ok((c, _)) => {
                    for m in &maps {
                        if let Err(e) = in_instance(&c, host, *m, &me, &mut said).await {
                            eprintln!("podvm {VERB}: {}: {e}", host.display());
                            return 1;
                        }
                    }
                    0
                }
                Err(e) => {
                    eprintln!("podvm {VERB}: incus tier: {e}");
                    1
                }
            }
        });
        if r != 0 {
            return Err("the removal in an instance with the map failed; HOST stays".into());
        }
    }
    let parent = host.parent().ok_or("no parent")?;
    let name = host
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("the last name of the path is not UTF-8")?;
    let d =
        podvm_extract::safety::Dir::open(&parent.to_string_lossy()).map_err(|e| e.to_string())?;
    match podvm_extract::remove::remove_at(d.fd(), name) {
        Ok(_) => {
            volmap::forget(store, host, &records);
            Ok(())
        }
        Err(e) if maps.is_empty() => Err(format!(
            "{e}; no run of the incus tier in this store recorded a map for it \
             (PODVM_STORE names the store of the runs)"
        )),
        Err(e) => Err(format!(
            "{e}; the instances with the maps said: {}",
            said.join("; ")
        )),
    }
}

/// One short instance with `map` and `host` at [`MOUNT`]: podvm in it
/// removes what the root of the map can. The instance goes in each case.
async fn in_instance(
    c: &Client,
    host: &Path,
    map: Map,
    me: &[u8],
    said: &mut Vec<String>,
) -> Result<(), String> {
    let nonce = crate::machine::guest::init::nonce()?;
    let name = format!("podvm-volrm-{}", nonce.chars().take(12).collect::<String>());
    let mut new = NewInstance::container(&name, Source::None);
    for (k, v) in [
        ("security.idmap.isolated", "true".to_string()),
        ("security.idmap.base", map.base.to_string()),
        ("security.idmap.size", map.size.to_string()),
        (
            "raw.lxc",
            "lxc.init.cmd = /.podvm/podvm guest-hold".to_string(),
        ),
        (super::verbs::LABEL, "1".to_string()),
        (super::verbs::IMAGE_KEY, "podvm volume rm".to_string()),
    ] {
        new.config.insert(k.to_string(), v);
    }
    let host_s = host.to_str().ok_or("the host path is not UTF-8")?;
    new.devices = super::run::disk_devices(&[(host_s.to_string(), MOUNT.to_string(), false)]);
    new.devices
        .extend(super::net::devices(true, &["eth0".to_string()], None));
    super::net::create(c, &new, true, None)
        .await
        .map_err(|(e, _)| e)?;
    let r = run_removal(c, &name, me).await;
    let left = super::verbs::discard(c, &name).await;
    match (r, left.is_empty()) {
        (Ok(out), true) => {
            said.extend(out);
            Ok(())
        }
        (Ok(_), false) => Err(left),
        (Err(e), _) => Err(super::verbs::with_left(e, &left)),
    }
}

/// Push podvm into the instance `name`, start it, and run the removal:
/// the lines of its stderr.
async fn run_removal(c: &Client, name: &str, me: &[u8]) -> Result<Vec<String>, String> {
    let mut dir = FileWrite::directory();
    (dir.uid, dir.gid, dir.mode) = (Some(0), Some(0), Some(0o755));
    c.put_file(name, "/.podvm", dir)
        .await
        .map_err(|e| e.to_string())?;
    let mut w = FileWrite::file(Payload::bytes(me.to_vec()));
    (w.uid, w.gid, w.mode) = (Some(0), Some(0), Some(0o755));
    c.put_file(name, super::guest_exec::PODVM, w)
        .await
        .map_err(|e| e.to_string())?;
    c.start(name).await.map_err(|e| e.to_string())?;
    let exec = Exec::new([super::guest_exec::PODVM, GUEST_VERB, MOUNT]);
    let out = c
        .exec_recorded(name, &exec)
        .await
        .map_err(|e| e.to_string())?;
    let lines = String::from_utf8_lossy(&out.stderr)
        .lines()
        .take(8)
        .map(str::to_string)
        .collect();
    match out.exit_code {
        0 | 1 => Ok(lines),
        n => Err(format!(
            "podvm {GUEST_VERB} in the instance ended with {n}: {}",
            lines.join("; ")
        )),
    }
}

/// `podvm guest-volume-rm DIR`, in the instance of the removal: remove
/// each entry of DIR that the root of the map can remove. Exit 0 where
/// each went, 1 where one stays (each with one line), 2 where DIR does not
/// open.
pub fn guest_main(args: &[String]) -> i32 {
    let [dir] = args else {
        eprintln!("podvm {GUEST_VERB}: takes one directory");
        return 2;
    };
    let d = match podvm_extract::safety::Dir::open(dir) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("podvm {GUEST_VERB}: {e}");
            return 2;
        }
    };
    let names = match std::fs::read_dir(dir) {
        Ok(it) => it.filter_map(Result::ok).map(|e| e.file_name()),
        Err(e) => {
            eprintln!("podvm {GUEST_VERB}: {dir}: {e}");
            return 2;
        }
    };
    let mut stays = 0;
    for n in names {
        let Some(n) = n.to_str() else {
            eprintln!("podvm {GUEST_VERB}: {n:?}: the name is not UTF-8, and stays");
            stays += 1;
            continue;
        };
        if let Err(e) = podvm_extract::remove::remove_at(d.fd(), n) {
            eprintln!("podvm {GUEST_VERB}: {n}: {e}");
            stays += 1;
        }
    }
    i32::from(stays > 0)
}

#[cfg(test)]
#[path = "volume_tests.rs"]
mod tests;
