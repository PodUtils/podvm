//! TODO/machine.md T-1656 and T-1718. `run --podvm-tier=machine` and
//! `run --podvm-tier=incus`: the image in the store, extracted, and run in
//! a guest of podvm ([`crate::machine::guest`]) or in an incus instance
//! ([`crate::incus`]).
//!
//! The command, the environment and the working directory come from the
//! image config and the flags, as on each other tier. The run makes no
//! container record: the guest or the instance is disposable, as under
//! `--rm`.

use super::*;

type Guest = fn(&str, &podvm_image::Store, &Opts, &Platform, &Policy) -> i32;

/// The `run` of a tier that runs the image in a guest, or `None`.
pub(super) fn guest_tier(tier: crate::tier::Tier) -> Option<Guest> {
    match tier {
        crate::tier::Tier::Machine => Some(machine_tier),
        crate::tier::Tier::Incus => Some(incus_tier),
        crate::tier::Tier::Runsc => Some(super::runsc::runsc_tier),
        _ => None,
    }
}

/// What both tiers take from the store and the image config.
pub(super) struct Prepared {
    pub(super) digest: String,
    pub(super) rootfs: std::path::PathBuf,
    pub(super) argv: Vec<String>,
    pub(super) env: Vec<String>,
    pub(super) workdir: String,
    pub(super) stdin: Option<Vec<u8>>,
    /// Held while the run uses the rootfs.
    _held: podvm_image::store::Lock,
}

pub(super) fn prepare_image(
    verb: &str,
    store: &podvm_image::Store,
    o: &Opts,
    platform: &Platform,
    policy: &Policy,
    tier: &str,
) -> Result<Prepared, i32> {
    let image = o.image.clone().unwrap_or_default();
    let record = acquire(verb, store, &image, platform, policy, &o.pull)?;
    crate::lifecycle::ensure_linux_guest(verb, &record.os, &record.architecture)?;
    let held = store.hold(&record).map_err(|e| {
        eprintln!("podvm {verb}: {e}");
        podvm_image::error::EXIT_RUNTIME_ERROR
    })?;
    if !podvm_extract::is_extracted(store, &record.manifest_digest) {
        extract_now(verb, store, &record, o.ask.verbose)?;
    }
    let (rootfs, _) = podvm_extract::paths(store, &record.manifest_digest);
    let cfg = config_of(verb, store, &record)?;
    let (image_entry, entry) = super::entrypoints(cfg.config.entrypoint.as_deref(), o);
    let argv = Plan::argv_for(image_entry, cfg.config.cmd.as_deref(), &o.command, entry);
    let argv = super::wrapped(o, argv); // T-1654: `fs.wrap`
    if argv.is_empty() {
        eprintln!("{}", super::nothing_to_run(verb, &image, o.init_image));
        return Err(EXIT_FLAG_ERROR);
    }
    // TODO/machine.md T-1823: an absolute program that the tree lacks
    // refuses before the boot with docker's 127, not with a line of the
    // guest shell. Each link resolves inside the tree, as in the guest.
    let maps = podvm_enter::userland::Maps::of_root(&rootfs.to_string_lossy());
    if let Some(prog) = argv.first().filter(|p| p.starts_with('/'))
        && podvm_enter::confine::host_resolved(&maps, prog).is_none()
    {
        eprintln!("podvm {verb}: {tier} tier: exec: {prog:?}: no such file in the image");
        return Err(127);
    }
    let workdir = o
        .workdir
        .clone()
        .unwrap_or_else(|| cfg.config.working_dir.clone());
    let workdir = if workdir.is_empty() {
        "/".to_string()
    } else {
        workdir
    };
    let env = Plan::env_for(&podvm_enter::NAMES, &cfg.config.env, &o.env);
    // TODO/machine.md T-1885: with -t the guest's terminal takes stdin
    // live (`machine/guest/tty.rs`); a read here left the pump nothing but
    // the end of input.
    let stdin = if o.tty && tier == "machine" {
        None
    } else {
        crate::machine::guest::read_stdin().map_err(|e| {
            eprintln!("podvm {verb}: {tier} tier: {e}");
            podvm_image::error::EXIT_RUNTIME_ERROR
        })?
    };
    Ok(Prepared {
        digest: record.manifest_digest.clone(),
        rootfs,
        argv,
        env,
        workdir,
        stdin,
        _held: held,
    })
}

/// The machine tier of `run` and `create`: `run` boots a guest; `create`
/// keeps a record of no guest, so it reports the profile and exits 125.
pub(super) fn machine_tier(
    verb: &str,
    store: &podvm_image::Store,
    o: &Opts,
    platform: &Platform,
    policy: &Policy,
) -> i32 {
    if verb != "run" {
        return crate::tier::enter_machine(verb, o.mem);
    }
    if let Some(m) = o.mem
        && let Ok((cur, _)) = podvm_probe::sys::prlimit(podvm_probe::sys::RLIMIT_FSIZE)
        && crate::tier::mem_over_ceiling(m, cur)
    {
        eprintln!(
            "podvm {verb}: machine tier refused: guest memory {m} bytes over the \
             RLIMIT_FSIZE ceiling {cur} bytes (prlimit(RLIMIT_FSIZE) cur={cur})"
        );
        return podvm_image::error::EXIT_RUNTIME_ERROR;
    }
    // TODO/machine.md T-1808: a flag that the guest does not honour is
    // refused by name before the boot, as the incus tier does. The payload
    // ran as root with each capability where the caller asked otherwise.
    let unhonoured = unhonoured_flags(o);
    if !unhonoured.is_empty() {
        eprintln!(
            "podvm {verb}: machine tier refused: the guest does not honour {} yet; podvm \
             refuses each by name and does not drop it (TODO/machine.md T-1808)",
            unhonoured.join(", ")
        );
        return EXIT_FLAG_ERROR;
    }
    // TODO/machine.md T-1889: the guest kernel is x86_64, so an image of
    // another architecture refuses by name before the boot. It booted and
    // ended with "the guest gave no exit marker" (PodUtils/podrunner#102).
    if let Some(why) = foreign_guest(platform) {
        eprintln!("podvm {verb}: machine tier refused: {why}");
        return EXIT_FLAG_ERROR;
    }
    // TODO/machine.md T-1751: a sealed run takes each value of the run
    // from the bundle; a flag would put it in the argv of podvm.
    if o.sealed
        && (!o.command.is_empty()
            || !o.env.is_empty()
            || o.workdir.is_some()
            || !o.volumes.is_empty()
            || o.tty
            || o.broker)
    {
        eprintln!(
            "podvm {verb}: --podvm-sealed takes the command, the environment, the working \
             directory and the files from the bundle on stdin: it refuses a command, -e, -w, \
             -v, -t and --network broker (TODO/machine.md T-1751)"
        );
        return EXIT_FLAG_ERROR;
    }
    // TODO/machine.md T-1750: `run -d` starts a keeper of the guest; the
    // keeper is this same run, in the foreground.
    let keeper = crate::machine::detach::keeper();
    if keeper.is_some() {
        crate::machine::guest::signals::block_early(); // T-2042: `kill` of a boot
    }
    if o.detach && keeper.is_none() {
        let image = o.image.as_deref().unwrap_or_default();
        return crate::machine::detach::start(verb, o.name.as_deref(), image, o.mem, o.timeout);
    }
    let mut p = match prepare_image(verb, store, o, platform, policy, "machine") {
        Ok(p) => p,
        Err(c) => return c,
    };
    let sealed = if o.sealed {
        let Some(b) = p.stdin.take() else {
            eprintln!(
                "podvm {verb}: --podvm-sealed reads the bundle of the run from stdin, and \
                 stdin is not a pipe or a file (a terminal, or closed) (TODO/machine.md T-1751, \
                 T-1810)"
            );
            return EXIT_FLAG_ERROR;
        };
        if let Err(why) = crate::machine::guest::sealed::check(&b) {
            eprintln!("podvm {verb}: --podvm-sealed: {why} (TODO/machine.md T-1751)");
            return EXIT_FLAG_ERROR;
        }
        Some(b)
    } else {
        None
    };
    // `-v HOST:GUEST[:ro]`: a 9p mount of the host directory
    // (TODO/machine.md T-1723).
    let volumes: Vec<_> = o
        .volumes
        .iter()
        .map(|v| {
            (
                std::path::PathBuf::from(&v.host),
                v.guest.clone(),
                o.ro_volumes.contains(&v.guest),
            )
        })
        .collect();
    let mut copies = Vec::new();
    // TODO/probe.md T-1666: the probe of probe-inside, at /.podvm.
    let probe = crate::sandprobe::guest_copy(store.root());
    copies.extend(probe.clone().map(|d| (d, "/.podvm".to_string())));
    // TODO/net.md T-1665: a loopback proxy is the gateway in the guest.
    // TODO/machine.md T-1778: with no NIC there is no gateway, and the
    // guest's loopback is where --podvm-channel listens, so the variable
    // stays as it is.
    let rewritten = if o.net_none || o.sealed {
        Ok(p.env.clone())
    } else {
        crate::machine::proxy::rewrite_env(&p.env)
    };
    let mut env = match rewritten {
        Ok(e) => e,
        Err(why) => {
            eprintln!("podvm {verb}: machine tier: {why}");
            return EXIT_FLAG_ERROR;
        }
    };
    env.extend(crate::sandprobe::host_env(store.root()));
    let discard = crate::initns::discards(true, &o.volumes, &o.ro_volumes); // a guest keeps no root
                                                                            // A guest has the terminal of `-t` in the /dev of its init (hvc0): no pty of the init.
    crate::initns::mark_env(&mut env, o.init_image, o.ask.verbose, discard, false); // T-2002, T-2010
    if o.net_none && o.broker {
        eprintln!(
            "podvm {verb}: --podvm-net none leaves no NIC for --network broker: the broker \
             is a forward of the guest's network (TODO/machine.md T-1751)"
        );
        return EXIT_FLAG_ERROR;
    }
    // TODO/net.md T-1665: the guest reaches the broker alone, at a forward
    // of its restricted network.
    let broker = if o.broker {
        match crate::broker::start(&o.allow, &o.ports) {
            Ok(b) => {
                let (h, p) = crate::broker::GUEST_FORWARD;
                crate::broker::replace_proxy_env(&mut env, b.env(h, p));
                Some(b.port)
            }
            Err(e) => {
                eprintln!("podvm {verb}: --network broker: {e} (TODO/net.md T-1665)");
                return podvm_image::error::EXIT_RUNTIME_ERROR;
            }
        }
    } else {
        None
    };
    // T-1803: from here the guest runs the payload; a refusal before this
    // line stays a refusal in the report.
    note_guest_run("machine", o, &p);
    let loud = crate::complete::banner_loud(store, &o.ask);
    let detached = keeper.is_some();
    let code = crate::machine::guest::boot(&crate::machine::guest::Request {
        verb,
        keeper,
        arch: crate::machine::guest::arch::Arch::of(&platform.arch)
            .unwrap_or(crate::machine::guest::arch::Arch::Amd64),
        loud,
        store_root: store.root(),
        rootfs: &p.rootfs,
        digest: &p.digest,
        argv: p.argv,
        env,
        workdir: p.workdir,
        mem: o.mem,
        cpus: o.cpus,
        broker,
        timeout: o
            .timeout
            .or(detached.then_some(podvm_windows::machines::DEFAULT_LIFETIME_SECS))
            .map(std::time::Duration::from_secs),
        qemu_args: o.qemu_args.clone(),
        copies,
        volumes,
        tty: o.tty,
        stdin: p.stdin,
        net_none: o.net_none || o.sealed,
        channel: o.channel.clone(),
        sealed,
        engine: o.engine,
        pids: o
            .limits
            .iter()
            .find(|(_, k, _)| *k == "limits.processes")
            .map(|(_, _, v)| v.clone()),
    });
    if let Some(d) = probe {
        let _ = std::fs::remove_dir_all(d);
    }
    if detached {
        crate::machine::detach::ended(code);
    }
    code
}

/// The flags of `o` that the guest of the machine tier does not honour
/// (TODO/machine.md T-1808), in the order of the help.
/// TODO/machine.md T-1889: the refusal of an image of another
/// architecture than the guest kernel (`amd64`), or None.
pub(super) fn foreign_guest(platform: &Platform) -> Option<String> {
    crate::machine::guest::arch::Arch::of(&platform.arch)
        .is_none()
        .then(|| {
            format!(
                "the guest of the machine tier is linux/amd64 or linux/arm64 \
                 (TODO/machine.md T-1890), and --platform asks for linux/{}: no guest of that \
                 architecture is planned",
                platform.arch
            )
        })
}

pub(super) fn unhonoured_flags(o: &Opts) -> Vec<&'static str> {
    let h = &o.hardening;
    [
        (o.confine.is_some(), "--podvm-confine"),
        (o.user.is_some(), "--user"),
        (o.hostname.is_some(), "--hostname"),
        (!o.ask.add_hosts.is_empty(), "--add-host"),
        (o.read_only, "--read-only"),
        (!o.tmpfs.is_empty(), "--tmpfs"),
        (!h.ulimits.is_empty(), "--ulimit"),
        (
            h.no_new_privs,
            "--security-opt no-new-privileges or --cap-drop ALL",
        ),
        (!h.labels.is_empty(), "--label"),
        (o.volume_base.is_some(), "--podvm-volume-base"),
    ]
    .into_iter()
    .filter_map(|(given, flag)| given.then_some(flag))
    .collect()
}

/// The flags of `o` that an incus instance does not honour (TODO/gate.md
/// T-1854): those of the machine tier, and the timeout, the terminal, the
/// devices and the ports, which its job does not carry.
pub(super) fn incus_unhonoured(o: &Opts) -> Vec<&'static str> {
    let mut u = unhonoured_flags(o);
    for (given, flag) in [
        (o.timeout.is_some(), "--timeout"),
        (o.tty, "-t"),
        (!o.devices.is_empty(), "--device"),
        (!o.ports.is_empty(), "--ports"),
    ] {
        if given {
            u.push(flag);
        }
    }
    u
}

/// TODO/machine.md T-1803: the report of `--report` learns that a guest
/// runs the payload, with its image and digest.
fn note_guest_run(tier: &'static str, o: &Opts, p: &Prepared) {
    super::report::note_guest(super::report::Guest {
        tier,
        image: o.image.clone().unwrap_or_default(),
        digest: p.digest.clone(),
        timeout_s: o.timeout,
        limits: o
            .limits
            .iter()
            .map(|(_, k, v)| format!("{k}={v}"))
            .collect(),
    });
}

/// The incus tier (T-1718): `run` only; each other verb refuses by name.
pub(super) fn incus_tier(
    verb: &str,
    store: &podvm_image::Store,
    o: &Opts,
    platform: &Platform,
    policy: &Policy,
) -> i32 {
    if verb != "run" && verb != "create" {
        eprintln!(
            "podvm {verb}: --podvm-tier=incus serves run and create (TODO/machine.md T-1718, \
             T-1722)"
        );
        return EXIT_FLAG_ERROR;
    }
    if o.mem.is_some() || o.cpus.is_some() || !o.qemu_args.is_empty() || o.broker || o.sealed {
        eprintln!(
            "podvm {verb}: --podvm-tier=incus takes no --podvm-mem, --podvm-cpus, \
             --podvm-qemu-arg, --podvm-sealed or --network broker yet \
             (TODO/machine.md T-1718)"
        );
        return EXIT_FLAG_ERROR;
    }
    // TODO/gate.md T-1854: the class of T-1808 on this tier. The job of an
    // incus instance carries no user, confine layer, read-only root, tmpfs,
    // limits, labels, hosts, timeout, terminal, device or port; each is
    // refused by name, not dropped.
    let unhonoured = incus_unhonoured(o);
    if !unhonoured.is_empty() {
        eprintln!(
            "podvm {verb}: --podvm-tier=incus refused: the instance does not honour {} yet; \
             podvm refuses each by name and does not drop it (TODO/gate.md T-1854)",
            unhonoured.join(", ")
        );
        return EXIT_FLAG_ERROR;
    }
    let p = match prepare_image(verb, store, o, platform, policy, "incus") {
        Ok(p) => p,
        Err(c) => return c,
    };
    note_guest_run("incus", o, &p);
    crate::incus::run::run(&crate::incus::run::Job {
        verb,
        rootfs: &p.rootfs,
        digest: &p.digest,
        argv: p.argv,
        env: p.env,
        workdir: p.workdir,
        stdin: p.stdin,
        limits: o
            .limits
            .iter()
            .map(|(_, k, v)| (*k, v.clone()))
            .chain(crate::incus::run::engine_keys(o.engine))
            .chain(crate::incus::run::snapshot_keys(o.init_image)) // T-1996
            .collect(),
        // TODO/machine.md T-1722: create and run -d keep the instance.
        detach: (verb == "create" || o.detach).then(|| crate::incus::verbs::Detach {
            name: o.name.clone(),
            start: verb == "run",
            image: o.image.clone().unwrap_or_default(),
            stdin: o.interactive,
            init_image: o.init_image, // T-1993
        }),
        // TODO/machine.md T-1779: no NIC, and the channel as a proxy device.
        net_none: o.net_none,
        channel: o.channel.clone(),
        // TODO/machine.md T-1722: each -v is a disk device of the instance.
        volumes: o
            .volumes
            .iter()
            .map(|v| {
                (
                    v.host.clone(),
                    v.guest.clone(),
                    o.ro_volumes.contains(&v.guest),
                )
            })
            .collect(),
    })
}
