//! The verbs of an incus instance that stays, TODO/machine.md T-1722.
//!
//! incusd keeps the state of the instance; podvm keeps no record of it.
//! A record of podvm's container table belongs to a launcher, and
//! `reconcile` calls a running record with no launcher dead, so a second
//! copy of the state would lie. An instance of podvm has the label
//! [`LABEL`] and the image of the caller in [`IMAGE_KEY`].
//!
//! * `run -d` and `create` make the instance with podvm as its init
//!   (`guest-hold --run`) and the command in `/.podvm/cmd`
//!   ([`super::Cmd`]); `run -d` starts it. The command's output goes to
//!   `/.podvm/log` and its exit code to `/.podvm/exit`; the instance stops
//!   when the command ends.
//! * `ps`, `start`, `stop`, `rm`, `logs` and `exec` with
//!   `--podvm-tier=incus` call the API on the instance of that name.

use std::io::Write;

use pod_incus::{Client, FileWrite, NewInstance, Payload, Settings, Source};
use podvm_image::error::EXIT_RUNTIME_ERROR;

use super::run::{job_devices, socket_path, Job};

/// The label of an instance of podvm.
pub const LABEL: &str = "user.podvm";
/// The image as the caller named it.
pub const IMAGE_KEY: &str = "user.podvm.image";
/// The init of an instance that stays.
pub const INIT: &str = "lxc.init.cmd = /.podvm/podvm guest-hold --run";

/// `run -d` (start) or `create` (no start), with the name of `--name`.
#[derive(Debug, Clone)]
pub struct Detach {
    pub name: Option<String>,
    pub start: bool,
    pub image: String,
    /// `-i`: the piped stdin goes to the command (TODO/machine.md T-1777).
    pub stdin: bool,
    /// `--podvm-init=image`: the init of the image is PID 1 (T-1993).
    pub init_image: bool,
    /// `--rm` (TODO/machine.md T-2080, OR-B39-07): an ephemeral instance,
    /// which incusd removes when it stops.
    pub rm: bool,
}

/// The arguments without `--podvm-tier=incus` (or `--podvm-tier incus`),
/// where they hold it.
pub fn wants(args: &[String]) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut found = false;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--podvm-tier=incus" {
            found = true;
        } else if args[i] == "--podvm-tier" && args.get(i + 1).is_some_and(|v| v == "incus") {
            found = true;
            i += 1;
        } else {
            out.push(args[i].clone());
        }
        i += 1;
    }
    found.then_some(out)
}

/// A valid incus name: a letter first, then letters, digits and `-`, at
/// most 63.
pub fn valid_name(n: &str) -> bool {
    n.len() <= 63
        && n.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// TODO/machine.md T-2080 (OR-B39-10): the bound of the first answer of
/// incusd. A socket that takes the connection and never answers held
/// podvm with no bound (90 s and more). Since podcrates `v0.83.0` (#653)
/// `Settings::answer_timeout` of pod-incus bounds each silence of each
/// answer; this is its value, for the line.
pub const FIRST_ANSWER: std::time::Duration = std::time::Duration::from_secs(30);

/// `GET /1.0`; a silence of incusd past [`FIRST_ANSWER`] gives one line
/// that names the socket.
pub(crate) async fn first_answer(
    c: &Client,
    sock: &std::path::Path,
) -> Result<pod_incus::Server, String> {
    match c.server().await {
        Ok(s) => Ok(s),
        Err(e) if matches!(e.kind(), pod_incus::ErrorKind::AnswerTimeout { .. }) => Err(format!(
            "the socket {} took the connection and gave no answer to GET /1.0 within {} s: \
             incusd hangs, or another program holds the socket (TODO/machine.md T-2080)",
            sock.display(),
            FIRST_ANSWER.as_secs()
        )),
        Err(e) => Err(e.to_string()),
    }
}

/// A client of the incusd of [`socket_path`], checked.
pub(crate) async fn connect() -> Result<(Client, String), String> {
    let sock = socket_path();
    if !sock.exists() {
        return Err(format!(
            "no incusd answers at {}: set PODVM_INCUS_SOCKET or INCUS_DIR, or start one \
             (docs/incus.md, route A)",
            sock.display()
        ));
    }
    let mut settings = Settings::new(sock.clone());
    settings.user_agent = format!("podvm/{}", env!("CARGO_PKG_VERSION"));
    // T-2080: each silence of incusd ends after this bound (podcrates
    // `v0.83.0`, #653).
    settings.answer_timeout = Some(super::verbs::FIRST_ANSWER);
    let c = Client::new(settings);
    let server = first_answer(&c, &sock).await?;
    server.check().map_err(|e| e.to_string())?;
    Ok((c, server.environment.server_version.clone()))
}

pub(crate) fn block_on<F: std::future::Future<Output = i32>>(verb: &str, f: F) -> i32 {
    match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt.block_on(f),
        Err(e) => {
            eprintln!("podvm {verb}: incus tier: the runtime: {e}");
            EXIT_RUNTIME_ERROR
        }
    }
}

/// Make (and for `run -d` start) the instance of `job`; print its name.
pub(super) async fn detached(c: &Client, job: &Job<'_>, image: &str, d: &Detach) -> i32 {
    let fail = |e: String| {
        eprintln!("podvm {}: incus tier: {e}", job.verb);
        EXIT_RUNTIME_ERROR
    };
    let name = match &d.name {
        Some(n) if valid_name(n) => n.clone(),
        Some(n) => {
            eprintln!(
                "podvm {}: --name {n:?}: incus takes a letter first, then letters, digits \
                 and '-', at most 63 (TODO/machine.md T-1722)",
                job.verb
            );
            return podvm_image::error::EXIT_FLAG_ERROR;
        }
        None => {
            let nonce = crate::machine::guest::init::nonce()
                .unwrap_or_else(|_| format!("{}", std::process::id()));
            format!("podvm-{}", &nonce[..nonce.len().min(12)])
        }
    };
    let mut new = NewInstance::container(&name, Source::Alias(image.to_string()));
    new.devices = job_devices(job);
    new.ephemeral = d.rm; // T-2080
    if let Err(e) = super::run::apply_limits(c, &mut new, &job.limits).await {
        return fail(e); // T-1897, T-1964
    }
    if !d.init_image {
        new.config.insert("raw.lxc".into(), INIT.into());
    } else if std::path::Path::new("/dev/fuse").exists() {
        // T-1996: a host with no FUSE starts the instance with no device.
        let (k, v) = super::run::fuse_device();
        new.devices.insert(k, v);
    }
    new.config.insert(LABEL.into(), "1".into());
    new.config.insert(IMAGE_KEY.into(), d.image.clone());
    if let Err((e, t)) = super::net::create(c, &new, job.net_none, job.channel.as_ref()).await {
        // T-2078: where the transport failed, the free space of the pool.
        let hint = super::space::after_failure(c, job.plan().as_ref(), false, t).await;
        return super::run::fail_twice(job.verb, (super::run::ids_hint(e, d.init_image), hint));
    }
    let cmd = super::Cmd {
        argv: job.argv.clone(),
        env: job.env.clone(),
        cwd: job.workdir.clone(),
    };
    let mut w = FileWrite::file(Payload::bytes(cmd.to_json()));
    w.uid = Some(0);
    w.gid = Some(0);
    w.mode = Some(0o600);
    if let Err(e) = c.put_file(&name, super::CMD, w).await {
        let _ = c.delete_instance(&name).await;
        return fail(e.to_string());
    }
    // TODO/machine.md T-1777 (PodUtils/podrunner#85): with `-i`, the piped
    // stdin goes in as a file of root, mode 0600, before the start; never
    // in argv or the environment.
    if d.stdin
        && let Some(b) = &job.stdin
    {
        let mut w = FileWrite::file(Payload::bytes(b.clone()));
        (w.uid, w.gid, w.mode) = (Some(0), Some(0), Some(0o600));
        if let Err(e) = c.put_file(&name, super::STDIN, w).await {
            let _ = c.delete_instance(&name).await;
            return fail(e.to_string());
        }
    }
    // T-1996: an empty machine-id, so that systemd writes a new one in
    // each instance of the same image (the snapshot ships it empty).
    if d.init_image {
        let mut w = FileWrite::file(Payload::bytes(Vec::new()));
        (w.uid, w.gid, w.mode) = (Some(0), Some(0), Some(0o444));
        if let Err(e) = c.put_file(&name, "/etc/machine-id", w).await {
            let _ = c.delete_instance(&name).await;
            return fail(format!("/etc/machine-id: {e}"));
        }
    }
    if let Err(e) = super::net::push_port(c, &name, job.channel.as_ref()).await {
        let _ = c.delete_instance(&name).await;
        return fail(e);
    }
    if d.start
        && let Err(e) = c.start(&name).await
    {
        // TODO/machine.md T-1846: a failed start leaves no instance.
        let left = discard(c, &name).await;
        return fail(with_left(e, &left));
    }
    if d.start
        && let Err(e) = super::run::size_held(c, &name, &job.limits).await
    {
        let left = discard(c, &name).await;
        return fail(with_left(e, &left));
    }
    if d.start
        && d.init_image
        && let Err(down) = super::systemd::boot_then_run(c, &name).await
    {
        if down.keep {
            // PodUtils/podrunner#349 ask 3 (TODO/machine.md T-2083): the
            // instance stays, so that the fault is not lost with it.
            return fail(format!(
                "{}; the instance {name} stays for a look (`incus console {name} --show-log`, \
                 `incus exec {name} -- systemctl status`): `podvm rm -f --podvm-tier incus \
                 {name}` removes it",
                down.line
            ));
        }
        let left = discard(c, &name).await;
        return fail(with_left(down.line, &left));
    }
    println!("{name}");
    0
}

/// `ps`, `start`, `stop` and `rm` with `--podvm-tier=incus`; `logs` is
/// [`super::logs`].
pub fn dispatch(verb: &str, args: &[String]) -> i32 {
    let flags: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| a.starts_with('-'))
        .collect();
    let names: Vec<String> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .cloned()
        .collect();
    let known: &[&str] = match verb {
        "ps" => &["-a", "--all"],
        "rm" => &["-f", "--force"],
        _ => &[],
    };
    if let Some(f) = flags.iter().find(|f| !known.contains(f)) {
        eprintln!(
            "podvm {verb}: {f} is not served with --podvm-tier=incus (TODO/machine.md T-1722)"
        );
        return podvm_image::error::EXIT_FLAG_ERROR;
    }
    if verb != "ps" && names.is_empty() {
        eprintln!("podvm {verb}: name an instance");
        return podvm_image::error::EXIT_FLAG_ERROR;
    }
    let all = flags.iter().any(|f| *f == "-a" || *f == "--all");
    let force = flags.iter().any(|f| *f == "-f" || *f == "--force");
    block_on(verb, async move {
        let c = match connect().await {
            Ok((c, _)) => c,
            Err(e) => {
                eprintln!("podvm {verb}: incus tier: {e}");
                return EXIT_RUNTIME_ERROR;
            }
        };
        if verb == "ps" {
            return ps(&c, all).await;
        }
        let mut code = 0;
        for n in &names {
            if let Err(e) = one(&c, verb, n, force).await {
                eprintln!("podvm {verb}: {n}: {e}");
                code = 1;
            }
        }
        code
    })
}

/// Whether `name` is an instance of podvm in the incusd of
/// [`socket_path`] (the routing of `tier = incus`, [`super::config`]).
pub fn is_instance(name: &str) -> bool {
    block_on("incus", async {
        match connect().await {
            Ok((c, _)) => i32::from(matches!(c.instance(name).await, Ok(i) if ours(&i))),
            Err(_) => 0,
        }
    }) == 1
}

/// Whether `inst` is an instance of podvm.
pub(crate) fn ours(i: &pod_incus::Instance) -> bool {
    i.config.get(LABEL).is_some_and(|v| v == "1")
}

/// TODO/machine.md T-1846: the rows of `podvm ps` for the instances of
/// podvm, in the columns of the containers: each one with `all`, else the
/// running ones.
pub(super) async fn rows(c: &Client, all: bool) -> Result<Vec<Vec<String>>, String> {
    let list = c.instances().await.map_err(|e| e.to_string())?;
    Ok(list
        .iter()
        .filter(|i| ours(i) && (all || i.status == "Running"))
        .map(|i| {
            let status = if i.status == "Running" {
                "Up".to_string()
            } else {
                format!("Exited ({})", i.status)
            };
            vec![
                i.name.clone(),
                i.config
                    .get(IMAGE_KEY)
                    .cloned()
                    .unwrap_or_else(|| "-".into()),
                "(incus instance)".to_string(),
                if i.created_at.is_empty() {
                    "-".into()
                } else {
                    i.created_at.clone()
                },
                status,
                i.name.clone(),
            ]
        })
        .collect())
}

/// The rows of `podvm ps` for the instances of podvm where the
/// configuration selects the incus tier and its incusd answers; none else.
pub fn ps_rows(all: bool) -> Vec<Vec<String>> {
    if !matches!(
        super::config::configured(),
        Ok(super::config::Configured::Incus { socket: true })
    ) {
        return Vec::new();
    }
    let out = std::sync::Mutex::new(Vec::new());
    block_on("ps", async {
        match connect().await {
            Ok((c, _)) => match rows(&c, all).await {
                Ok(r) => {
                    *out.lock().unwrap_or_else(|e| e.into_inner()) = r;
                    0
                }
                Err(e) => {
                    eprintln!("podvm ps: incus tier: {e}");
                    1
                }
            },
            Err(e) => {
                eprintln!("podvm ps: incus tier: {e}");
                1
            }
        }
    });
    out.into_inner().unwrap_or_default()
}

async fn ps(c: &Client, all: bool) -> i32 {
    let list = match c.instances().await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("podvm ps: incus tier: {e}");
            return EXIT_RUNTIME_ERROR;
        }
    };
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{:<24} {:<10} IMAGE", "NAMES", "STATUS");
    for i in list.iter().filter(|i| ours(i)) {
        if all || i.status == "Running" {
            let image = i.config.get(IMAGE_KEY).map(String::as_str).unwrap_or("-");
            let _ = writeln!(out, "{:<24} {:<10} {image}", i.name, i.status);
        }
    }
    0
}

pub(super) async fn one(c: &Client, verb: &str, name: &str, force: bool) -> Result<(), String> {
    let i = c.instance(name).await.map_err(|e| e.to_string())?;
    if !ours(&i) {
        return Err("not an instance of podvm (no user.podvm label)".into());
    }
    let running = i.status == "Running";
    match verb {
        "start" if !running => {
            c.start(name).await.map_err(|e| e.to_string())?;
        }
        "stop" if running => {
            c.stop(name, false, Some(std::time::Duration::from_secs(10)))
                .await
                .map_err(|e| e.to_string())?;
        }
        "rm" => {
            // TODO/machine.md T-1846: the state of the instance, as incus
            // names it (an ERROR instance does not run).
            if running && !force {
                return Err("the instance is Running: stop it, or rm -f".into());
            }
            if i.status != "Stopped" {
                let _ = c
                    .stop(name, true, Some(std::time::Duration::from_secs(10)))
                    .await;
            }
            c.delete_instance(name)
                .await
                .map_err(|e| format!("the instance is {}: {e}", i.status))?;
        }
        "start" | "stop" => {}
        _ => return Err(format!("{verb} is not a verb of the incus tier")),
    }
    println!("{name}");
    Ok(())
}

/// TODO/machine.md T-1846: stop and delete an instance after a failure.
/// Empty where it went; else the text that names the instance that stays.
/// A 404 of the delete (TODO/machine.md T-2080, OR-B39-12) is an instance
/// that is not there: nothing stays.
pub(super) async fn discard(c: &Client, name: &str) -> String {
    let _ = c
        .stop(name, true, Some(std::time::Duration::from_secs(10)))
        .await;
    match c.delete_instance(name).await {
        Ok(_) => String::new(),
        Err(e) if gone(&e) => String::new(),
        Err(e) => {
            format!("the instance {name} stays ({e}): `incus delete --force {name}` removes it")
        }
    }
}

/// An error of incusd that says that the instance is not there.
pub(crate) fn gone(e: &pod_incus::Error) -> bool {
    e.status() == Some(404)
}

/// The error `e` and the text of [`discard`], where one stays.
pub(super) fn with_left(e: impl std::fmt::Display, left: &str) -> String {
    if left.is_empty() {
        e.to_string()
    } else {
        format!("{e}; {left}")
    }
}
