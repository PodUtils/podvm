//! `run --podvm-tier=machine`: a disposable guest from an OCI image,
//! TODO/machine.md T-1656.
//!
//! The rootfs of the image becomes a `newc` base archive, built once for
//! each image digest and kept in the store; each run appends a small
//! archive with `/podvm-init` and the console nodes. QEMU boots the pinned
//! kernel ([`kernel`]) with that initrd, under KVM where `/dev/kvm` opens
//! and under TCG elsewhere. `PODVM_MACHINE_ACCEL=kvm` asks for KVM and then
//! refuses by name where it does not open, with no fall back to TCG; `tcg`
//! asks for TCG. The command's exit code comes back through the markers of
//! [`init`]. The guest has user-mode networking.
//!
//! A `-v HOST:GUEST` is a 9p mount of the host directory ([`vol`],
//! TODO/machine.md T-1723), so a write in the guest lands on the host; with
//! a kernel of `PODVM_MACHINE_KERNEL` (no 9p module) it is a copy. `-t`
//! runs the command on a terminal of a virtio console ([`tty`]).

pub(crate) mod arch; // TODO/machine.md T-1890
pub(crate) mod archive;
pub(crate) mod channel;
pub(crate) mod cpio;
pub(crate) mod disk; // TODO/machine.md T-1968
mod drive;
pub(crate) mod engine;
pub(crate) mod exec; // TODO/machine.md T-1750
pub(crate) mod init;
mod input;
pub(crate) mod job; // TODO/machine.md T-2008
pub(crate) mod kernel;
pub(crate) mod kernel_arm64; // TODO/machine.md T-1890
pub(crate) mod kernel_image; // TODO/machine.md T-1994, part 2
mod line;
mod mem; // TODO/machine.md T-1922
pub(crate) mod net;
pub(crate) mod pid1; // TODO/machine.md T-1994, steps 2 and 3
mod rundir; // TODO/machine.md T-1906
pub(crate) mod sealed;
pub(crate) mod signals; // TODO/machine.md T-2041
pub(crate) mod squashfs;
pub(crate) mod state; // TODO/machine.md T-2008
pub(crate) mod stderr;
pub(crate) mod tty;
pub(crate) mod vol;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use podvm_image::error::EXIT_RUNTIME_ERROR;
use podvm_windows::plan::Accel;

#[cfg(test)]
use archive::base_path;
use archive::tools;
pub use archive::{base_archive, run_archive};
use drive::drive;
pub use input::{read_stdin, MAX_STDIN};
pub use line::notice;
use line::{no_nic, with_broker, with_cpus};

/// The version of the base archive format; a change of [`cpio`] bumps it,
/// so an old cached archive is not reused. 2 keeps the hard links; 3 the
/// owners of the image (T-1776).
pub const BASE_FORMAT: u32 = 3;
const DEFAULT_MEM: u64 = 512 << 20;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);

/// One guest run.
pub struct Request<'a> {
    pub verb: &'a str,
    /// `run -d`: the files of the keeper (TODO/machine.md T-1750).
    pub keeper: Option<exec::Keeper>,
    /// The architecture of the image (TODO/machine.md T-1890).
    pub arch: arch::Arch,
    /// The banner switch (`--verbose`, not `-q`): the notice of the tier.
    pub loud: bool,
    pub store_root: &'a Path,
    pub rootfs: &'a Path,
    pub digest: &'a str,
    pub argv: Vec<String>,
    pub env: Vec<String>,
    pub workdir: String,
    pub mem: Option<u64>,
    /// `--podvm-cpus`: the `-smp` of the guest; one where it is `None`.
    pub cpus: Option<u32>,
    /// `--network broker` (TODO/net.md T-1665): the port of the broker on
    /// the host's loopback. The guest network is restricted, and its one
    /// way out is a forward to the broker.
    pub broker: Option<u16>,
    pub timeout: Option<Duration>,
    pub qemu_args: Vec<String>,
    /// Host paths copied into the guest at GUEST (the probe of
    /// probe-inside). Writes in the guest do not return to the host.
    pub copies: Vec<(PathBuf, String)>,
    /// `-v HOST:GUEST[:ro]`: each a 9p mount of the host directory
    /// (TODO/machine.md T-1723); `true` is read-only.
    pub volumes: Vec<(PathBuf, String, bool)>,
    /// The guest paths of the `-v HOST:GUEST:mapped` volumes: their 9p
    /// share keeps the guest's owners and modes in xattrs (T-2103).
    pub mapped: Vec<String>,
    /// podvm's stdin where it is a pipe or a file, read to its end: the
    /// command's stdin in the guest (`/podvm/stdin`).
    pub stdin: Option<Vec<u8>>,
    /// `-t`: the command runs on a terminal of a virtio console, and stdin
    /// goes there live ([`tty`], TODO/machine.md T-1723).
    pub tty: bool,
    /// `--podvm-net none`: no NIC, and the init loads no NIC module
    /// (TODO/machine.md T-1751).
    pub net_none: bool,
    /// `--podvm-channel`: the host socket and the guest's TCP port of the
    /// one channel ([`channel`], T-1751).
    pub channel: Option<(PathBuf, u16)>,
    /// `--podvm-sealed`: the bundle of the run, which reaches the guest
    /// through the channel only ([`sealed`], T-1751).
    pub sealed: Option<Vec<u8>>,
    /// `--podvm-engine`: the guest loads the modules of a container
    /// engine ([`engine`], T-1780).
    pub engine: bool,
    /// `--pids-limit`: the `pids.max` of the cgroup of the payload in the
    /// guest ([`init::with_pids`], TODO/machine.md T-1967).
    pub pids: Option<String>,
    /// `--storage-opt size=N`: the bytes of the sized disk that holds the
    /// root and the working directory of the guest ([`disk`], T-1968).
    pub disk: Option<u64>,
}

/// The accelerator: `Err` where KVM was asked for and does not open.
pub fn accel(want: Option<&str>, kvm_opens: bool) -> Result<Accel, String> {
    match want {
        Some("kvm") if kvm_opens => Ok(Accel::Kvm),
        Some("kvm") => Err("PODVM_MACHINE_ACCEL=kvm asks for KVM, and /dev/kvm does not open \
                            or runs no guest of this arch (T-1907): podvm does not fall back to TCG (TODO/machine.md T-1656)"
            .to_string()),
        Some("tcg") => Ok(Accel::Tcg),
        Some(other) => Err(format!("PODVM_MACHINE_ACCEL={other}: the values are kvm and tcg")),
        None if kvm_opens => Ok(Accel::Kvm),
        None => Ok(Accel::Tcg),
    }
}

/// The emulator's arguments.
pub fn qemu_argv(
    accel: Accel,
    mem: u64,
    kernel: &Path,
    initrd: &Path,
    extra: &[String],
) -> Vec<String> {
    let mut v: Vec<String> = [
        "-M",
        "pc,acpi=off",
        "-nodefaults",
        "-no-user-config",
        "-display",
        "none",
        "-serial",
        "stdio",
        "-no-reboot",
        "-smp",
        "1",
        "-nic",
        "user,model=virtio-net-pci",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    v.push("-accel".to_string());
    v.push(match accel {
        Accel::Kvm => "kvm".to_string(),
        // TODO/enter.md T-2010: a translation cache of 512 MiB, where QEMU
        // takes 1 GiB. Measured with experiment 468 under TCG: the peak
        // memory of a run goes from 3,670 MiB to 3,300 MiB, and its time
        // stays in the difference between two runs.
        Accel::Tcg => "tcg,thread=multi,tb-size=512".to_string(),
    });
    v.push("-cpu".to_string());
    v.push(accel.cpu().to_string());
    v.push("-m".to_string());
    v.push(format!("{}M", mem.div_ceil(1 << 20)));
    v.push("-kernel".to_string());
    v.push(kernel.display().to_string());
    v.push("-initrd".to_string());
    v.push(initrd.display().to_string());
    v.push("-append".to_string());
    v.push("console=ttyS0 quiet loglevel=0 panic=-1 rdinit=/podvm-init".to_string());
    v.extend(extra.iter().cloned());
    v
}

/// Boot the guest, run the command, and give its exit code.
pub fn boot(r: &Request) -> i32 {
    let fail = |what: String| {
        eprintln!("podvm {}: machine tier: {what}", r.verb);
        EXIT_RUNTIME_ERROR
    };
    let want = std::env::var("PODVM_MACHINE_ACCEL").ok();
    // TODO/machine.md T-1890: the architecture of the image.
    let accel = match arch::accel(r.arch, want.as_deref(), crate::machine::ssh::kvm_opens()) {
        Ok(a) => a,
        Err(e) => return fail(e),
    };
    // TODO/machine.md T-1875: the store copy where the host has no QEMU.
    let qemu = match arch::emulator(r.verb, r.arch) {
        Ok(q) => q,
        Err(e) => return fail(e),
    };
    if std::fs::symlink_metadata(r.rootfs.join("bin/sh")).is_err() {
        return fail("the image has no /bin/sh, which runs /podvm-init in the guest".to_string());
    }
    // T-1994 part 2: the kernel of the image (`own`) where the variable asks.
    let (assets, own) = match kernel_image::of_run(r) {
        Ok(k) => k,
        Err(e) => return fail(format!("the guest kernel: {e}")),
    };
    // TODO/machine.md T-1723: a volume is a 9p mount where the kernel has
    // the modules (the pin, or the kernel of the image); a kernel of
    // PODVM_MACHINE_KERNEL has none, and a volume is then a copy.
    let nine = !r.volumes.is_empty() && (own.is_some() || !assets.volume_modules.is_empty());
    if let Err(e) = vol::check_mapped(&r.volumes, &r.mapped, nine) {
        return fail(e); // T-2103
    }
    let mut copies = r.copies.clone();
    let vols = if nine {
        vol::of_run(&r.volumes, &r.mapped)
    } else {
        copies.extend(r.volumes.iter().map(|(h, g, _)| (h.clone(), g.clone())));
        Vec::new()
    };
    let mut modules: Vec<(String, PathBuf)> = assets
        .modules
        .iter()
        .map(|(n, p)| (n.to_string(), p.clone()))
        .collect();
    if nine {
        modules.extend(
            assets
                .volume_modules
                .iter()
                .map(|(n, p)| (n.to_string(), p.clone())),
        );
    }
    // TODO/machine.md T-1780: the modules of a container engine.
    let mut engine_names = Vec::new();
    if r.engine {
        match engine::ensure(r.store_root) {
            Ok(m) => {
                engine_names = m.iter().map(|(n, _)| n.clone()).collect();
                modules.extend(m);
            }
            Err(e) => return fail(e),
        }
    }
    // TODO/machine.md T-1994: the root on the disk of the image.
    let set = !assets.modules.is_empty() || own.is_some();
    let on_disk = match disk::root_wanted(r, set) {
        Ok(b) => b,
        Err(e) => return fail(e),
    };
    let own_load = own.as_ref().map(|o| o.load.as_slice());
    let disk_load = match disk::add_modules(r, on_disk, set, own_load, &mut modules) {
        Ok(names) => names, // TODO/machine.md T-1968
        Err(e) => return fail(e),
    };
    let tools = match tools(
        &modules,
        engine_names,
        own.as_ref().map(|o| o.release.clone()),
    ) {
        Ok(t) => t,
        Err(e) => return fail(e),
    };
    let kernel = assets.kernel;
    let init_image = r.env.iter().any(|e| e == crate::initns::MARK);
    let pinned = !assets.volume_modules.is_empty() && !assets.modules.is_empty();
    // The saved state first where it serves the run (call C27, T-2008);
    // on the root disk its guest has systemd as PID 1, as each other run of
    // such an image (T-1994).
    let by_state = init_image && state::serves(r, pinned);
    let pid1 = match pid1::of(r.rootfs, init_image, on_disk) {
        Ok(b) => b,
        Err(e) => return fail(e),
    };
    // The base archive where the root is in the initramfs, and for the
    // saved state of T-2008, whose guest keeps its root there.
    let base = match (!on_disk || (init_image && pid1.is_none()))
        .then(|| base_archive(r.store_root, r.digest, r.rootfs))
        .transpose()
    {
        Ok(b) => b,
        Err(e) => return fail(e),
    };
    let root = match on_disk.then(|| disk::root_of(r)) {
        None => None,
        Some(Ok(d)) => Some(d),
        Some(Err(e)) => return fail(e),
    };
    if let Some(why) = tools
        .is_none()
        .then(|| archive::needs_tools(r, init_image))
        .flatten()
    {
        return fail(why);
    }
    // T-2008: the run from a saved state, or with the save of one.
    if by_state {
        let mut all = modules.clone(); // with the modules of the root disk
        for (n, p) in &assets.volume_modules {
            if !all.iter().any(|(m, _)| m == n) {
                all.push((n.to_string(), p.clone()));
            }
        }
        let ctx = state::Ctx {
            accel,
            qemu: &qemu,
            kernel: &kernel,
            base: base.as_ref().map(|b| b.0.as_path()),
            root: pid1
                .zip(root.as_ref())
                .map(|(bound, (disk, _))| state::RootLine {
                    disk,
                    load: &disk_load,
                    bound,
                    nonce: "",
                }),
            modules: &all,
            pinned_kernel: pinned,
        };
        if let state::Outcome::Done(code) = state::run(r, &ctx) {
            return code;
        }
    }
    let size = tty::size()
        .map(|(rows, cols)| format!("{rows}x{cols}"))
        .unwrap_or_default();
    let nonce = match init::nonce() {
        Ok(n) => n,
        Err(e) => return fail(e),
    };
    rundir::sweep(&r.store_root.join("machine")); // TODO/machine.md T-1906
    let dir =
        r.store_root
            .join("machine")
            .join(format!("run-{}-{}", std::process::id(), &nonce[..8]));
    let initrd = dir.join("initrd.cpio");
    let made = std::fs::create_dir_all(&dir)
        .and_then(|()| match (&root, &base) {
            (None, Some((b, _))) => std::fs::copy(b, &initrd).map(drop),
            _ => std::fs::write(&initrd, b""), // T-1994: podvm's files only
        })
        .and_then(|()| {
            let script = init::script(
                &nonce,
                &r.argv,
                &if pid1.is_some() {
                    pid1::image_env(&r.env)
                } else {
                    r.env.clone()
                },
                &r.workdir,
                tools.is_some() && !r.net_none,
                &vols,
                r.tty.then_some(size.as_str()),
                (r.channel.is_some() || r.sealed.is_some()).then(|| init::Chan {
                    port: r.channel.as_ref().map(|c| c.1),
                    sealed: r.sealed.is_some(),
                }),
            );
            let script = init::with_pids(script, r.pids.as_deref(), &nonce);
            let script = init::with_image_init(script, init_image && pid1.is_none()); // T-2002
            let script = exec::with_server_if(script, r.keeper.is_some());
            let (script, more) = match &root {
                Some(_) => (script, Vec::new()), // `guest-root` mounts the disk
                None => disk::init(script, &disk_load, &nonce)?, // T-1968
            };
            let mut a = run_archive(&script, tools.as_ref(), &copies, r.stdin.as_deref())?;
            a.extend(more);
            Ok(a)
        })
        .and_then(|extra| {
            let mut f = std::fs::OpenOptions::new().append(true).open(&initrd)?;
            f.write_all(&extra)
        });
    if let Err(e) = made {
        let _ = std::fs::remove_dir_all(&dir);
        return fail(format!("the initrd of the run: {e}"));
    }
    let disk_file = match r.disk.map(|b| disk::make(&dir, b)).transpose() {
        Ok(d) => d,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            return fail(e);
        }
    };
    let mut source = disk::root::source(&root, &base);
    if pid1.is_some() {
        source.push_str(", systemd of the image as PID 1 of the guest (T-1994)");
    }
    if let Some(o) = &own {
        source.push_str(&format!(", the kernel {} of the image", o.release));
    }
    if let Some(line) = notice(r, accel, &source) {
        eprintln!("{line}");
    }
    vol::notices(r.verb, &r.volumes, &r.mapped, nine, r.disk.is_some());
    let sock = stderr::socket_path(&nonce);
    let _ = std::fs::remove_file(&sock);
    let mut extra = stderr::argv(&sock);
    if r.tty {
        let _ = std::fs::remove_file(tty::socket_path(&sock));
        extra.extend(tty::argv(&tty::socket_path(&sock)));
    }
    // TODO/machine.md T-1751: the channel to a host socket; the memory locked where it can.
    if r.sealed.is_some() {
        match sealed::mem_lock(mem::size(r.verb, r.mem, &initrd, false)) {
            Ok(a) => extra.extend(a),
            Err(why) => eprintln!(
                "podvm {}: machine tier: the guest's memory is not locked: {why} \
                 (TODO/machine.md T-1751)",
                r.verb
            ),
        }
    }
    if r.channel.is_some() || r.sealed.is_some() {
        let ch = channel::socket_path(&sock);
        if let Err(e) = channel::make_dir(&ch) {
            return fail(format!("the directory of the channel socket: {e}"));
        }
        extra.extend(channel::argv(&ch));
    }
    for (v, (host, _, _)) in vols.iter().zip(&r.volumes) {
        extra.extend(vol::argv(host, v));
    }
    extra.extend(disk_file.iter().flat_map(|d| disk::argv(d)));
    extra.extend(root.iter().flat_map(|(p, _)| disk::root::argv(p))); // T-1994
    extra.extend(r.qemu_args.iter().cloned());
    let mut argv = qemu_argv(
        accel,
        mem::size(r.verb, r.mem, &initrd, true), // T-1922
        &kernel,
        &initrd,
        &extra,
    );
    arch::adapt(&mut argv, r.arch);
    if root.is_some() {
        disk::root::with_init(
            &mut argv,
            &disk_load,
            r.disk.is_some(),
            pid1.map(|b| (b, false)),
            &nonce,
        );
    }
    argv.extend(r.keeper.iter().flat_map(exec::keeper_args));
    arch::err_port(&mut argv, r.arch);
    if r.net_none {
        no_nic(&mut argv);
    }
    with_broker(&mut argv, r.broker);
    with_cpus(&mut argv, r.cpus);
    // TODO/machine.md T-2016: one more boot where the emulator crashed
    // before the command ran.
    let mut again = true;
    let code = loop {
        match drive(r, &qemu, &argv, &nonce, &dir, &sock, None) {
            state::Driven::Code(code) => break code,
            state::Driven::Again(why) if again => {
                again = false;
                eprintln!(
                    "podvm {}: machine tier: {why}; podvm boots the guest once more \
                     (TODO/machine.md T-2016)",
                    r.verb
                );
                let _ = std::fs::remove_file(&sock);
                let _ = std::fs::remove_file(tty::socket_path(&sock));
                if let Some(Err(e)) = r.disk.map(|b| disk::make(&dir, b)) {
                    break fail(e); // a new disk, T-1968
                }
            }
            // Only the guest of a state ends Cold, and this guest is none.
            state::Driven::Cold(why) | state::Driven::Again(why) => break fail(why),
        }
    };
    let _ = std::fs::remove_file(&sock);
    let _ = std::fs::remove_file(tty::socket_path(&sock));
    if let Some(d) = channel::socket_path(&sock).parent() {
        let _ = std::fs::remove_dir_all(d);
    }
    let _ = std::fs::remove_dir_all(&dir);
    code
}

#[cfg(test)]
mod crash_tests;
#[cfg(test)]
mod tests;
