//! The values of the flags of the hardening line (TODO/cli.md T-1655),
//! `--podvm-cpus` (T-1654) and the file of `--env-file` (T-0801).

use super::*;

pub(super) use super::bound::timeout;

/// One `--ulimit`, checked now so that a bad spec is a flag error before
/// anything is fetched. TODO/cli.md T-1655.
pub(super) fn ulimit(verb: &str, a: &str) -> Result<String, i32> {
    match podvm_enter::ulimit(a) {
        Ok(_) => Ok(a.to_string()),
        Err(why) => {
            eprintln!("podvm {verb}: --ulimit {why}");
            Err(EXIT_FLAG_ERROR)
        }
    }
}

/// TODO/cli.md T-1827: the `--ulimit` values of a run as the plan takes
/// them. The last value of each resource wins, as docker and podman keep
/// it (podcrates `v0.45.0` refuses two limits of one resource). `own`
/// gives the hard limit that podvm has for a resource number: `-1` takes
/// it where it is finite, and a hard value over it is refused with both
/// numbers, before anything runs.
pub(crate) fn ulimit_plan(
    specs: &[String],
    own: &dyn Fn(u64) -> Option<u64>,
) -> Result<Vec<String>, String> {
    let mut last: Vec<podvm_enter::Limit> = Vec::new();
    for spec in specs {
        let l = podvm_enter::ulimit(spec)?;
        last.retain(|e| e.resource != l.resource);
        last.push(l);
    }
    let inf = pod_enter_infinity();
    let mut out = Vec::new();
    for mut l in last {
        if let Some(h) = own(l.resource.number()).filter(|h| *h != inf) {
            if l.soft == inf {
                l.soft = h;
            }
            if l.hard == inf {
                l.hard = h;
            }
            if l.hard > h {
                return Err(format!(
                    "{}: the hard value {} is over the hard limit {h} of {} that podvm \
                     has, and podvm cannot raise it",
                    l.resource.word(),
                    l.hard,
                    l.resource.name()
                ));
            }
        }
        out.push(if l.soft == l.hard {
            format!("{}={}", l.resource.word(), render(l.soft))
        } else {
            format!(
                "{}={}:{}",
                l.resource.word(),
                render(l.soft),
                render(l.hard)
            )
        });
    }
    Ok(out)
}

fn pod_enter_infinity() -> u64 {
    podvm_enter::ulimit("nofile=-1")
        .map(|l| l.hard)
        .unwrap_or(u64::MAX)
}

fn render(v: u64) -> String {
    if v == pod_enter_infinity() {
        "-1".to_string()
    } else {
        v.to_string()
    }
}

/// [`ulimit_plan`] with the hard limits of this process, refused as a
/// flag error.
pub(super) fn ulimits_of(verb: &str, specs: &[String]) -> Result<Vec<String>, i32> {
    if specs.is_empty() {
        return Ok(Vec::new());
    }
    let own = |n: u64| podvm_probe::sys::prlimit(n).ok().map(|(_, hard)| hard);
    ulimit_plan(specs, &own).map_err(|why| {
        eprintln!("podvm {verb}: --ulimit {why} (TODO/cli.md T-1827)");
        EXIT_FLAG_ERROR
    })
}

/// One `--security-opt`: `no-new-privileges` (also `:true`, `=true`) sets
/// the bit in the child; each other option is refused by name, because no
/// security backend of podvm takes it. TODO/cli.md T-1655.
pub(super) fn security_opt(verb: &str, a: &str) -> Result<bool, i32> {
    match a {
        "no-new-privileges" | "no-new-privileges:true" | "no-new-privileges=true" => Ok(true),
        "no-new-privileges:false" | "no-new-privileges=false" => Ok(false),
        // TODO/cli.md T-1829: the option is known; its value is not.
        other
            if other.starts_with("no-new-privileges=")
                || other.starts_with("no-new-privileges:") =>
        {
            eprintln!(
                "podvm {verb}: --security-opt {other:?}: no-new-privileges takes true or \
                 false, and {:?} is neither (TODO/cli.md T-1829)",
                &other["no-new-privileges=".len()..]
            );
            Err(EXIT_FLAG_ERROR)
        }
        other => {
            eprintln!(
                "podvm {verb}: --security-opt {other:?} is refused: podvm takes \
                 no-new-privileges only; seccomp, apparmor, label and the rest have \
                 no backend here (TODO/cli.md T-1655)"
            );
            Err(EXIT_FLAG_ERROR)
        }
    }
}

/// One `--cap-drop`: `ALL` sets no-new-privileges in the child, which is
/// what podvm can enforce without a privilege; a named capability is
/// refused by name, because podvm does not change the capability sets.
/// TODO/cli.md T-1655.
pub(super) fn cap_drop(verb: &str, a: &str) -> Result<bool, i32> {
    if a.eq_ignore_ascii_case("all") {
        return Ok(true);
    }
    eprintln!(
        "podvm {verb}: --cap-drop {a:?} is refused: podvm takes ALL only, as \
         no-new-privileges in the payload; it does not change the capability \
         sets (TODO/cli.md T-1655)"
    );
    Err(EXIT_FLAG_ERROR)
}

/// One `--tmpfs PATH[:OPTIONS]`. TODO/cli.md T-1655. PATH is an absolute
/// guest path; `rw`, `ro`, `nosuid`, `nodev`, `exec`, `noexec`, `size=`
/// and `mode=` are read (`ro` makes it read-only; the others have nothing
/// to act on, because the directory is not a mount); each other option is
/// refused by name.
pub(super) fn tmpfs(verb: &str, spec: &str, o: &mut Opts) -> Result<(), i32> {
    let (path, opts) = spec.split_once(':').unwrap_or((spec, ""));
    let refuse = |why: String| {
        eprintln!("podvm {verb}: --tmpfs {spec:?}: {why}");
        EXIT_FLAG_ERROR
    };
    if !path.starts_with('/') || path == "/" {
        return Err(refuse("the path must be absolute and not /".to_string()));
    }
    let path = path.trim_end_matches('/').to_string();
    for opt in opts.split(',').filter(|x| !x.is_empty()) {
        match opt.split_once('=').map_or(opt, |(k, _)| k) {
            "size" if path == "/tmp" => match opt
                .split_once('=')
                .and_then(|(_, v)| crate::tier::parse_mem(v))
            {
                Some(n) => o.tmp_size = Some(n),
                None => return Err(refuse(format!("{opt:?} is not a size"))),
            },
            "rw" | "nosuid" | "nodev" | "exec" | "noexec" | "size" | "mode" => {}
            "ro" => o.ro_volumes.push(path.clone()),
            other => return Err(refuse(format!("the option {other:?} is not taken"))),
        }
    }
    o.tmpfs.push(path);
    Ok(())
}

/// One `-v`: the pair, and its guest path in the read-only list where the
/// options hold `ro`. TODO/enter.md T-1653, TODO/cli.md T-1655.
pub(super) fn volume(verb: &str, spec: &str, o: &mut Opts) -> std::result::Result<(), i32> {
    let (mut v, mode) = crate::lifecycle::volume::parse(spec).map_err(|why| {
        eprintln!("podvm {verb}: -v {spec:?}: {why}");
        EXIT_FLAG_ERROR
    })?;
    // TODO/enter.md T-1804: one spelling of the guest path, here, for the
    // volume and for each table built from it. `/data/` and `/data` were
    // two entries, and the `rw:` one widened a `:ro` volume.
    v.guest = crate::lifecycle::volume::guest_spelling(&v.guest);
    o.volume_given
        .push(spec.split(':').next().unwrap_or_default().to_string());
    super::super::overlay::note_mode(mode, &v.guest, o); // :ro, :overlay (T-2101)
    o.volumes.push(v);
    Ok(())
}

/// `-e NAME=VALUE` as given; `-e NAME` takes the value of the caller's
/// environment, and names nothing where the caller does not set NAME, as
/// docker does (TODO/cli.md T-1765).
pub(crate) fn from_host(e: &str) -> Option<String> {
    if e.contains('=') {
        return Some(e.to_string());
    }
    std::env::var_os(e).map(|v| format!("{e}={}", v.to_string_lossy()))
}

/// TODO/cli.md T-1828: `-e` and `--env` with a name. `-e =V` set nothing
/// and exited 0; docker and podman refuse it.
pub(crate) fn env_arg(verb: &str, e: &str) -> std::result::Result<Option<String>, i32> {
    if e.is_empty() || e.starts_with('=') {
        eprintln!("podvm {verb}: -e takes NAME=VALUE or NAME, with a name, not {e:?}");
        return Err(EXIT_FLAG_ERROR);
    }
    Ok(from_host(e))
}

/// TODO/cli.md T-1828: a repeated `--label` key keeps its last value, as
/// podman does.
pub(crate) fn push_label(labels: &mut Vec<String>, kv: String) {
    let key = kv
        .split_once('=')
        .map(|(k, _)| k.to_string())
        .unwrap_or_default();
    labels.retain(|l| l.split_once('=').map(|(k, _)| k) != Some(key.as_str()));
    labels.push(kv);
}

/// The line and the byte of the first byte that is not UTF-8 (TODO/cli.md
/// T-2064); `(1, 0)` for bytes that are.
pub(crate) fn utf8_line(bytes: &[u8]) -> (usize, usize) {
    let at = std::str::from_utf8(bytes).map_or_else(|e| e.valid_up_to(), |_| 0);
    (bytes[..at].iter().filter(|b| **b == b'\n').count() + 1, at)
}

/// `--env-file`: docker's file of `KEY=VALUE` lines, loaded at the flag's
/// position so a later `-e` wins over the file and a later file over an
/// earlier flag, the same rule `-e` already documents. TODO/cli.md T-0801.
///
/// The file is bounded (1 MiB) and must be UTF-8: buffering an unbounded
/// body or guessing at bytes is the shape
/// `docs/conventions/forbidden-patterns.md` refuses. A line without `=`
/// is refused naming its number rather than skipped, because a skipped
/// line is a variable the caller thinks is set.
pub(crate) fn read_env_file(verb: &str, path: &str) -> std::result::Result<Vec<String>, i32> {
    const MAX_ENV_FILE: u64 = 1_048_576;
    let meta = std::fs::metadata(path).map_err(|e| {
        eprintln!("podvm {verb}: --env-file {path:?}: {e}");
        EXIT_FLAG_ERROR
    })?;
    if !meta.is_file() {
        eprintln!("podvm {verb}: --env-file {path:?} is not a file");
        return Err(EXIT_FLAG_ERROR);
    }
    if !super::access::caller_may(&meta, 0o400) {
        eprintln!("podvm {verb}: --env-file {path:?}: Permission denied (the mode lets its owner not read it)");
        return Err(EXIT_FLAG_ERROR);
    }
    if meta.len() > MAX_ENV_FILE {
        eprintln!(
            "podvm {verb}: --env-file {path:?} is {} bytes, over the 1048576-byte ceiling",
            meta.len()
        );
        return Err(EXIT_FLAG_ERROR);
    }
    let bytes = std::fs::read(path).map_err(|e| {
        eprintln!("podvm {verb}: --env-file {path:?}: {e}");
        EXIT_FLAG_ERROR
    })?;
    // TODO/cli.md T-2064: bytes that are not UTF-8 name the line.
    let text = String::from_utf8(bytes).map_err(|e| {
        let (line, at) = utf8_line(e.as_bytes());
        eprintln!(
            "podvm {verb}: --env-file {path:?} line {line} is not UTF-8 (byte {at} of the file)"
        );
        EXIT_FLAG_ERROR
    })?;
    let mut out = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            eprintln!(
                "podvm {verb}: --env-file {path:?} line {} has no `=`: {line:?}",
                n + 1
            );
            return Err(EXIT_FLAG_ERROR);
        };
        let key = k.trim();
        if key.is_empty() {
            eprintln!(
                "podvm {verb}: --env-file {path:?} line {} has no name: {line:?}",
                n + 1
            );
            return Err(EXIT_FLAG_ERROR);
        }
        // TODO/cli.md T-1828: the value as the line gives it, quotes too,
        // as docker and podman keep it (`B="two words"` is `"two words"`).
        out.push(format!("{key}={v}"));
    }
    Ok(out)
}

/// `--env-file0 PATH` (PodUtils/podrunner#300, TODO/cli.md T-1971): the
/// entries `KEY=VALUE` with a NUL after each, as `env -0` writes them, so
/// that a value can hold a newline. The checks of the file are those of
/// [`read_env_file`]; a value is kept byte for byte, with no trim.
pub(crate) fn read_env_file0(verb: &str, path: &str) -> std::result::Result<Vec<String>, i32> {
    const MAX_ENV_FILE: u64 = 1_048_576;
    let refuse = |why: String| {
        eprintln!("podvm {verb}: --env-file0 {path:?}{why}");
        EXIT_FLAG_ERROR
    };
    let meta = std::fs::metadata(path).map_err(|e| refuse(format!(": {e}")))?;
    if !meta.is_file() {
        return Err(refuse(" is not a file".into()));
    }
    if !super::access::caller_may(&meta, 0o400) {
        return Err(refuse(
            ": Permission denied (the mode lets its owner not read it)".into(),
        ));
    }
    if meta.len() > MAX_ENV_FILE {
        return Err(refuse(format!(
            " is {} bytes, over the 1048576-byte ceiling",
            meta.len()
        )));
    }
    let bytes = std::fs::read(path).map_err(|e| refuse(format!(": {e}")))?;
    let mut out = Vec::new();
    for (n, raw) in bytes.split(|b| *b == 0).enumerate() {
        if raw.is_empty() {
            continue;
        }
        let Ok(entry) = std::str::from_utf8(raw) else {
            return Err(refuse(format!(" entry {} is not UTF-8", n + 1)));
        };
        match entry.split_once('=') {
            Some((k, _)) if !k.is_empty() && !k.contains(['\n', '\r']) => {
                out.push(entry.to_string())
            }
            Some(_) => return Err(refuse(format!(" entry {} has no name", n + 1))),
            None => return Err(refuse(format!(" entry {} has no `=`", n + 1))),
        }
    }
    Ok(out)
}

/// `--report` is a flag of `run` only (TODO/cli.md T-1695).
pub(super) fn report(verb: &str) -> std::result::Result<(), i32> {
    if verb == "run" {
        return Ok(());
    }
    eprintln!(
        "podvm {verb}: --report is a flag of `run`: one report for each run (TODO/cli.md T-1695)"
    );
    Err(EXIT_FLAG_ERROR)
}

/// TODO/cli.md T-1831: `create -d` printed an id and started nothing.
pub(super) fn create_detach(verb: &str, o: &Opts) -> std::result::Result<(), i32> {
    if verb == "create" && o.detach {
        eprintln!(
            "podvm create: -d is refused: create starts nothing; `podvm start NAME` \
             starts the container (TODO/cli.md T-1831)"
        );
        return Err(EXIT_FLAG_ERROR);
    }
    Ok(())
}

/// `--label KEY[=VALUE]` as `KEY=VALUE` (TODO/cli.md T-1695).
pub(super) fn label(verb: &str, a: &str) -> std::result::Result<String, i32> {
    let (k, v) = a.split_once('=').unwrap_or((a, ""));
    if k.is_empty() || k.contains(char::is_whitespace) {
        eprintln!("podvm {verb}: --label takes KEY=VALUE with a key, not {a:?}");
        return Err(EXIT_FLAG_ERROR);
    }
    Ok(format!("{k}={v}"))
}

/// The SPEC of `--podvm-confine` (TODO/enter.md T-1658).
pub(super) fn confine(verb: &str, a: &str) -> std::result::Result<podvm_enter::confine::Ask, i32> {
    podvm_enter::confine::parse(a).map_err(|why| {
        eprintln!(
            "podvm {verb}: --podvm-confine takes read=P,...;write=P,...;exec=P,...;connect=PORT,...: {why}"
        );
        EXIT_FLAG_ERROR
    })
}

/// The vCPUs of `--podvm-cpus`: a whole number from 1 to 64.
/// TODO/cli.md T-1654.
pub(super) fn cpus(verb: &str, a: &str) -> std::result::Result<u32, i32> {
    match a.parse::<u32>() {
        Ok(n) if (1..=64).contains(&n) => Ok(n),
        _ => {
            eprintln!("podvm {verb}: --podvm-cpus takes a whole number from 1 to 64, not {a:?}");
            Err(EXIT_FLAG_ERROR)
        }
    }
}

/// `--network`: `broker` (TODO/net.md T-1665) or `host` (TODO/cli.md
/// T-2087: the network of this machine, which a rung with no network
/// namespace gives), one of them; podvm has no network namespace to select.
pub(super) fn network(verb: &str, a: &str, o: &mut Opts) -> Result<(), i32> {
    match a {
        "broker" if !o.net_host => o.broker = true,
        "host" if !o.broker => o.net_host = true,
        _ => {
            eprintln!(
                "podvm {verb}: --network takes broker or host, one of them, not {a:?}: podvm has \
                 no network namespace to select; `--network host` shares the network of this \
                 machine, and `--network broker` holds the payload to an egress broker \
                 (TODO/net.md T-1665)"
            );
            return Err(EXIT_FLAG_ERROR);
        }
    }
    Ok(())
}

/// `--ports`: a list of ports, 1 to 65535, for the broker.
pub(super) fn ports(verb: &str, a: &str) -> Result<Vec<u16>, i32> {
    a.split(',')
        .filter(|p| !p.is_empty())
        .map(|p| match p.parse::<u16>() {
            Ok(n) if n > 0 => Ok(n),
            _ => {
                eprintln!("podvm {verb}: --ports takes ports from 1 to 65535, not {p:?}");
                Err(EXIT_FLAG_ERROR)
            }
        })
        .collect()
}

/// TODO/cli.md T-1815: `-t` with `-d`, or on `create`, is refused by name.
/// A detached terminal needs a launcher that holds the master, which podvm
/// has not; the run took the flag and gave the payload no terminal.
pub(super) fn detached_tty(verb: &str, o: &Opts) -> std::result::Result<(), i32> {
    if o.tty && (o.detach || verb == "create") {
        eprintln!(
            "podvm {verb}: -t is refused for a detached container: podvm gives a \
             terminal to a foreground run only, and a detached payload would get none \
             (TODO/cli.md T-1815)"
        );
        return Err(EXIT_FLAG_ERROR);
    }
    Ok(())
}

/// The usage text for whichever verb this parser is serving.
///
/// ⛔ `create` shares `run`'s parser, so before this existed `podvm create
/// --no-such-flag` answered `podvm run: unknown option` and printed
/// `usage: podvm run`, naming a verb the caller had not typed. A diagnostic
/// that names the wrong operation is the class TODO/cli.md T-0805 exists to
/// close, and T-0804 forbids outright.
pub fn usage(verb: &str) -> String {
    let mut s = format!("usage: podvm {verb} [options] <image> [command] [arg...]\n\n");
    if verb != "run" {
        // ⭐ Said rather than implied. The option set below is `run`'s, because
        // one parser serves both, and `podvm system info` says the same thing
        // about the same rows instead of holding a second copy of them.
        s.push_str(&format!(
            "  note: podvm {verb} takes podvm run's option set, listed below, because one\n    \
             parser serves both. `podvm system info` lists those rows under `run`.\n\n"
        ));
    }
    // TODO/cli.md T-1831: `create` starts nothing and writes no report, so
    // its help lists neither -d nor --report.
    if verb == "create" {
        s.push_str(&without_blocks(RUN_OPTIONS, &["-d, --detach", "--report"]));
    } else {
        s.push_str(RUN_OPTIONS);
    }
    s
}

/// The option text with each block whose first line starts with one of
/// `heads` left out. A block is its first line (`  -x` or `  --x`) and the
/// lines that continue it (indented deeper).
fn without_blocks(text: &str, heads: &[&str]) -> String {
    let mut out = String::new();
    let mut skip = false;
    for line in text.lines() {
        // The first line of the text has no indent: the string's `\`
        // continuation eats it. A block starts with an option either way.
        let t = line.trim_start();
        if t.starts_with('-') && line.len() - t.len() <= 2 {
            skip = heads.iter().any(|h| t.starts_with(h));
        }
        if !skip {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}
