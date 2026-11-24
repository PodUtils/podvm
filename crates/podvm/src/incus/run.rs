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
//! 3. The command: the exec API with its websockets ([`super::attach`],
//!    TODO/machine.md T-2080): stdout and stderr come apart as they come,
//!    piped stdin goes in, a signal goes to the command, and podvm in the
//!    image starts it ([`super::guest_exec`]).
//! 4. The end: the instance is deleted, as under `--rm`.
//!
//! A `-v HOST:GUEST[:ro]` is a `disk` device of the instance
//! ([`disk_devices`], TODO/machine.md T-1722): incusd maps the host
//! directory into the instance, so a write lands on the host.
//!
//! The command is not PID 1 here (the hold is); the parity note says so.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pod_incus::{Client, Exec, NewInstance, Settings, Source};
use podvm_image::error::EXIT_RUNTIME_ERROR;

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
    /// The store of the image: the lock of its import (T-2081).
    pub store: Option<&'a podvm_image::Store>,
    /// `--name` of a run in the foreground: the name of its instance
    /// (TODO/machine.md T-2080, OR-B39-07).
    pub name: Option<String>,
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

/// A failure of a step: its line, and a second line (T-2078) where one
/// names more of the cause.
pub(super) type Fault = (String, Option<String>);

/// The environment as incus takes it.
pub fn env_map(env: &[String]) -> BTreeMap<String, String> {
    env.iter()
        .filter_map(|e| e.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
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

/// Print the line of `fault` and its second line (T-2078); the exit of a
/// runtime error.
pub(super) fn fail_twice(verb: &str, fault: Fault) -> i32 {
    eprintln!("podvm {verb}: incus tier: {}", fault.0);
    if let Some(h) = fault.1 {
        eprintln!("podvm {verb}: incus tier: {h}");
    }
    EXIT_RUNTIME_ERROR
}

async fn run_async(job: &Job<'_>, sock: PathBuf) -> i32 {
    let fail = |e: String| {
        eprintln!("podvm {}: incus tier: {e}", job.verb);
        EXIT_RUNTIME_ERROR
    };
    // TODO/machine.md T-2080 (OR-B39-07): `--name` of a run in the
    // foreground names its instance; a name that incus does not take is
    // refused before each call.
    if let (None, Some(n)) = (&job.detach, &job.name)
        && !super::verbs::valid_name(n)
    {
        eprintln!(
            "podvm {}: --name {n:?}: incus takes a letter first, then letters, digits \
             and '-', at most 63 (TODO/machine.md T-1722)",
            job.verb
        );
        return podvm_image::error::EXIT_FLAG_ERROR;
    }
    // TODO/machine.md T-2080 (OR-B39-03): the signals of a run in the
    // foreground go to its command; blocked here, before each thread of the
    // run starts. Before the instance exists, a signal ends podvm.
    let mut sig = (job.detach.is_none()).then(|| super::attach::Signals::open(job.verb));
    let mut settings = Settings::new(sock.clone());
    settings.user_agent = format!("podvm/{}", env!("CARGO_PKG_VERSION"));
    let c = Client::new(settings);
    let prepare = async {
        let server = super::verbs::first_answer(&c, &sock)
            .await
            .map_err(|e| (e, None))?;
        server.check().map_err(|e| (e.to_string(), None))?;
        let (image, cached) =
            super::import::ensure_image(&c, job, &server.environment.storage).await?;
        Ok::<_, Fault>((server.environment.server_version, image, cached))
    };
    let prepared = match sig.as_mut() {
        Some(sig) => tokio::select! {
            r = prepare => r,
            s = sig.next() => return super::attach::before_command(job.verb, s),
        },
        None => prepare.await,
    };
    let (version, image, cached) = match prepared {
        Ok(p) => p,
        Err(f) => return fail_twice(job.verb, f),
    };
    eprintln!(
        "podvm {}: incus tier: incusd {version} at {}, image {image} {} (TODO/machine.md T-1718)",
        job.verb,
        sock.display(),
        if cached { "from incus" } else { "imported now" }
    );
    let Some(sig) = sig.as_mut() else {
        return match &job.detach {
            Some(d) => super::verbs::detached(&c, job, &image, d).await,
            None => fail("no reader of the signals".to_string()),
        };
    };
    let inst = match &job.name {
        Some(n) => n.clone(),
        None => {
            let nonce = crate::machine::guest::init::nonce()
                .unwrap_or_else(|_| format!("{}", std::process::id()));
            format!("podvm-{}", &nonce[..nonce.len().min(12)])
        }
    };
    let (code, made) = run_in(&c, job, &image, &inst, sig).await;
    // As under --rm: the instance goes, whatever the command gave; one that
    // stays is named (TODO/machine.md T-1846). An instance of another run
    // with the name of `--name` is not this run's (T-2080).
    if made {
        let left = super::verbs::discard(&c, &inst).await;
        if !left.is_empty() {
            eprintln!("podvm {}: incus tier: {left}", job.verb);
        }
    }
    code
}

/// The command of `job` in a new instance `inst`: its exit code, and
/// whether the instance may be this run's (the caller removes it).
async fn run_in(
    c: &Client,
    job: &Job<'_>,
    image: &str,
    inst: &str,
    sig: &mut super::attach::Signals,
) -> (i32, bool) {
    let fail = |e: String| {
        eprintln!("podvm {}: incus tier: {e}", job.verb);
        EXIT_RUNTIME_ERROR
    };
    let mut new = NewInstance::container(inst, Source::Alias(image.to_string()));
    new.devices = job_devices(job);
    if let Err(e) = apply_limits(c, &mut new, &job.limits).await {
        return (fail(e), false);
    }
    new.config.insert(
        "raw.lxc".into(),
        "lxc.init.cmd = /.podvm/podvm guest-hold".into(),
    );
    let setup = async {
        // T-2080 (OR-B39-07): docker's refusal of a name in use, before the
        // create; incusd's own 409 stays for a race.
        if job.name.is_some() {
            match c.instance(inst).await {
                Ok(_) => {
                    let e = format!(
                        "the name {inst:?} is in use by an instance of incus: remove it \
                         (`podvm rm -f --podvm-tier incus {inst}` for an instance of podvm), \
                         or give another --name (TODO/machine.md T-2080)"
                    );
                    return Err((fail(e), false));
                }
                Err(e) if super::verbs::gone(&e) => {}
                Err(e) => return Err((fail(e.to_string()), false)),
            }
        }
        if let Err((e, t)) = super::net::create(c, &new, job.net_none, job.channel.as_ref()).await {
            // T-2080: an instance of the name of `--name` that was there
            // is another one's; the instance of a name of podvm may be
            // half made.
            let mine = job.name.is_none() || !e.contains("already exists");
            let hint = super::space::after_failure(c, job.rootfs, false, t).await;
            return Err((fail_twice(job.verb, (e, hint)), mine));
        }
        let step = async {
            super::net::push_port(c, inst, job.channel.as_ref()).await?;
            c.start(inst).await.map_err(|e| e.to_string())?;
            size_held(c, inst, &job.limits).await?; // T-1964
            super::net::wait_ready(c, inst, job.channel.as_ref()).await
        };
        step.await.map_err(|e| (fail(e), true))
    };
    tokio::select! {
        r = setup => if let Err((code, mine)) = r {
            return (code, mine);
        },
        s = sig.next() => return (super::attach::before_command(job.verb, s), true),
    }
    // T-2080: podvm in the image starts the command in its working
    // directory, which it makes where it is missing (OR-B39-09), and names
    // the cause of a start that fails (OR-B39-01, OR-B39-16).
    let mut exec = Exec::new(super::guest_exec::wrap(&job.argv, &job.workdir));
    exec.environment = env_map(&job.env);
    let input = job.stdin.as_deref().unwrap_or_default();
    let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
    let r = super::attach::exec_streamed(c, job.verb, inst, &exec, input, sig, &mut out, &mut err)
        .await;
    // TODO/supervise.md T-1969: the OOM kills of the instance go to the run
    // report, read before the instance goes. A metrics endpoint that fails
    // or names no sample leaves the optional field out, never a zero.
    if crate::run::report::wanted()
        && let Ok(Some(n)) = c.oom_kills(inst).await
    {
        crate::run::cgroup::set_guest_oom_kills(n);
    }
    match r {
        Ok(o) => (o.code, true),
        Err(e) => (fail(e), true),
    }
}
