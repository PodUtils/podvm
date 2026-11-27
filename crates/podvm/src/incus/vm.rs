//! `--podvm-tier=incus --podvm-vm`: an OCI image with a kernel runs as a
//! virtual machine of incusd (TODO/machine.md T-1998,
//! PodUtils/podrunner#316). incus runs an OCI image as a container only
//! (lxc/incus#1360); the hosted snapshot needs a kernel of its own and
//! `losetup`.
//!
//! 1. The image ([`build`]): once for each digest and build of podvm, an
//!    ext4 root disk of the tree of the image (the writer of the machine
//!    tier, T-1994) with [`SPARE`] free bytes, the files of
//!    [`agent`], and the label of the root of the image's `/etc/fstab`; as
//!    qcow2 ([`qcow2`]) in a split VM image under the alias [`alias`]. The
//!    kernel and the initrd of the image (the search of the machine tier,
//!    `kernel_image`) stay in the store beside it. A second run of the
//!    same digest takes the alias and the files.
//! 2. The boot: a direct boot of that kernel and initrd. `raw.qemu` gives
//!    QEMU `-kernel`, `-initrd` and `-append` ([`raw_qemu`]), and incusd
//!    then starts the VM with no UEFI firmware; `raw.apparmor` lets QEMU
//!    read the two files ([`raw_apparmor`]). The kernel line has `fstab=no`:
//!    the disks of the image's fstab (an ESP, a resource disk) are the
//!    host's, not the VM's.
//! 3. The run: as T-1993 runs a container with `--podvm-init=image`. podvm
//!    waits for systemd through the agent, pushes the command, and starts
//!    `guest-hold --run --detach --linger` ([`LINGER_S`]): the VM stays up
//!    after the command until `wait` read the exit code, which incusd reads
//!    only from a running VM.
//!
//! An incusd with no VM driver, and an image with no kernel, no initrd or
//! no driver of the VM, are refusals by name with 125.

pub mod agent;
pub mod build;
pub mod qcow2;
#[cfg(test)]
mod tests;

use pod_incus::{Client, FileWrite, Kind, NewInstance, Payload, Source};
use podvm_image::error::EXIT_RUNTIME_ERROR;

use super::run::{job_devices, Job};
use super::verbs::{discard, with_left, Detach, IMAGE_KEY, LABEL};
use crate::machine::guest::kernel_image::Group;

/// The free bytes of the root disk past the tree of the image.
pub const SPARE: u64 = 8 << 30;
/// The seconds that a VM stays up after its command for `wait`.
pub const LINGER_S: u64 = 120;
/// The label of the root where the fstab of the image names none.
pub const DEFAULT_LABEL: &str = "podvm-root";
/// The keys of `--memory` and `--cpus` that a VM holds; incusd refuses
/// `limits.processes` and `limits.cpu.allowance` on a VM.
pub const VM_LIMITS: [&str; 2] = ["limits.memory", "limits.cpu"];

/// The drivers of the root disk of an incus VM: incusd gives it on
/// virtio-scsi.
pub const ROOT: Group = Group {
    what: "the root disk of an incus VM (virtio-scsi, ext4)",
    need: &["virtio_pci", "virtio_scsi", "ext4"],
    optional: &[],
};
/// The drivers of incus-agent: its virtio port, vsock and the 9p drive.
pub const AGENT: Group = Group {
    what: "incus-agent (a virtio port, vsock and the 9p drive of incusd)",
    need: &[
        "virtio_console",
        "vmw_vsock_virtio_transport",
        "9pnet_virtio",
        "9p",
    ],
    optional: &[],
};

/// What a run asks of the VM mode, as the parser reads it.
#[derive(Debug, Clone, Default)]
pub struct Ask<'a> {
    pub vm: bool,
    /// `--podvm-tier=incus`.
    pub incus: bool,
    pub init_image: bool,
    /// `run -d` or `create`.
    pub stays: bool,
    pub engine: bool,
    pub channel: bool,
    /// The flag and the incus key of each limit.
    pub limits: Vec<(&'a str, &'a str)>,
}

/// Why the flags of `a` cannot run as a VM, or `None`.
pub fn refusal(a: &Ask) -> Option<String> {
    if !a.vm {
        return None;
    }
    let why = if !a.incus {
        "takes --podvm-tier=incus".to_string()
    } else if !a.init_image {
        "boots the init of the image: give --podvm-init=image".to_string()
    } else if !a.stays {
        "holds on run -d and create".to_string()
    } else if a.engine {
        "does not take --podvm-engine, whose keys are those of a container".to_string()
    } else if a.channel {
        "does not take --podvm-channel, whose socket is a mount of a container".to_string()
    } else if let Some((flag, key)) = a.limits.iter().find(|(_, k)| !VM_LIMITS.contains(k)) {
        format!("does not take {flag}: incusd has no {key} for a VM")
    } else {
        return None;
    };
    Some(format!("--podvm-vm {why} (TODO/machine.md T-1998)"))
}

/// The refusal of an incusd whose `environment.driver` (`lxc | qemu`)
/// has no `qemu`, or `None`.
pub fn no_vm_driver(driver: &str) -> Option<String> {
    if driver.split('|').any(|d| d.trim() == "qemu") {
        return None;
    }
    Some(format!(
        "--podvm-vm: incusd has no VM support (its drivers: {:?}): it needs /dev/kvm, \
         /dev/vsock and QEMU (TODO/machine.md T-1998)",
        driver.trim()
    ))
}

/// The label of the root of the image: the `LABEL=` of the line of `/` in
/// its fstab where it is 1 to 16 plain bytes, else [`DEFAULT_LABEL`].
pub fn root_label(fstab: &str) -> String {
    for line in fstab.lines() {
        let mut w = line.split_whitespace();
        let (Some(src), Some("/")) = (w.next(), w.next()) else {
            continue;
        };
        if let Some(l) = src.strip_prefix("LABEL=")
            && (1..=16).contains(&l.len())
            && l.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        {
            return l.to_string();
        }
    }
    DEFAULT_LABEL.to_string()
}

/// A path of the host that goes into `raw.qemu` and `raw.apparmor`: UTF-8
/// with no character that their quotes or AppArmor's globs would read.
pub fn host_path(p: &std::path::Path) -> Result<String, String> {
    let s = p
        .to_str()
        .ok_or_else(|| format!("{}: not UTF-8", p.display()))?;
    match s
        .chars()
        .find(|c| c.is_control() || "'\"\\*?[]{}^,".contains(*c))
    {
        Some(c) => Err(format!(
            "{s}: QEMU and AppArmor of incusd read this path of the store, and {c:?} is \
             not a plain character of a path; set PODVM_STORE to a plain path"
        )),
        None if !s.starts_with('/') => Err(format!("{s}: not an absolute path")),
        None => Ok(s.to_string()),
    }
}

/// The kernel line of the VM.
pub fn kernel_line(label: &str) -> String {
    format!("root=LABEL={label} rw console=ttyS0 fsck.mode=skip fstab=no")
}

/// `raw.qemu` of the direct boot of `kernel` and `initrd` (plain paths of
/// [`host_path`]).
pub fn raw_qemu(kernel: &str, initrd: &str, label: &str) -> String {
    format!(
        "-kernel '{kernel}' -initrd '{initrd}' -append '{}'",
        kernel_line(label)
    )
}

/// `raw.apparmor`: QEMU reads the two files.
pub fn raw_apparmor(kernel: &str, initrd: &str) -> String {
    format!("\"{kernel}\" r,\n\"{initrd}\" r,")
}

/// The size of the root device of a disk of `bytes`: whole GiB, so that
/// the volume of incusd holds the disk.
pub fn root_size(bytes: u64) -> u64 {
    bytes.div_ceil(1 << 30) << 30
}

/// The alias of the VM image of `digest` and a build of podvm (`me`, the
/// hex SHA-256 of podvm).
pub fn alias(digest: &str, me: &str) -> String {
    let hex = digest.trim_start_matches("sha256:");
    format!(
        "podvm-vm-{}-{}-v{}",
        hex.chars().take(16).collect::<String>(),
        me.chars().take(16).collect::<String>(),
        build::FORMAT
    )
}

/// The instance of a VM of `img` for `job` and `d`, named `name`.
pub fn instance(
    job: &Job<'_>,
    d: &Detach,
    img: &build::VmImage,
    name: &str,
) -> Result<NewInstance, String> {
    let (k, i) = (host_path(&img.kernel)?, host_path(&img.initrd)?);
    let mut new = NewInstance::container(name, Source::Alias(img.alias.clone()));
    new.kind = Kind::VirtualMachine;
    new.devices = job_devices(job);
    new.ephemeral = d.rm;
    for (key, v) in &job.limits {
        new.config.insert(key.to_string(), v.clone());
    }
    let config = [
        ("raw.qemu", raw_qemu(&k, &i, &img.label)),
        ("raw.apparmor", raw_apparmor(&k, &i)),
        ("security.secureboot", "false".to_string()),
        (LABEL, "1".to_string()),
        (IMAGE_KEY, d.image.clone()),
    ];
    for (key, v) in config {
        new.config.insert(key.to_string(), v);
    }
    Ok(new)
}

/// The root device: the one of the default profile, with the size of the
/// disk of the image.
async fn root_device(c: &Client, new: &mut NewInstance, bytes: u64) -> Result<(), String> {
    let profile = c
        .profile("default")
        .await
        .map_err(|e| format!("--podvm-vm: the default profile: {e}"))?;
    let Some(mut root) = profile.devices.get("root").cloned() else {
        return Err("--podvm-vm: the default profile of incusd has no `root` disk".into());
    };
    root.insert("size".to_string(), root_size(bytes).to_string());
    new.devices.insert("root".to_string(), root);
    Ok(())
}

/// Make the VM of `job`, boot it, start the command, and print its name.
pub(super) async fn detached(c: &Client, job: &Job<'_>, img: &build::VmImage, d: &Detach) -> i32 {
    let fail = |e: String| {
        eprintln!("podvm {}: incus tier: {e}", job.verb);
        EXIT_RUNTIME_ERROR
    };
    let name = match super::verbs::name_of(job.verb, d) {
        Ok(n) => n,
        Err(code) => return code,
    };
    let mut new = match instance(job, d, img, &name) {
        Ok(n) => n,
        Err(e) => return fail(e),
    };
    if let Err(e) = root_device(c, &mut new, img.disk_bytes).await {
        return fail(e);
    }
    if let Err((e, t)) = super::net::create(c, &new, job.net_none, None).await {
        let hint = super::space::after_failure(c, job.plan().as_ref(), false, t).await;
        return super::run::fail_twice(job.verb, (e, hint));
    }
    super::volmap::note(c, job.store, &name, &job.volumes, job.verb).await; // T-2103
    if !d.start {
        println!("{name}");
        return 0;
    }
    if let Err(e) = c.start(&name).await {
        let left = discard(c, &name).await;
        return fail(with_left(e, &left));
    }
    if let Err(down) = super::systemd::wait_up(c, &name).await {
        return super::verbs::down(job.verb, c, &name, down).await;
    }
    if let Err(e) = push(c, job, d, &name).await {
        let left = discard(c, &name).await;
        return fail(with_left(e, &left));
    }
    let linger = LINGER_S.to_string();
    if let Err(down) = super::systemd::start_hold(c, &name, &["--linger", &linger]).await {
        return super::verbs::down(job.verb, c, &name, down).await;
    }
    println!("{name}");
    0
}

/// Push the command, and the stdin of `-i`, into the running VM.
async fn push(c: &Client, job: &Job<'_>, d: &Detach, name: &str) -> Result<(), String> {
    let cmd = super::Cmd {
        argv: job.argv.clone(),
        env: job.env.clone(),
        cwd: job.workdir.clone(),
    };
    let mut files = vec![(super::CMD, cmd.to_json())];
    if d.stdin
        && let Some(b) = &job.stdin
    {
        files.push((super::STDIN, b.clone()));
    }
    for (path, bytes) in files {
        let mut w = FileWrite::file(Payload::bytes(bytes));
        (w.uid, w.gid, w.mode) = (Some(0), Some(0), Some(0o600));
        c.put_file(name, path, w)
            .await
            .map_err(|e| format!("{path}: {e}"))?;
    }
    Ok(())
}
