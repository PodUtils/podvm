//! TODO/machine.md T-2008: the saved state of a guest that booted the init
//! of an image, so that a run of `--podvm-init=image` on the machine tier
//! does not boot the kernel and systemd each time.
//!
//! The first run of an image and a guest shape boots a guest with nothing
//! of a run in it ([`super::init::template`]): the init of the image is up,
//! and the guest waits for one job on its serial line ([`super::job`]).
//! podvm asks QEMU for the state of that guest (QMP `migrate` to a file of
//! the store), lets the guest go on, and gives it the job of the run. Each
//! later run starts QEMU with `-incoming` and that file, and gives its own
//! job: the command, the environment, the volumes, the time of the host and
//! a seed for the random pool.
//!
//! The rules of the state:
//!
//! * a state is used only where its fingerprint is the text of this run,
//!   line for line ([`fingerprint`]): the digest of the image, the digest
//!   of podvm, the emulator and its version, the CPU, the kernel, the
//!   memory, the machine line and the script of the guest. Another value is another file name:
//!   a stale state is never loaded;
//! * a state that is missing, that another run saves now, that does not
//!   load, or whose guest does not take the job, gives a boot as before
//!   ([`Outcome::Boot`]). The fall back comes only before the guest starts
//!   the command;
//! * the save writes under a name of its own (the nonce of the run) and
//!   under a lock, syncs the file, and gives it its name last: a reader
//!   finds a whole state or none;
//! * each failure is counted in a marker with its reason; after three in a
//!   row no run tries the state for one day, so that each run does not pay
//!   for the same failure. A load that works ends the count.
//!
//! What a state does not hold is in `docs/limits.md`: each run of one state
//! has the machine-id, the boot id and the service processes of the save.

use std::path::Path;
use std::time::{Duration, SystemTime};

use podvm_windows::plan::Accel;

use super::{arch, init, Request};

mod run;
mod save;
mod store;

pub use run::{run, send_job, Ctx, Driven, Mode, Outcome, Warm};
#[cfg(test)]
use save::running;
pub use save::save;
#[cfg(test)]
use store::{files, mark_bad, marked_bad, plan, strikes, sweep, touch, valid, Files, Meta, Plan};

/// `PODVM_MACHINE_STATE`: `0` boots each guest; `1` (the default) saves
/// and loads; `pipe` saves and loads through a pipe of `cat`, which is the
/// path of a QEMU before 8.2.
pub const VAR: &str = "PODVM_MACHINE_STATE";
/// The 9p devices of a saved guest. A device cannot come after the save,
/// so each guest has this count, and a run with more volumes boots.
pub const SLOTS: usize = 4;
/// The states of one image that stay in the store: the last ones used.
const KEEP: usize = 2;
/// How long the marker of a failed state holds.
const BAD_HOLDS: Duration = Duration::from_secs(24 * 3600);
/// The failures in a row after which the marker holds. One failure can be
/// an emulator that ended for a reason of its own. Measured: QEMU 8.2.2
/// with four vCPUs under TCG ended on SIGSEGV in the first seconds of a
/// boot in 6 of about 245 starts on a busy host, in
/// `address_space_translate_for_iotlb` (the fault that QEMU 10.0.4 and
/// 10.1 fix as "system/physmem: fix use-after-free with dispatch"). Three
/// failures in a row are a state that this host cannot use.
const STRIKES: u32 = 3;
/// The bound of a loaded guest for its ready marker. Measured under TCG: 2 s.
pub const READY_BOUND: Duration = Duration::from_secs(60);
/// The bound of one save.
const SAVE_BOUND: Duration = Duration::from_secs(180);
const SCHEMA: &str = "podvm-machine-state.v1";

/// How the state goes to its file and comes back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// QEMU writes and reads the file (`file:`, QEMU 8.2 and later).
    File,
    /// QEMU writes to and reads from a pipe of `cat` (`exec:`).
    Pipe,
}

impl Transport {
    fn word(self) -> &'static str {
        match self {
            Transport::File => "file",
            Transport::Pipe => "pipe",
        }
    }

    fn of(word: &str) -> Option<Transport> {
        match word {
            "file" => Some(Transport::File),
            "pipe" => Some(Transport::Pipe),
            _ => None,
        }
    }

    /// The URI of QEMU for a save into `path`.
    fn save_uri(self, path: &str) -> String {
        match self {
            Transport::File => format!("file:{path}"),
            Transport::Pipe => format!("exec:cat > {}", init::sh_quote(path)),
        }
    }

    /// The URI of QEMU for a load from `path`.
    fn load_uri(self, path: &str) -> String {
        match self {
            Transport::File => format!("file:{path}"),
            Transport::Pipe => format!("exec:cat {}", init::sh_quote(path)),
        }
    }
}

/// What the variable asks for: no state, or the first transport to try.
pub fn wanted(var: Option<&str>) -> Result<Option<Transport>, String> {
    match var {
        None | Some("1") => Ok(Some(Transport::File)),
        Some("pipe") => Ok(Some(Transport::Pipe)),
        Some("0") => Ok(None),
        Some(other) => Err(format!("{VAR}={other}: the values are 0, 1 and pipe")),
    }
}

/// What a run asks of its guest, as far as a saved state cares.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Asks {
    pub init_image: bool,
    pub tty: bool,
    pub detach: bool,
    pub channel: bool,
    pub engine: bool,
    pub pids: bool,
    pub broker: bool,
    pub copies: bool,
    pub volumes: usize,
    pub stdin: usize,
    /// An x86_64 guest on an x86_64 host.
    pub native: bool,
    /// The kernel of the guest brings podvm and the 9p modules (the pin).
    pub pinned_kernel: bool,
}

impl Asks {
    pub fn of(r: &Request, init_image: bool, pinned_kernel: bool) -> Asks {
        Asks {
            init_image,
            tty: r.tty,
            detach: r.keeper.is_some(),
            channel: r.channel.is_some() || r.sealed.is_some(),
            engine: r.engine,
            pids: r.pids.is_some(),
            broker: r.broker.is_some(),
            copies: !r.copies.is_empty(),
            volumes: r.volumes.len(),
            stdin: r.stdin.as_ref().map_or(0, Vec::len),
            native: r.arch == arch::Arch::Amd64 && arch::Arch::host() == arch::Arch::Amd64,
            pinned_kernel,
        }
    }
}

/// Why a run cannot start from a saved state, or `None` where it can. Each
/// reason is a part of the run that the saved guest does not have, or that
/// goes to the guest by a way that a loaded guest does not read.
pub fn refusal(a: &Asks) -> Option<String> {
    let no = |what: &str| Some(format!("a saved state does not serve {what}"));
    if !a.init_image {
        return no("a run with no --podvm-init=image");
    }
    if !a.native {
        return no("a guest of another architecture than x86_64 on x86_64");
    }
    if !a.pinned_kernel {
        return no("the kernel of PODVM_MACHINE_KERNEL");
    }
    if a.tty {
        return no("-t");
    }
    if a.detach {
        return no("run -d");
    }
    if a.channel {
        return no("--podvm-channel and --podvm-sealed");
    }
    if a.engine {
        return no("--podvm-engine");
    }
    if a.pids {
        return no("--pids-limit");
    }
    if a.broker {
        return no("--network broker");
    }
    if a.copies {
        return no("a copy into the guest");
    }
    if a.volumes > SLOTS {
        return Some(format!(
            "a saved state has {SLOTS} volume devices, and the run has {} volumes",
            a.volumes
        ));
    }
    if a.stdin > super::job::MAX_STDIN {
        return Some(format!(
            "a saved state takes {} bytes of stdin, and the run has {}",
            super::job::MAX_STDIN,
            a.stdin
        ));
    }
    None
}

/// The length and the change time of a file, as one word of a fingerprint.
fn ident(p: &Path) -> Result<String, String> {
    let m = std::fs::metadata(p).map_err(|e| format!("{}: {e}", p.display()))?;
    let t = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    Ok(format!("{} {t}", m.len()))
}

/// What a state depends on.
pub struct Shape<'a> {
    pub digest: &'a str,
    pub accel: Accel,
    pub qemu: &'a Path,
    pub kernel: &'a Path,
    /// `--podvm-mem`, or none for the size that the initrd gives.
    pub mem: Option<u64>,
    /// The machine line with each path of a run as a fixed name.
    pub line: &'a [String],
    /// The script of the guest, with a fixed word in place of its nonce.
    pub script: &'a str,
}

/// The CPU of a state: `tcg`; under KVM the CPU of the host
/// (`pod_guest::warm::host_cpu`) and the release of its kernel, as
/// `-cpu host` gives the guest what both have.
fn cpu(accel: Accel) -> String {
    match accel {
        Accel::Tcg => "tcg".to_string(),
        Accel::Kvm => {
            let info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
            let os = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
            format!("{} {}", podvm_windows::warm::host_cpu(&info), os.trim())
        }
    }
}

/// The text that a state is saved under and loaded by. An error where a
/// part cannot be read: the run then boots.
pub fn fingerprint(s: &Shape) -> Result<String, String> {
    let version = podvm_windows::warm::qemu_version(s.qemu).map_err(|e| e.to_string())?;
    let mut t = format!("{SCHEMA}\n");
    // podvm itself is in the guest of a state, and it reads the job of each
    // later run: the bytes of this program, not its change time. Measured
    // with experiment 468, which copies the binary for each of its runs: a
    // change time in this line saved a second state for the same program.
    t.push_str(&format!(
        "podvm {} sha256:{}\n",
        env!("CARGO_PKG_VERSION"),
        crate::assets::sha256_file(Path::new("/proc/self/exe"))?
    ));
    t.push_str(&format!(
        "qemu {version} {} {}\n",
        s.qemu.display(),
        ident(s.qemu)?
    ));
    t.push_str(&format!("cpu {}\n", cpu(s.accel)));
    // The file of the pinned kernel has the head of its digest in its name.
    let kernel = std::fs::metadata(s.kernel)
        .map_err(|e| format!("{}: {e}", s.kernel.display()))?
        .len();
    t.push_str(&format!("kernel {} {kernel}\n", s.kernel.display()));
    t.push_str(&format!(
        "image {} base-format {}\n",
        s.digest,
        super::BASE_FORMAT
    ));
    t.push_str(&match s.mem {
        Some(m) => format!("mem {m}\n"),
        None => "mem of-the-initrd\n".to_string(),
    });
    t.push_str(&format!(
        "script {}\n",
        super::kernel::sha256_hex(s.script.as_bytes())
    ));
    for a in s.line {
        t.push_str(&format!("arg {a}\n"));
    }
    Ok(t)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_save;
