//! `run --podvm-tier=incus`: one command in an incus system container,
//! TODO/machine.md T-1718, in the form of `docs/incus.md`. The calls of the
//! incus REST API are `pod-incus` of PodUtils/podcrates (T-1727).
//!
//! 1. The image: podvm's extracted rootfs as an incus unified tarball
//!    ([`super::image`]) with podvm itself at `/.podvm/podvm`, imported
//!    one time for each manifest digest and podvm build under the alias
//!    `podvm-HEX16-PODVM16-vN` (TODO/machine.md T-1722).
//! 2. The instance: from that alias, with podvm itself as its init
//!    (`lxc.init.cmd = /.podvm/podvm guest-hold`), so an image with no
//!    `sleep` and no shell still starts.
//! 3. The command: the exec API with `record-output`, so stdout and stderr
//!    come back apart with the exit code. Piped stdin goes in as a file.
//! 4. The end: the instance is deleted, as under `--rm`.
//!
//! A `-v HOST:GUEST[:ro]` is a `disk` device of the instance
//! ([`disk_devices`], TODO/machine.md T-1722): incusd maps the host
//! directory into the instance, so a write lands on the host.
//!
//! The command is not PID 1 here (the hold is); the parity note says so.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use pod_incus::{
    Client, Exec, FileWrite, ImageData, ImageImport, NewInstance, Payload, Settings, Source,
};
use podvm_image::error::EXIT_RUNTIME_ERROR;

use super::image::{alias, unified_tarball_to};

pub struct Job<'a> {
    pub verb: &'a str,
    pub rootfs: &'a Path,
    pub digest: &'a str,
    pub argv: Vec<String>,
    pub env: Vec<String>,
    pub workdir: String,
    pub stdin: Option<Vec<u8>>,
    /// `--memory`, `--cpus`, `--pids-limit`: keys of the instance
    /// (TODO/machine.md T-1897, PodUtils/podrunner#110).
    pub limits: Vec<(&'static str, String)>,
    /// `-v HOST:GUEST[:ro]`; `true` is read-only.
    pub volumes: Vec<(String, String, bool)>,
    /// `run -d` and `create`: an instance that stays (T-1722).
    pub detach: Option<super::verbs::Detach>,
    /// `--podvm-net none` and `--podvm-channel` ([`super::net`], T-1779).
    pub net_none: bool,
    pub channel: Option<(PathBuf, u16)>,
}

/// The limits of `job` on `new`: each key goes into the configuration,
/// except the size of the root disk (`--storage-opt size=N`,
/// PodUtils/podrunner#296 ask 1, TODO/machine.md T-1964), which is a copy of
/// the `root` device of the default profile with `size`. A pool driver that
/// cannot size it refuses the create with the words of incusd.
pub async fn apply_limits(
    c: &Client,
    new: &mut pod_incus::NewInstance,
    limits: &[(&'static str, String)],
) -> Result<(), String> {
    for (k, v) in limits {
        if *k != crate::run::limits::ROOT_SIZE {
            new.config.insert(k.to_string(), v.clone());
            continue;
        }
        let profile = c
            .profile("default")
            .await
            .map_err(|e| format!("--storage-opt: the default profile: {e}"))?;
        let Some(mut root) = profile.devices.get("root").cloned() else {
            return Err(
                "--storage-opt: the default profile of incusd has no `root` disk \
                        device to size"
                    .to_string(),
            );
        };
        root.insert("size".to_string(), v.clone());
        new.devices.insert("root".to_string(), root);
    }
    Ok(())
}

/// After the start of an instance with `--storage-opt size=N`: the state
/// of its root disk must name a size, or the pool does not hold one (the
/// driver `dir` takes `size` and bounds nothing). The quality rule: no flag
/// that a host cannot hold runs as if it held (TODO/machine.md T-1964).
pub async fn size_held(
    c: &Client,
    name: &str,
    limits: &[(&'static str, String)],
) -> Result<(), String> {
    let Some((_, v)) = limits
        .iter()
        .find(|(k, _)| *k == crate::run::limits::ROOT_SIZE)
    else {
        return Ok(());
    };
    let s = c
        .instance_state(name)
        .await
        .map_err(|e| format!("--storage-opt: the state of the instance: {e}"))?;
    match s.disk.get("root") {
        Some(d) if d.total > 0 => Ok(()),
        _ => Err(format!(
            "--storage-opt size={v}: the storage pool of the instance holds no size of \
             its root disk (incusd reports no limit; a pool of the driver `dir` bounds \
             nothing), and podvm does not run it as if it held one"
        )),
    }
}

/// PodUtils/podrunner#296 ask 3 (TODO/machine.md T-1966): the keys that
/// let a container engine run in an unprivileged instance with
/// `--podvm-engine`: a nested `proc` and `sysfs`, and the intercepts of
/// `mknod` and `setxattr` that its layers need. The root of the instance
/// stays an id of the subordinate range of the host.
pub fn engine_keys(engine: bool) -> Vec<(&'static str, String)> {
    if !engine {
        return Vec::new();
    }
    [
        "security.nesting",
        "security.syscalls.intercept.mknod",
        "security.syscalls.intercept.setxattr",
    ]
    .into_iter()
    .map(|k| (k, "true".to_string()))
    .collect()
}

/// PodUtils/podrunner#316 ask 1 (TODO/machine.md T-1996): the ids of an
/// instance with the init of the image. The hosted snapshot gives `runner`
/// the subordinate ids `165536:65536`, so rootless podman needs ids up to
/// 231071: an isolated map of this size, from the range of incusd.
pub const SNAPSHOT_IDS: u32 = 231_072;
// runner's subordinate ids `165536:65536` fit in the map.
const _: () = assert!(SNAPSHOT_IDS >= 165_536 + 65_536);

/// The keys of an instance with `--podvm-init=image` (T-1996).
pub fn snapshot_keys(init_image: bool) -> Vec<(&'static str, String)> {
    if !init_image {
        return Vec::new();
    }
    vec![
        ("security.idmap.isolated", "true".to_string()),
        ("security.idmap.size", SNAPSHOT_IDS.to_string()),
    ]
}

/// An error of incusd that names too few ids, with what `--podvm-init=image`
/// needs and where the range lives (T-1996).
pub fn ids_hint(e: String, init_image: bool) -> String {
    if init_image && e.contains("Not enough uid/gid") {
        format!(
            "{e}: --podvm-init=image maps {SNAPSHOT_IDS} ids for the hosted snapshot \
             (TODO/machine.md T-1996), and incusd has fewer free: give the user of \
             incusd a larger range in /etc/subuid and /etc/subgid"
        )
    } else {
        e
    }
}

/// The device `/dev/fuse` of an instance with `--podvm-init=image`
/// (T-1996): `fuse-overlayfs` and `sshfs` of the snapshot need it.
pub fn fuse_device() -> (String, BTreeMap<String, String>) {
    let mut d = BTreeMap::new();
    d.insert("type".to_string(), "unix-char".to_string());
    d.insert("path".to_string(), "/dev/fuse".to_string());
    ("podvm-fuse".to_string(), d)
}

/// The devices of an instance of `job`: the volumes, the mask of the NIC
/// and the channel.
pub fn job_devices(job: &Job<'_>) -> pod_incus::Devices {
    let mut d = disk_devices(&job.volumes);
    d.extend(super::net::devices(
        job.net_none,
        &["eth0".to_string()],
        job.channel.as_ref(),
    ));
    d
}

/// One `disk` device for each volume, named `podvm-volN`.
pub fn disk_devices(volumes: &[(String, String, bool)]) -> pod_incus::Devices {
    volumes
        .iter()
        .enumerate()
        .map(|(i, (host, guest, ro))| {
            let mut d = BTreeMap::new();
            d.insert("type".to_string(), "disk".to_string());
            d.insert("source".to_string(), host.clone());
            d.insert("path".to_string(), guest.clone());
            if *ro {
                d.insert("readonly".to_string(), "true".to_string());
            }
            (format!("podvm-vol{i}"), d)
        })
        .collect()
}

/// The socket of an incusd: `PODVM_INCUS_SOCKET`, else `INCUS_DIR`'s, else
/// the system daemon's.
pub fn socket_path() -> PathBuf {
    if let Some(p) = std::env::var_os("PODVM_INCUS_SOCKET") {
        return PathBuf::from(p);
    }
    if let Some(d) = std::env::var_os("INCUS_DIR") {
        return PathBuf::from(d).join("unix.socket");
    }
    PathBuf::from("/var/lib/incus/unix.socket")
}

/// The alias of the image in incus: imported now where it is not there.
async fn ensure_image(c: &Client, job: &Job<'_>) -> Result<(String, bool), String> {
    let me = std::fs::read("/proc/self/exe").map_err(|e| format!("/proc/self/exe: {e}"))?;
    let name = alias(job.digest, &crate::machine::guest::kernel::sha256_hex(&me));
    if c.alias(&name).await.map_err(|e| e.to_string())?.is_some() {
        return Ok((name, true));
    }
    // TODO/machine.md T-1997 (PodUtils/podrunner#316): the tarball goes to
    // incusd through a pipe, in chunks, as a thread writes it: no tarball
    // file in the store beside the tree and the image of incusd.
    let (r, w) = std::io::pipe().map_err(|e| format!("a pipe for the import: {e}"))?;
    let rootfs = job.rootfs.to_path_buf();
    let writer = std::thread::spawn(move || unified_tarball_to(&rootfs, w, &me));
    let reader = tokio::fs::File::from_std(std::fs::File::from(std::os::fd::OwnedFd::from(r)));
    let mut import = ImageImport::new(ImageData::Unified(Payload::new(reader, None)));
    import.aliases = vec![name.clone()];
    let done = c.import_image(import).await;
    // The writer ends at the end of the tree, or with EPIPE where incusd
    // stopped reading; incusd's own error is the one that names the cause.
    let wrote = writer
        .join()
        .unwrap_or_else(|_| Err("the writer of the tarball ended with a panic".into()));
    done.map_err(|e| e.to_string())?;
    wrote?;
    Ok((name, false))
}

/// A file or a directory in the instance, owned by 0 with `mode`.
fn owned(mut w: FileWrite, mode: u32) -> FileWrite {
    w.uid = Some(0);
    w.gid = Some(0);
    w.mode = Some(mode);
    w
}

/// The environment as incus takes it.
pub fn env_map(env: &[String]) -> BTreeMap<String, String> {
    env.iter()
        .filter_map(|e| e.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// The command as it runs: the argv, or, with stdin, through `/bin/sh`
/// with the stdin file.
pub fn command(argv: &[String], stdin: bool) -> Vec<String> {
    if !stdin {
        return argv.to_vec();
    }
    let mut v = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        "exec \"$@\" </.podvm/stdin".to_string(),
        "sh".to_string(),
    ];
    v.extend(argv.iter().cloned());
    v
}

pub fn run(job: &Job) -> i32 {
    let fail = |e: String| {
        eprintln!("podvm {}: incus tier: {e}", job.verb);
        EXIT_RUNTIME_ERROR
    };
    let sock = socket_path();
    if !sock.exists() {
        return fail(format!(
            "no incusd answers at {}: set PODVM_INCUS_SOCKET or INCUS_DIR, or start one \
             (docs/incus.md, route A)",
            sock.display()
        ));
    }
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => return fail(format!("the runtime: {e}")),
    };
    rt.block_on(run_async(job, sock))
}

async fn run_async(job: &Job<'_>, sock: PathBuf) -> i32 {
    let fail = |e: String| {
        eprintln!("podvm {}: incus tier: {e}", job.verb);
        EXIT_RUNTIME_ERROR
    };
    let mut settings = Settings::new(sock.clone());
    settings.user_agent = format!("podvm/{}", env!("CARGO_PKG_VERSION"));
    let c = Client::new(settings);
    let server = match c.server().await {
        Ok(s) => s,
        Err(e) => return fail(e.to_string()),
    };
    if let Err(e) = server.check() {
        return fail(e.to_string());
    }
    let (image, cached) = match ensure_image(&c, job).await {
        Ok(a) => a,
        Err(e) => return fail(e),
    };
    eprintln!(
        "podvm {}: incus tier: incusd {} at {}, image {image} {} (TODO/machine.md T-1718)",
        job.verb,
        server.environment.server_version,
        sock.display(),
        if cached { "from incus" } else { "imported now" }
    );
    if let Some(d) = &job.detach {
        return super::verbs::detached(&c, job, &image, d).await;
    }
    let nonce =
        crate::machine::guest::init::nonce().unwrap_or_else(|_| format!("{}", std::process::id()));
    let inst = format!("podvm-{}", &nonce[..nonce.len().min(12)]);
    let code = run_in(&c, job, &image, &inst).await;
    // As under --rm: the instance goes, whatever the command gave; one that
    // stays is named (TODO/machine.md T-1846).
    let left = super::verbs::discard(&c, &inst).await;
    if !left.is_empty() {
        eprintln!("podvm {}: incus tier{left}", job.verb);
    }
    code
}

async fn run_in(c: &Client, job: &Job<'_>, image: &str, inst: &str) -> i32 {
    let fail = |e: String| {
        eprintln!("podvm {}: incus tier: {e}", job.verb);
        EXIT_RUNTIME_ERROR
    };
    let mut new = NewInstance::container(inst, Source::Alias(image.to_string()));
    new.devices = job_devices(job);
    if let Err(e) = apply_limits(c, &mut new, &job.limits).await {
        return fail(e);
    }
    new.config.insert(
        "raw.lxc".into(),
        "lxc.init.cmd = /.podvm/podvm guest-hold".into(),
    );
    if let Err(e) = super::net::create(c, &new, job.net_none, job.channel.as_ref()).await {
        return fail(e);
    }
    // podvm is in the image (T-1722): a run pushes only its stdin.
    let mut pushes = Vec::new();
    if let Some(b) = &job.stdin {
        pushes.push((
            super::STDIN,
            owned(FileWrite::file(Payload::bytes(b.clone())), 0o600),
        ));
    }
    for (path, w) in pushes {
        if let Err(e) = c.put_file(inst, path, w).await {
            return fail(e.to_string());
        }
    }
    if let Err(e) = super::net::push_port(c, inst, job.channel.as_ref()).await {
        return fail(e);
    }
    if let Err(e) = c.start(inst).await {
        return fail(e.to_string());
    }
    if let Err(e) = size_held(c, inst, &job.limits).await {
        return fail(e); // T-1964
    }
    if let Err(e) = super::net::wait_ready(c, inst, job.channel.as_ref()).await {
        return fail(e);
    }
    let mut exec = Exec::new(command(&job.argv, job.stdin.is_some()));
    exec.environment = env_map(&job.env);
    exec.cwd = job.workdir.clone();
    let out = match c.exec_recorded(inst, &exec).await {
        Ok(o) => o,
        Err(e) => return fail(e.to_string()),
    };
    // TODO/supervise.md T-1969: the OOM kills of the instance go to the run
    // report, read before the instance goes. A metrics endpoint that fails
    // or names no sample leaves the optional field out, never a zero.
    if crate::run::report::wanted()
        && let Ok(Some(n)) = c.oom_kills(inst).await
    {
        crate::run::cgroup::set_guest_oom_kills(n);
    }
    let _ = std::io::stdout().write_all(&out.stdout);
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().write_all(&out.stderr);
    out.exit_code
}
