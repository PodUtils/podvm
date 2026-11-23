//! `inspect --podvm-tier=incus NAME...`, TODO/machine.md T-1722: the
//! state of an instance of podvm in incus, as JSON or through `--format`,
//! with the fields of [`FIELDS`].

use pod_incus::FileContent;
use podvm_image::error::{EXIT_CLI_ERROR, EXIT_FLAG_ERROR, EXIT_RUNTIME_ERROR};

use super::verbs::{block_on, connect, ours, IMAGE_KEY};

/// The fields of an instance. `EnteredRung` is `incus`; `ExitCode` is the
/// code of the command of a stopped instance, or a dash.
pub const FIELDS: &[&str] = &[
    "Id",
    "Name",
    "Image",
    "State",
    "State.Status",
    "State.Running",
    "State.ExitCode",
    "Status",
    "Running",
    "ExitCode",
    "Created",
    "Cmd",
    "EnteredRung",
    "CpuSeconds",
    "PeakMemoryBytes",
    "OomKills",
];

/// What incusd measured of an instance (PodUtils/podrunner#296 asks 2 and
/// 6, TODO/machine.md T-1965, TODO/supervise.md T-1969); `None` where it
/// gave nothing.
#[derive(Default)]
pub struct Usage {
    pub cpu_s: Option<f64>,
    pub peak_bytes: Option<i64>,
    pub oom_kills: Option<u64>,
}

/// The usage of a stopped instance from the file that its init wrote
/// (TODO/machine.md T-2076, PodUtils/podrunner#345), or why there is none.
/// Only an instance whose init is podvm's hold (`guest-hold --run`) has
/// the file: with `--podvm-init=image` the init of the image is PID 1.
pub async fn stopped_usage(
    c: &pod_incus::Client,
    i: &pod_incus::Instance,
) -> (Usage, Option<String>) {
    if i.config.get("raw.lxc").map(String::as_str) != Some(super::verbs::INIT) {
        return (
            Usage::default(),
            Some(
                "the init of the instance is not podvm's (--podvm-init=image): no usage \
                 stays after the stop"
                    .into(),
            ),
        );
    }
    let file = match c.get_file(&i.name, super::usage::USAGE).await {
        Ok(f) => f,
        Err(e) => {
            return (
                Usage::default(),
                Some(format!(
                    "{}: {e}: a podvm before 0.2.0-beta.39 made the instance, or its init \
                     did not end on its own (a forced stop)",
                    super::usage::USAGE
                )),
            );
        }
    };
    usage_of_file(file.meta.kind, &file.content)
}

/// The usage of the answer of incusd for the usage file: the kind of the
/// entry and its content. A value that the file does not give is `None`,
/// with the reason.
pub fn usage_of_file(kind: pod_incus::FileKind, content: &FileContent) -> (Usage, Option<String>) {
    let path = super::usage::USAGE;
    let b = match (kind, content) {
        (pod_incus::FileKind::File, FileContent::Bytes(b)) => b,
        (k, _) => {
            return (
                Usage::default(),
                Some(format!("{path} is a {}, not a file", k.word())),
            );
        }
    };
    match super::usage::Measure::parse(b) {
        Ok(m) => {
            let u = Usage {
                cpu_s: m.cpu_usec.map(|us| us as f64 / 1e6),
                peak_bytes: m.peak_bytes.and_then(|b| i64::try_from(b).ok()),
                oom_kills: m.oom_kills,
            };
            let missing = u.cpu_s.is_none() || u.peak_bytes.is_none() || u.oom_kills.is_none();
            let why = missing.then(|| {
                if m.why.is_empty() {
                    format!("{path} gives no value")
                } else {
                    format!("{path}: {}", m.why.join("; "))
                }
            });
            (u, why)
        }
        Err(e) => (Usage::default(), Some(format!("{path}: {e}"))),
    }
}

/// The usage of the instance `name` from its state and the metrics.
pub async fn usage_of(c: &pod_incus::Client, name: &str) -> Usage {
    let mut u = Usage::default();
    if let Ok(s) = c.instance_state(name).await {
        u.cpu_s = (s.cpu.usage > 0).then(|| s.cpu.usage as f64 / 1e9);
        u.peak_bytes = (s.memory.usage_peak > 0).then_some(s.memory.usage_peak);
    }
    u.oom_kills = c.oom_kills(name).await.ok().flatten();
    u
}

/// A measured value, or a dash.
fn or_dash<T: ToString>(v: Option<T>) -> String {
    v.map_or_else(|| "-".to_string(), |v| v.to_string())
}

/// `inspect` of each name; the exit is 1 where a name is not an instance.
pub fn inspect(args: &[String]) -> i32 {
    let mut template: Option<String> = None;
    let mut names: Vec<String> = Vec::new();
    let mut verbose = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            // T-2076: why a value of the usage is a dash, on stderr.
            "--verbose" => verbose = true,
            "-f" | "--format" => match it.next() {
                Some(t) => template = Some(t.clone()),
                None => {
                    eprintln!("podvm inspect: --format needs a template");
                    return EXIT_FLAG_ERROR;
                }
            },
            t if t.starts_with("--format=") => template = Some(t["--format=".len()..].to_string()),
            f if f.starts_with('-') => {
                eprintln!(
                    "podvm inspect: {f} is not served with --podvm-tier=incus (TODO/machine.md T-1722)"
                );
                return EXIT_FLAG_ERROR;
            }
            n => names.push(n.to_string()),
        }
    }
    if names.is_empty() {
        eprintln!("podvm inspect: name an instance");
        return EXIT_FLAG_ERROR;
    }
    if let Some(t) = &template
        && let Err(bad) = crate::format::check(t, FIELDS)
    {
        eprintln!("podvm inspect: {bad}");
        return EXIT_CLI_ERROR;
    }
    block_on("inspect", async move {
        let c = match connect().await {
            Ok((c, _)) => c,
            Err(e) => {
                eprintln!("podvm inspect: incus tier: {e}");
                return EXIT_RUNTIME_ERROR;
            }
        };
        let mut code = 0;
        let mut docs = Vec::new();
        for n in &names {
            let i = match c.instance(n).await {
                Ok(i) if ours(&i) => i,
                Ok(_) => {
                    eprintln!("podvm inspect: {n}: not an instance of podvm");
                    code = 1;
                    continue;
                }
                Err(e) => {
                    eprintln!("podvm inspect: {n}: {e}");
                    code = 1;
                    continue;
                }
            };
            let exit = if i.status == "Running" {
                "-".to_string()
            } else {
                match c.get_file(n, super::EXIT).await {
                    Ok(f) => match f.content {
                        FileContent::Bytes(b) => String::from_utf8_lossy(&b).trim().to_string(),
                        _ => "-".to_string(),
                    },
                    Err(_) => "-".to_string(),
                }
            };
            let cmd = match c.get_file(n, super::CMD).await {
                Ok(f) => match f.content {
                    FileContent::Bytes(b) => {
                        super::Cmd::parse(&b).map(|c| c.argv).unwrap_or_default()
                    }
                    _ => Vec::new(),
                },
                Err(_) => Vec::new(),
            };
            let usage = if i.status == "Running" {
                usage_of(&c, n).await
            } else {
                // T-2076: incusd gives no usage of a stopped instance; its
                // init wrote it at the end.
                let (u, why) = stopped_usage(&c, &i).await;
                if verbose && let Some(why) = why {
                    eprintln!("podvm inspect: {n}: a usage value is a dash: {why}");
                }
                u
            };
            let fields = fields(&i, &exit, &cmd, &usage);
            match &template {
                Some(t) => match crate::format::render(t, &fields) {
                    Ok(line) => println!("{line}"),
                    Err(bad) => {
                        eprintln!("podvm inspect: {bad}");
                        return EXIT_CLI_ERROR;
                    }
                },
                None => docs.push(json(&i, &exit, &cmd, &usage)),
            }
        }
        if template.is_none() && !docs.is_empty() {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::Value::Array(docs)).unwrap_or_default()
            );
        }
        code
    })
}

/// The fields of an instance for `--format`.
pub fn fields(
    i: &pod_incus::Instance,
    exit: &str,
    cmd: &[String],
    u: &Usage,
) -> Vec<(&'static str, String)> {
    let image = i
        .config
        .get(IMAGE_KEY)
        .cloned()
        .unwrap_or_else(|| "-".into());
    vec![
        ("Id", i.name.clone()),
        ("Name", i.name.clone()),
        ("Image", image),
        ("State", i.status.to_lowercase()),
        // TODO/cli.md T-2001 OP-06: docker's names of the same values.
        ("State.Status", i.status.to_lowercase()),
        ("State.Running", (i.status == "Running").to_string()),
        ("State.ExitCode", exit.to_string()),
        ("Status", i.status.clone()),
        ("Running", (i.status == "Running").to_string()),
        ("ExitCode", exit.to_string()),
        ("Created", i.created_at.clone()),
        ("Cmd", cmd.join(" ")),
        ("EnteredRung", "incus".to_string()),
        ("CpuSeconds", or_dash(u.cpu_s.map(|s| format!("{s:.3}")))),
        ("PeakMemoryBytes", or_dash(u.peak_bytes)),
        ("OomKills", or_dash(u.oom_kills)),
    ]
}

/// The JSON of an instance, in docker's shape where a field has one.
pub fn json(i: &pod_incus::Instance, exit: &str, cmd: &[String], u: &Usage) -> serde_json::Value {
    serde_json::json!({
        "Id": i.name,
        "Name": i.name,
        "Image": i.config.get(IMAGE_KEY),
        "Created": i.created_at,
        "State": {
            "Status": i.status.to_lowercase(),
            "Running": i.status == "Running",
            "ExitCode": exit.parse::<i64>().ok(),
        },
        "Config": { "Cmd": cmd },
        "EnteredRung": "incus",
        "CpuSeconds": u.cpu_s,
        "PeakMemoryBytes": u.peak_bytes,
        "OomKills": u.oom_kills,
    })
}
