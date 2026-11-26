//! `exec --podvm-tier=incus NAME CMD...` (TODO/machine.md T-1722, T-2080).

use podvm_image::error::EXIT_RUNTIME_ERROR;

use pod_incus::Exec;

use super::verbs::{block_on, connect, ours};

/// `exec --podvm-tier=incus NAME CMD...`: the command in the instance, its
/// output and its exit code.
///
/// TODO/machine.md T-2080: with its websockets ([`super::attach`]): the
/// output as it comes, the stdin of `-i` (OR-B39-05), a signal to the
/// command, and the exit code of a command that ends with 126 or 127
/// (OR-B39-01). The environment and the working directory of the run are
/// under those of `exec`, as docker gives them (OR-B39-05).
pub fn exec(
    name: &str,
    argv: &[String],
    env: &[String],
    cwd: Option<&str>,
    interactive: bool,
) -> i32 {
    let input = if interactive {
        match crate::machine::guest::read_stdin() {
            Ok(b) => b.unwrap_or_default(),
            Err(e) => {
                eprintln!("podvm exec: incus tier: {e}");
                return EXIT_RUNTIME_ERROR;
            }
        }
    } else {
        Vec::new()
    };
    block_on("exec", async move {
        let mut sig = super::attach::Signals::open("exec");
        let c = match connect().await {
            Ok((c, _)) => c,
            Err(e) => {
                eprintln!("podvm exec: incus tier: {e}");
                return EXIT_RUNTIME_ERROR;
            }
        };
        match c.instance(name).await {
            Ok(i) if ours(&i) && i.status == "Running" => {}
            Ok(i) if ours(&i) => {
                eprintln!("podvm exec: {name}: the instance is {}", i.status);
                return EXIT_RUNTIME_ERROR;
            }
            Ok(_) => {
                eprintln!("podvm exec: {name}: not an instance of podvm");
                return EXIT_RUNTIME_ERROR;
            }
            Err(e) => {
                eprintln!("podvm exec: {name}: {e}");
                return EXIT_RUNTIME_ERROR;
            }
        }
        // The command of the run, where the instance has one.
        let run = match c.get_file(name, super::CMD).await {
            Ok(pod_incus::File {
                content: pod_incus::FileContent::Bytes(b),
                ..
            }) => super::Cmd::parse(&b).ok(),
            _ => None,
        };
        let (env, cwd) = exec_env(run.as_ref(), env, cwd);
        // TODO/cli.md T-2079 (OR-B39-09): incusd runs the command in `/`
        // where the working directory is not there; docker's exec fails.
        if let Some(line) = workdir_refusal(name, &cwd, super::walk::find(&c, name, &cwd).await) {
            eprintln!("podvm exec: {line}");
            return podvm_image::error::EXIT_CANNOT_INVOKE;
        }
        let mut x = Exec::new(argv.to_vec());
        x.environment = env;
        x.cwd = cwd;
        let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
        match super::attach::exec_streamed(
            &c, "exec", name, &x, &input, &mut sig, &mut out, &mut err,
        )
        .await
        {
            Ok(o) => {
                if let Some(h) = silent_hint(o) {
                    eprintln!("podvm exec: incus tier: {h}");
                }
                o.code
            }
            Err(e) => {
                eprintln!("podvm exec: {name}: {e}");
                EXIT_RUNTIME_ERROR
            }
        }
    })
}

/// The environment and the working directory of an `exec`: those of `run`
/// (the command of the instance), each variable of `env` over them, and
/// `cwd` where it is given.
pub fn exec_env(
    run: Option<&super::Cmd>,
    env: &[String],
    cwd: Option<&str>,
) -> (std::collections::BTreeMap<String, String>, String) {
    let mut m = run.map(|r| super::run::env_map(&r.env)).unwrap_or_default();
    m.extend(super::run::env_map(env));
    let cwd = match (cwd, run) {
        (Some(c), _) => c.to_string(),
        (None, Some(r)) => r.cwd.clone(),
        (None, None) => String::new(),
    };
    (m, cwd)
}

/// The refusal of the working directory `cwd` of an `exec` in the
/// instance `name`, as [`super::walk::find`] found it; `None` where the
/// command can run there, or where podvm cannot tell (incusd decides, as
/// before).
pub fn workdir_refusal(name: &str, cwd: &str, found: super::walk::Found) -> Option<String> {
    use super::walk::Found;
    if cwd.is_empty() {
        return None;
    }
    let what = match found {
        Found::Missing => "is not there",
        Found::Kind(pod_incus::FileKind::File) => "is a file, not a directory",
        Found::Kind(_) | Found::Unknown(_) => return None,
    };
    Some(format!(
        "{name}: the working directory {cwd:?} {what} in the instance: exec makes no \
         directory, and docker's exec fails there too (exit 126); run -w makes it \
         (TODO/cli.md T-2079)"
    ))
}

/// The line after an `exec` that ended with 126 or 127 and wrote nothing:
/// incusd gives the same answer where the program is not there.
pub fn silent_hint(o: super::attach::Outcome) -> Option<String> {
    let words = match o.code {
        127 => super::attach::NOT_FOUND,
        126 => super::attach::NOT_EXECUTABLE,
        _ => return None,
    };
    (o.wrote == 0).then(|| {
        format!(
            "the command ended with {} and wrote nothing; incusd gives the same end (\"{words}\") \
             where the program is not in the instance or cannot run there (TODO/machine.md T-2080)",
            o.code
        )
    })
}
