//! The `/podvm-init` of a guest and the reading of its console,
//! TODO/machine.md T-1656.
//!
//! The kernel runs `/podvm-init` (`rdinit=`) with the console on the first
//! serial port. The script mounts `/proc`, `/sys` and `/dev`, prints the
//! ready marker, runs the command with the image's environment in the
//! working directory, prints the exit marker with the code, and resets the
//! machine; `-no-reboot` makes the emulator exit then. Each marker carries
//! a nonce of 128 bits of this run, as T-1304 rules: the payload can print
//! anything, so a marker without a nonce is a collision waiting to happen.
//! stdout and stderr share the serial line.

/// The channel of the init: the guest's port of `--podvm-channel`, and
/// whether the run is sealed (T-1751).
pub struct Chan {
    pub port: Option<u16>,
    pub sealed: bool,
}

/// Quote one word for `/bin/sh`.
pub fn sh_quote(w: &str) -> String {
    format!("'{}'", w.replace('\'', "'\\''"))
}

/// The init script for `argv` with `env` (`NAME=VALUE`) in `workdir`.
/// `net` runs `podvm guest-net` first, which loads the NIC driver and
/// configures the network; a failure of it is a line of output. `vols`
/// runs `podvm guest-9p` before the `cd`, so the working directory can be
/// in a volume (TODO/machine.md T-1723); where a mount fails, the run ends
/// with 125 and the reason, and the command does not run. `tty` (`-t`, the
/// size `ROWSxCOLS` or empty) runs the command through `podvm guest-tty` on
/// the virtio console (TODO/machine.md T-1723) in place of the FIFOs.
/// `channel` starts `podvm guest-channel` and waits until it listens
/// (TODO/machine.md T-1751); a sealed run ([`super::sealed`]) fetches the
/// bundle first and sources its fragment after the image's command.
#[allow(clippy::too_many_arguments)]
pub fn script(
    nonce: &str,
    argv: &[String],
    env: &[String],
    workdir: &str,
    net: bool,
    vols: &[super::vol::Vol],
    tty: Option<&str>,
    channel: Option<Chan>,
) -> String {
    let mut s = head();
    s.push_str(&body(
        nonce, argv, env, workdir, net, vols, tty, channel, false,
    ));
    s
}

/// The first lines of the init of a run: the mounts and the modules, which
/// come before the root moves to a sized disk (TODO/machine.md T-1968).
pub fn head() -> String {
    format!("#!/bin/sh\n{}", setup())
}

/// TODO/machine.md T-1968: the init where the root did not reach its
/// sized disk: the reason of `podvm guest-disk` on the second port, then
/// the exit marker with 125, as [`early_exit`] does; the command does not
/// run on the RAM of the guest in place of its disk.
pub fn disk_fail(nonce: &str) -> String {
    let err = super::disk::ERR;
    format!(
        "#!/bin/sh\n{{ echo PODVM-READY-{nonce}; {ERR_PORT}; \
         l=\"podvm: machine tier: --storage-opt size=: the root of the guest is not on its \
         disk: $(cat {err} 2>/dev/null) (TODO/machine.md T-1968)\"; \
         if [ -c \"$E\" ]; then echo \"$l\" >\"$E\"; else echo \"$l\" >&2; fi; echo; \
         echo PODVM-EXIT-{nonce} 125; sync; reboot -f; echo b > /proc/sysrq-trigger; }}\n"
    )
}

/// The first lines of each init: the mounts of the guest and its modules.
fn setup() -> String {
    let mut s = String::new();
    s.push_str("mount -t proc proc /proc 2>/dev/null\n");
    s.push_str("mount -t sysfs sysfs /sys 2>/dev/null\n");
    s.push_str("mount -t devtmpfs devtmpfs /dev 2>/dev/null\n");
    // TODO/machine.md T-1780: cgroup2 where an engine or a payload looks
    // for it; `lo` up in each guest that carries podvm, and the modules of
    // `--podvm-engine` where the archive names them.
    s.push_str("mount -t cgroup2 cgroup2 /sys/fs/cgroup 2>/dev/null\n");
    s.push_str(&format!(
        "[ -x /podvm/podvm ] && /podvm/podvm guest-net --load $([ -f {l} ] && cat {l})\n",
        l = super::archive::ENGINE_LIST
    ));
    s
}

/// The state file of the held init of a guest whose state is saved
/// (TODO/machine.md T-2008), and the file of the job of a run.
pub const HOLD_FILE: &str = "/podvm/hold";
pub const JOB_FILE: &str = "/podvm/job.sh";

/// TODO/machine.md T-2008: the init of a guest whose state podvm saves. It
/// has nothing of a run: after the mounts and the network it boots the init
/// of the image (`guest-init --hold`), waits for the job of a run on the
/// serial line (`guest-job`, [`super::job`]), and runs the script of the
/// job ([`job`]). `nonce` is the one of the state, for the markers before
/// the job. A job that the guest does not take ends the guest.
pub fn template(nonce: &str, net: bool) -> String {
    let mut s = String::from("#!/bin/sh\n");
    s.push_str(&setup());
    if net {
        s.push_str("/podvm/podvm guest-net\n");
    }
    // The 9p modules load before the save, with no volume to mount: the
    // driver takes each 9p device now, so a guest from the state changes
    // no PCI device when it mounts a volume. QEMU before 10.0.4 can end on
    // SIGSEGV when a guest of more than one vCPU changes the memory map
    // ("system/physmem: fix use-after-free with dispatch"); in the guest
    // that boots, podvm boots again, and after the command starts it can
    // not.
    s.push_str("/podvm/podvm guest-9p\n");
    s.push_str(&format!(
        "/podvm/podvm {} --hold {HOLD_FILE} --cgroup auto\nh=$?\n",
        crate::initns::VERB
    ));
    s.push_str(&format!(
        "/podvm/podvm guest-job {nonce} $h {JOB_FILE} || {{ sync; reboot -f 2>/dev/null; \
         echo b > /proc/sysrq-trigger; }}\n"
    ));
    s.push_str(&format!(". {JOB_FILE}\n"));
    s
}

/// TODO/machine.md T-2008: the script of one run for a guest of
/// [`template`]: the lines of [`script`] after the mounts, with the
/// volumes mounted in the mount namespace of the held init and the command
/// run in that init. The network of the guest is up from the save. The
/// channel listener and the process limit belong to the job.
pub fn job(
    nonce: &str,
    argv: &[String],
    env: &[String],
    workdir: &str,
    vols: &[super::vol::Vol],
    channel_port: Option<u16>,
    pids: Option<&str>,
) -> String {
    let channel = channel_port.map(|port| Chan {
        port: Some(port),
        sealed: false,
    });
    let script = body(nonce, argv, env, workdir, false, vols, None, channel, true);
    with_pids(script, pids, nonce)
}

/// The lines of an init for one run, after [`setup`]. `held` is the run
/// of a guest with a held init (TODO/machine.md T-2008).
#[allow(clippy::too_many_arguments)]
fn body(
    nonce: &str,
    argv: &[String],
    env: &[String],
    workdir: &str,
    net: bool,
    vols: &[super::vol::Vol],
    tty: Option<&str>,
    channel: Option<Chan>,
    held: bool,
) -> String {
    let mut s = String::new();
    if !vols.is_empty() {
        let args: Vec<String> = vols.iter().map(|v| sh_quote(&super::vol::arg(v))).collect();
        let hold = if held {
            format!("{} {HOLD_FILE} ", super::vol::HOLD_FLAG)
        } else {
            String::new()
        };
        s.push_str(&format!(
            "/podvm/podvm guest-9p {hold}{} 2>/podvm-9p.err || {{ echo PODVM-READY-{nonce}; \
             cat /podvm-9p.err; echo; echo PODVM-EXIT-{nonce} 125; sync; reboot -f; \
             echo b > /proc/sysrq-trigger; }}\n",
            args.join(" ")
        ));
    }
    for e in env {
        if let Some((k, v)) = e.split_once('=')
            && !k.is_empty()
            && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            s.push_str(&format!("export {k}={}\n", sh_quote(v)));
        }
    }
    // TODO/machine.md T-1823: a missing working directory is made, as on
    // the rootfs rungs and in docker; a file there refuses by name.
    let w = sh_quote(workdir);
    s.push_str(&format!(
        "mkdir -p {w} 2>/dev/null; cd {w} 2>/dev/null || {}\n",
        early_exit(
            nonce,
            &format!("the working directory {workdir} is not a directory in the image")
        )
    ));
    s.push_str(&super::signals::start(nonce)); // T-2041: before the marker
    s.push_str(&format!("echo PODVM-READY-{nonce}\n"));
    if net {
        s.push_str("/podvm/podvm guest-net\n");
    }
    // TODO/machine.md T-1751: the channel listens before the command runs.
    if let Some(c) = &channel {
        let args = match (c.sealed, c.port) {
            (true, Some(p)) => format!("--sealed {p}"),
            (true, None) => "--sealed".to_string(),
            (false, Some(p)) => p.to_string(),
            (false, None) => String::new(),
        };
        // A bundle of 64 MiB takes longer than 10 s under TCG.
        let tries = if c.sealed { 1200 } else { 100 };
        s.push_str(&format!(
            "/podvm/podvm guest-channel {args} & i=0\nwhile [ ! -e {r} ] && [ $i -lt {tries} ]; do sleep 0.1; i=$((i+1)); done\n",
            r = super::channel::READY
        ));
        if c.sealed || held {
            let why = if c.sealed {
                "the run bundle did not reach the guest"
            } else {
                "--podvm-channel: the guest channel did not start"
            };
            s.push_str(&format!(
                "[ -e {r} ] || {{ echo 'podvm: {why}'; echo; \
                 echo PODVM-EXIT-{nonce} 125; sync; reboot -f; echo b > /proc/sysrq-trigger; exit 125; }}\n",
                r = super::channel::READY
            ));
        }
    }
    // stdin: the bytes of podvm's stdin where there were some, else none.
    s.push_str("if [ -f /podvm/stdin ]; then exec 0</podvm/stdin; else exec 0</dev/null; fi\n");
    let words: Vec<String> = argv.iter().map(|a| sh_quote(a)).collect();
    s.push_str(&format!("set -- {}\n", words.join(" ")));
    if channel.as_ref().is_some_and(|c| c.sealed) {
        s.push_str(&format!(
            ". {} || {}\n",
            super::sealed::FRAGMENT,
            early_exit(nonce, "the run bundle did not unpack in the guest")
        ));
    }
    if let Some(size) = tty {
        let size = if size.is_empty() {
            String::new()
        } else {
            format!("{} ", sh_quote(size))
        };
        s.push_str(&format!(
            "{}\nrc=$?\n{OOM}\necho; echo PODVM-EXIT-{nonce} $rc $o\n",
            super::signals::wrap(&format!("/podvm/podvm guest-tty {size}-- \"$@\""))
        ));
        s.push_str("sync\nreboot -f 2>/dev/null\necho b > /proc/sysrq-trigger\n");
        return s;
    }
    // TODO/machine.md T-1728. The command's stdout and stderr are FIFOs, as
    // docker's are without -t: a serial port is a terminal, so `ls` coloured
    // its output and `[ -t 1 ]` was true. One `cat` copies each FIFO to its
    // port: stdout to the console, stderr to the second port (T-1656) where
    // the node is there.
    s.push_str("mkfifo /podvm-out /podvm-err\n");
    s.push_str("cat /podvm-out & o=$!\n");
    // TODO/machine.md T-1890: an arm64 guest (`-M virt`, one PL011) has its
    // second port as a virtio port.
    s.push_str(&format!(
        "{ERR_PORT}\nif [ -c \"$E\" ]; then cat /podvm-err >\"$E\" & else cat /podvm-err >&2 & fi\ne=$!\n"
    ));
    let run = if held {
        format!(
            "/podvm/podvm {} --attach {HOLD_FILE} --cwd \"$PWD\" -- \"$@\"",
            crate::initns::VERB
        )
    } else {
        "\"$@\"".to_string()
    };
    s.push_str(&format!(
        "{} >/podvm-out 2>/podvm-err\nrc=$?\nwait $o $e\n",
        super::signals::wrap(&run) // T-2041: the pid of the command
    ));
    s.push_str(&format!("{OOM}\necho; echo PODVM-EXIT-{nonce} $rc $o\n"));
    s.push_str("sync\nreboot -f 2>/dev/null\necho b > /proc/sysrq-trigger\n");
    s
}

/// What one line of the console is.
#[derive(Debug, PartialEq, Eq)]
pub enum Line<'a> {
    /// Before the ready marker: the kernel and the image's start.
    Before,
    /// The ready marker.
    Ready,
    /// Output of the command.
    Out(&'a str),
    /// The exit marker, with the code and the output before it on the line.
    Exit(i32, &'a str),
}

/// The console reader of one run.
pub struct Console {
    nonce: String,
    ready: bool,
    /// The OOM kills of the guest that the exit marker gave (T-1969).
    pub oom_kills: Option<u64>,
}

impl Console {
    pub fn new(nonce: &str) -> Self {
        Console {
            nonce: nonce.to_string(),
            ready: false,
            oom_kills: None,
        }
    }

    /// Read one line (without its newline; a `\r` is dropped).
    pub fn line<'a>(&mut self, raw: &'a str) -> Line<'a> {
        let raw = raw.trim_end_matches('\r');
        if !self.ready {
            if raw.trim() == format!("PODVM-READY-{}", self.nonce) {
                self.ready = true;
                return Line::Ready;
            }
            return Line::Before;
        }
        let mark = format!("PODVM-EXIT-{} ", self.nonce);
        if let Some(at) = raw.find(&mark) {
            let mut f = raw[at + mark.len()..].split_whitespace();
            let code = f.next().and_then(|c| c.parse::<i32>().ok()).unwrap_or(125);
            self.oom_kills = f.next().and_then(|n| n.parse::<u64>().ok());
            return Line::Exit(code, &raw[..at]);
        }
        Line::Out(raw)
    }
}

/// A nonce of 128 bits from the kernel's random source, as hex.
pub fn nonce() -> Result<String, String> {
    use std::io::Read;
    let mut b = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .map_err(|e| format!("/dev/urandom: {e}"))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// TODO/machine.md T-1969 (PodUtils/podrunner#296 ask 6): the count
/// `oom_kill` of the guest's `/proc/vmstat` into `$o`, with shell builtins
/// only (an image can have no `sed`); empty where the kernel has no such
/// line. The exit marker carries it after the code.
const OOM: &str =
    "o=; while read -r k v; do [ \"$k\" = oom_kill ] && o=$v; done </proc/vmstat 2>/dev/null";

/// The second port of the guest, the one of stderr, into `$E`: `ttyS1` on
/// x86_64; on arm64 the virtio port named `podvm.err` (TODO/machine.md
/// T-1890), found by its name in `/sys/class/virtio-ports`.
/// The named port wins: the 8250 driver of an arm64 kernel makes the node
/// `ttyS1` with no port behind it, and a write there fails with EIO.
const ERR_PORT: &str = "E=/dev/ttyS1; for p in /sys/class/virtio-ports/*; do \
     [ \"$(cat $p/name 2>/dev/null)\" = podvm.err ] && E=/dev/${p##*/}; done";

/// TODO/machine.md T-1823: an end of the init before the command, as one
/// shell group: the reason on the second port (stderr), then the exit
/// marker with 125, so that the host names the reason and not a missing
/// marker.
fn early_exit(nonce: &str, why: &str) -> String {
    let line = sh_quote(&format!(
        "podvm: machine tier: {why} (TODO/machine.md T-1823)"
    ));
    format!(
        "{{ echo PODVM-READY-{nonce}; {ERR_PORT}; if [ -c \"$E\" ]; then echo {line} >\"$E\"; \
         else echo {line} >&2; fi; echo; echo PODVM-EXIT-{nonce} 125; sync; reboot -f; \
         echo b > /proc/sysrq-trigger; }}"
    )
}

/// TODO/enter.md T-2002: `--podvm-init=image` in the guest. The command
/// runs through `podvm guest-init`, which boots the init of the image as
/// PID 1 of a PID namespace of the guest and runs the command in it.
/// podvm's own init stays PID 1 of the guest, with the markers, the
/// volumes and the exit code as before. An internal variable of podvm
/// does not reach the command (`initns::join`).
pub fn with_image_init(script: String, init_image: bool) -> String {
    if !init_image {
        return script;
    }
    let run = format!(
        "/podvm/podvm {} --cgroup auto --cwd \"$PWD\" -- \"$@\"",
        crate::initns::VERB
    );
    // The last `"$@"` is the one of the script. An argument or a variable
    // of the run can hold the same four bytes (`sh -c 'exec "$@"'`), and
    // it must stay as it is.
    match script.rfind("\"$@\"") {
        Some(at) => {
            let mut s = script;
            s.replace_range(at..at + 4, &run);
            s
        }
        None => script,
    }
}

/// PodUtils/podrunner#296 ask 4 (TODO/machine.md T-1967): `--pids-limit N`
/// in the guest. Before the ready marker, the init enables the `pids`
/// controller, makes the cgroup `podvm-payload` with `pids.max`, and moves
/// itself into it, so the command and each of its children count. A guest
/// that cannot hold it ends the run with 125 and the reason; the command
/// does not run without its limit.
pub fn with_pids(script: String, pids: Option<&str>, nonce: &str) -> String {
    let Some(n) = pids.filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())) else {
        return script;
    };
    let c = "/sys/fs/cgroup";
    let block = format!(
        "{{ echo +pids > {c}/cgroup.subtree_control && mkdir -p {c}/podvm-payload && \
         echo {n} > {c}/podvm-payload/pids.max && echo $$ > {c}/podvm-payload/cgroup.procs; \
         }} 2>/dev/null || {{ {}; exit 125; }}\n",
        early_exit(
            nonce,
            "--pids-limit: the guest has no cgroup2 pids controller to hold it"
        )
    );
    // Before the ready marker: the early exit prints the marker itself.
    match script.find(&format!("\necho PODVM-READY-{nonce}\n")) {
        Some(at) => {
            let mut s = script;
            s.insert_str(at + 1, &block);
            s
        }
        None => script,
    }
}
