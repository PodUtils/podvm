//! The options of `run` and `create`, as `parse` fills them.

#[derive(Debug)]
pub(crate) struct Opts {
    pub(crate) rm: bool,
    pub(crate) detach: bool,
    pub(crate) name: Option<String>,
    pub(crate) env: Vec<String>,
    pub(crate) workdir: Option<String>,
    pub(crate) entrypoint: Option<String>,
    pub(crate) platform: Option<String>,
    pub(crate) pull: String,
    pub(crate) insecure: Vec<String>,
    pub(crate) tls_verify: Option<bool>,
    pub(crate) tty: bool,
    pub(crate) image: Option<String>,
    pub(crate) command: Vec<String>,
    /// `--user`. Carried raw here and resolved against the image once its
    /// rootfs exists, because a name means the image's own passwd entry.
    pub(crate) user: Option<String>,
    /// `--hostname` (TODO/enter.md T-1908).
    pub(crate) hostname: Option<String>,
    /// `--podvm-tier`. Carried raw here and resolved in `prepare`.
    /// TODO/machine.md T-1302.
    pub(crate) tier: Option<crate::tier::Want>,
    /// `--podvm-qemu-arg`. One emulator token per occurrence, carried for
    /// the machine tier's driver (T-1303); refused on every other tier
    /// rather than silently dropped.
    pub(crate) qemu_args: Vec<String>,
    /// `--podvm-mem`. The validated guest memory in bytes, carried for the
    /// machine tier's ceiling check in `prepare`; refused on every other
    /// tier rather than silently dropped. TODO/machine.md T-1305.
    pub(crate) mem: Option<u64>,
    /// `--podvm-cpus`. The guest's vCPUs, machine tier only and refused on
    /// every other tier. TODO/cli.md T-1654 (`limits.cpus` of a profile).
    pub(crate) cpus: Option<u32>,
    /// `--memory`, `--cpus`, `--pids-limit` (TODO/machine.md T-1897): the
    /// flag, the incus key and its value; held on the incus tier only.
    pub(crate) limits: Vec<(String, &'static str, String)>,
    /// `--podvm-net none`: the guest of the machine tier has no NIC
    /// (TODO/machine.md T-1751, PodUtils/podrunner#83).
    pub(crate) net_none: bool,
    /// `-i`: the incus tier pushes the piped stdin of `run -d` (T-1777).
    pub(crate) interactive: bool,
    /// `--podvm-channel HOST_SOCKET:127.0.0.1:PORT`: one channel from a
    /// TCP port of the guest to a Unix socket of the host (T-1751).
    pub(crate) channel: Option<(std::path::PathBuf, u16)>,
    /// `--podvm-sealed`: the run comes as a bundle on stdin and reaches
    /// the guest through the channel only (T-1751, ask V-7 of podswarm).
    pub(crate) sealed: bool,
    /// `--podvm-engine`: the guest loads the modules of a container
    /// engine (T-1780, PodUtils/podrunner#92).
    pub(crate) engine: bool,
    /// `--podvm-init=image`: the init of the image is PID 1 of an incus
    /// instance (PodUtils/podrunner#313, TODO/machine.md T-1993).
    pub(crate) init_image: bool,
    /// `--init` (TODO/cli.md T-2087, OR-B40-06): held where the run has a
    /// PID namespace, whose PID 1 is podvm's init; refused elsewhere.
    pub(crate) init: bool,
    /// `--podvm-volume-base DIR`: each `-v` source under DIR opens under it
    /// with no symbolic link, and the bind takes that descriptor
    /// (PodUtils/podrunner#277, T-1945).
    pub(crate) volume_base: Option<String>,
    /// The host path of each `-v` as the caller gave it, in its order, for
    /// the check of `--podvm-volume-base` (`values::check_volume_base`).
    pub(crate) volume_given: Vec<String>,
    /// `--podvm-volume-base` as the caller gave it.
    pub(crate) volume_base_given: Option<String>,
    /// `--profile` and `-s`: the profile of the run and its overrides.
    /// TODO/cli.md T-1654.
    pub(crate) profile: Option<String>,
    pub(crate) sets: Vec<String>,
    /// `fs.wrap` of the profile: a prefix of the command.
    pub(crate) wrap: Vec<String>,
    /// The words of the command that `wrap` already holds: the runtime of
    /// the profile (TODO/cli.md T-1661).
    pub(crate) drops: Vec<String>,
    /// `needs` of the profile: the programs that the image must have
    /// (TODO/cli.md T-1659).
    pub(crate) needs: Vec<String>,
    /// `--network broker` with `--ports` and `--allow`: the egress broker
    /// of the run (TODO/net.md T-1665).
    pub(crate) broker: bool,
    /// `--network host` (TODO/cli.md T-2087, OR-B40-14): the payload shares
    /// the network of this machine, as each rung that opens no network
    /// namespace gives it.
    pub(crate) net_host: bool,
    /// `--podvm-confine=SPEC`: the confine layer (TODO/enter.md T-1658).
    pub(crate) confine: Option<podvm_enter::confine::Ask>,
    pub(crate) ports: Vec<u16>,
    pub(crate) allow: Vec<String>,
    /// `-p`: the forwards of a detached Windows guest (TODO/machine.md T-1869).
    pub(crate) publish: Vec<String>,
    /// `--device`. Carried raw here and parsed now: a malformed shape is a
    /// flag error, before anything is fetched. TODO/enter.md T-0501.
    pub(crate) devices: Vec<String>,
    /// The guest paths of the devices that go as volumes (`devices::fold`,
    /// TODO/enter.md T-1939), for the line of a refusal.
    pub(crate) device_guests: Vec<String>,
    /// `--unsafe-host-paths`. Opts into the userland pass-through with no
    /// `PODVM_MAPS` anchoring. TODO/enter.md T-1407.
    pub(crate) unsafe_host_paths: bool,
    /// `-v`. Parsed now, so a shape podvm cannot serve is a flag error
    /// before anything is fetched. TODO/enter.md T-1653.
    pub(crate) volumes: Vec<podvm_enter::userland::Volume>,
    /// `--timeout SECONDS` (podman's spelling): the wall bound of the run,
    /// after which the payload's tree is killed. TODO/cli.md T-1655.
    pub(crate) timeout: Option<u64>,
    /// `--read-only` and the guest paths of the `:ro` volumes. TODO/cli.md
    /// T-1655: the interposer's read-only layer serves them with `EROFS`.
    pub(crate) read_only: bool,
    pub(crate) ro_volumes: Vec<String>,
    /// The guest paths of the `:overlay` volumes (TODO/enter.md T-2101).
    pub(crate) overlay_volumes: Vec<String>,
    /// The guest paths of the `:mapped` volumes (TODO/machine.md T-2103).
    pub(crate) mapped_volumes: Vec<String>,
    /// The guest paths of `--tmpfs`, made writable directories of the run.
    pub(crate) tmpfs: Vec<String>,
    /// `size=` of `--tmpfs /tmp`: the cap of `/tmp` on the syscall tier
    /// (TODO/enter.md T-1657); no other tier enforces it.
    pub(crate) tmp_size: Option<u64>,
    /// `--ulimit` and `--security-opt no-new-privileges`. TODO/cli.md T-1655.
    pub(crate) hardening: podvm_supervise::table::Hardening,
    /// M5 and T-0804. ⚠ Carried in one struct so `run`, `create` and the
    /// launcher cannot each grow their own copy of the same three answers.
    pub(crate) ask: crate::complete::Ask,
}

impl Opts {
    /// TODO/cli.md T-1649. The first word of the caller where `argv[0]`
    /// is the Entrypoint of the image and the caller gave a command: the
    /// word that a refusal of that script names after `--entrypoint`.
    /// `None` under `--entrypoint`, with no command, or with no
    /// Entrypoint (the command is then `argv` itself).
    pub(crate) fn retry_word(&self, image_entrypoint: Option<&[String]>) -> Option<&str> {
        if self.entrypoint.is_some() || image_entrypoint.is_none_or(<[String]>::is_empty) {
            return None;
        }
        self.command.first().map(String::as_str)
    }
}

impl Opts {
    /// The options with no flag given.
    pub(crate) fn new() -> Opts {
        Opts {
            rm: false,
            detach: false,
            name: None,
            env: Vec::new(),
            workdir: None,
            entrypoint: None,
            platform: None,
            pull: "missing".into(),
            insecure: Vec::new(),
            tls_verify: None,
            tty: false,
            image: None,
            command: Vec::new(),
            user: None,
            hostname: None,
            tier: None,
            qemu_args: Vec::new(),
            mem: None,
            cpus: None,
            limits: Vec::new(),
            net_none: false,
            sealed: false,
            engine: false,
            init_image: false,
            init: false,
            volume_base: None,
            volume_given: Vec::new(),
            volume_base_given: None,
            interactive: false,
            channel: None,
            profile: None,
            sets: Vec::new(),
            wrap: Vec::new(),
            drops: Vec::new(),
            needs: Vec::new(),
            broker: false,
            net_host: false,
            confine: None,
            ports: Vec::new(),
            allow: Vec::new(),
            publish: Vec::new(),
            devices: Vec::new(),
            device_guests: Vec::new(),
            unsafe_host_paths: false,
            volumes: Vec::new(),
            timeout: None,
            read_only: false,
            ro_volumes: Vec::new(),
            overlay_volumes: Vec::new(),
            mapped_volumes: Vec::new(),
            tmpfs: Vec::new(),
            tmp_size: None,
            hardening: Default::default(),
            ask: crate::complete::Ask::default(),
        }
    }
}

/// `--hostname NAME` (TODO/enter.md T-1908): a host name of 1 to 63
/// letters, digits, dots and hyphens, as the nodename of `uname` holds.
pub(super) fn hostname_arg(verb: &str, a: &str) -> Result<String, i32> {
    let ok = (1..=63).contains(&a.len())
        && a.bytes().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && a.bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'.');
    if ok {
        return Ok(a.to_string());
    }
    eprintln!(
        "podvm {verb}: --hostname takes 1 to 63 letters, digits, dots and hyphens, \
         with a letter or a digit first, not {a:?} (TODO/enter.md T-1908)"
    );
    Err(podvm_image::error::EXIT_FLAG_ERROR)
}

/// `--name NAME` (PodUtils/podvm#484): docker's rule of a container name,
/// `[a-zA-Z0-9][a-zA-Z0-9_.-]*`, as for the name of a machine, at most 64
/// bytes. An empty name, a `/` or a space made a record that `ps` showed
/// and `inspect` resolved, and a long name a path of the store.
pub(super) fn name_arg(verb: &str, a: &str) -> Result<String, i32> {
    if podvm_windows::machines::valid_name(a) {
        return Ok(a.to_string());
    }
    eprintln!(
        "podvm {verb}: --name takes 1 to 64 letters, digits, '_', '.' and '-', with a \
         letter or a digit first ([a-zA-Z0-9][a-zA-Z0-9_.-]*), not {a:?}"
    );
    Err(podvm_image::error::EXIT_FLAG_ERROR)
}

/// PodUtils/podvm#482: where the profile `name` names an image, each operand
/// is the command; a first operand that reads as an image reference (a `:`
/// or a `/`, and not an absolute path) is refused by name, not run as the
/// command.
pub(super) fn profile_operand(
    verb: &str,
    name: &str,
    image: &str,
    first: Option<&str>,
) -> Result<(), i32> {
    let Some(f) = first.filter(|f| !f.starts_with('/') && f.contains([':', '/'])) else {
        return Ok(());
    };
    eprintln!(
        "podvm {verb}: --profile {name} names the image {image}, so each operand is the \
         command, and {f:?} reads as an image; give only the command, or another image \
         with -s fs.image={f}"
    );
    Err(podvm_image::error::EXIT_FLAG_ERROR)
}

#[cfg(test)]
#[test]
fn an_operand_that_reads_as_an_image_is_refused_under_a_profile_of_an_image() {
    let p = |f| profile_operand("run", "p", "busybox:musl", Some(f));
    assert!(p("alpine:3.20").is_err() && p("ghcr.io/x/y").is_err());
    assert!(p("python3").is_ok() && p("/bin/echo").is_ok());
}

#[cfg(test)]
#[test]
fn a_name_follows_the_rule_of_docker() {
    for ok in ["web", "a", "u-1", "x.y_z", &"n".repeat(64)] {
        assert_eq!(name_arg("run", ok), Ok(ok.to_string()));
    }
    for bad in ["", "bad/name", "bad name", "-x", ".x", &"n".repeat(65)] {
        assert!(name_arg("run", bad).is_err(), "{bad:?}");
    }
}

/// `--podvm-init VALUE` (TODO/machine.md T-1993): `image` (the init of the
/// image is PID 1) or `podvm` (podvm's own init, the default).
pub(super) fn init_arg(verb: &str, a: &str) -> Result<bool, i32> {
    match a {
        "image" => Ok(true),
        "podvm" => Ok(false),
        _ => {
            eprintln!("podvm {verb}: --podvm-init takes image or podvm, not {a:?}");
            Err(podvm_image::error::EXIT_FLAG_ERROR)
        }
    }
}

/// `--podvm-init=VALUE`, as [`init_arg`].
pub(super) fn init_eq(verb: &str, a: &str, o: &mut Opts) -> Result<(), i32> {
    o.init_image = init_arg(verb, a.strip_prefix("--podvm-init=").unwrap_or(a))?;
    Ok(())
}

/// `--podvm-init=image` holds on an incus instance that stays (`run -d`
/// and `create`, T-1993), on a foreground `run` of the namespace rung, the
/// runsc tier or the machine tier, and on `run -d` and `create` with no
/// tier, which take the namespace rung (TODO/enter.md T-2002); elsewhere
/// it is a refusal by name, never a silent podvm init.
pub(super) fn init_check(verb: &str, o: &Opts) -> Result<(), i32> {
    use crate::tier::Want;
    let stays = o.detach || verb == "create";
    let holds = match o.tier {
        Some(Want::Incus) => stays,
        Some(Want::Machine) | Some(Want::Runsc) => verb == "run" && !stays,
        None => verb == "run" || verb == "create",
        Some(_) => false,
    };
    if o.init && o.init_image {
        eprintln!(
            "podvm {verb}: --init and --podvm-init=image each name PID 1 of the run: give one \
             (TODO/cli.md T-2087)"
        );
        return Err(podvm_image::error::EXIT_FLAG_ERROR);
    }
    if !o.init_image || holds {
        return Ok(());
    }
    eprintln!(
        "podvm {verb}: --podvm-init=image holds on a foreground run (the namespace rung, \
         else the runsc tier, else the machine tier), on run -d and create with no tier (the \
         namespace rung) and on --podvm-tier=incus with run -d or create (TODO/machine.md \
         T-1993, TODO/enter.md T-2002, T-2007)"
    );
    Err(podvm_image::error::EXIT_FLAG_ERROR)
}

/// `--podvm-mem VALUE`: a byte count with an optional K/M/G/T suffix.
pub(super) fn mem_arg(verb: &str, a: &str) -> Result<u64, i32> {
    crate::tier::parse_mem(a).ok_or_else(|| {
        eprintln!(
            "podvm {verb}: --podvm-mem takes a byte count with an optional K/M/G/T suffix, not {a:?}"
        );
        podvm_image::error::EXIT_FLAG_ERROR
    })
}

/// `--tls-verify=VALUE`: true or false.
pub(super) fn tls_eq(verb: &str, v: &str) -> Result<bool, i32> {
    match v {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => {
            eprintln!("podvm {verb}: --tls-verify takes true or false, not {v:?}");
            Err(podvm_image::error::EXIT_FLAG_ERROR)
        }
    }
}
