use super::*;

/// ⚠ By name or by number, and an unknown one is refused rather than defaulted:
/// sending the wrong signal is not something a caller can notice. A number is
/// the rule `SIGNAL` of [`crate::number`] (TODO/gate.md T-1947).
pub(crate) fn signal_number(v: &str) -> Option<i32> {
    if v.starts_with(|c: char| c.is_ascii_digit()) {
        return crate::number::parse(&crate::number::SIGNAL, v)
            .ok()
            .map(|n| n as i32);
    }
    let name = v.trim_start_matches("SIG").to_ascii_uppercase();
    Some(match name.as_str() {
        "HUP" => 1,
        "INT" => 2,
        "QUIT" => 3,
        "KILL" => 9,
        "USR1" => 10,
        "USR2" => 12,
        "TERM" => 15,
        "CONT" => 18,
        "STOP" => 19,
        _ => return None,
    })
}

/// The line that refuses the signal `v`: the range where `v` is a number, the
/// names where it is not.
pub(crate) fn signal_refusal(v: &str) -> String {
    if v.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '+') {
        return crate::number::check(&crate::number::SIGNAL, v)
            .err()
            .unwrap_or_default();
    }
    format!(
        "--signal takes a name (HUP, INT, QUIT, KILL, USR1, USR2, TERM, CONT or STOP) or a \
         number from 1 to 64, not {v:?}"
    )
}

/// `podvm wait <container>...`
pub fn wait(args: &[String]) -> i32 {
    // ⭐ TODO/cli.md T-1330: bundled shorts expand before admission.
    let expanded = match crate::parity::expand("wait", args) {
        Ok(a) => a,
        Err(member) => {
            return crate::parity::refuse_member(
                "wait",
                &member,
                "usage: podvm wait [--podvm-tier incus] <container> [container...]",
            )
        }
    };
    let args: &[String] = &expanded;
    if let Some(c) = crate::parity::admit_all(
        "wait",
        args,
        "usage: podvm wait [--podvm-tier incus] <container> [container...]",
    ) {
        return c;
    }
    const WAIT_USAGE: &str = "usage: podvm wait [--podvm-tier incus] <container> [container...]";
    if args.is_empty() || args[0] == "-h" || args[0] == "--help" {
        return super::usage(args.is_empty(), WAIT_USAGE);
    }
    for want in args {
        if want.starts_with('-') {
            if let Err(c) = crate::parity::admit(
                "wait",
                want,
                "usage: podvm wait [--podvm-tier incus] <container> [container...]",
            ) {
                return c;
            }
            return crate::parity::no_arm("wait", want);
        }
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    let mut code = 0;
    for want in args {
        // TODO/machine.md T-2042: a machine, through its keeper.
        if let Some(r) = crate::windows::machine::find(want) {
            match super::target::wait(&r, WAIT_BOUND_MS) {
                Ok(n) => println!("{n}"),
                Err(c) => code = c,
            }
            continue;
        }
        match podvm_supervise::wait(&s, want, WAIT_BOUND_MS).map(|(c, n)| settle(&s, c, n)) {
            Ok((_, Some(n))) => println!("{n}"),
            // TODO/cli.md T-2108 (OR-B42-06): a container that was never
            // started has no exit code to wait for. docker waits until a
            // start; podvm refuses at once, as a refusal after the parse
            // (exit 1), and never waits for a start that nobody ordered.
            Ok((c, None)) if c.state == podvm_supervise::table::State::Created => {
                eprintln!(
                    "podvm wait: {} is created and was never started, so it has no exit \
                     code; `podvm start {}` first (docker waits for a start, podvm refuses)",
                    c.name, c.name
                );
                code = EXIT_CLI_ERROR;
            }
            Ok((c, None)) => {
                // ⛔ THE BOUND, OR AN UNWATCHED EXIT, AND NEVER A NUMBER. A `0`
                // printed here for a container podvm did not see end is the
                // lie this whole crate is arranged to refuse.
                eprintln!(
                    "podvm wait: {} has no exit code podvm measured: it is {} \
                     (TODO/supervise.md T-0604)",
                    c.name,
                    c.status()
                );
                code = EXIT_RUNTIME_ERROR;
            }
            Err(e) => code = fail_named("wait", want, e),
        }
    }
    code
}

/// ⛔ TODO/supervise.md T-1720. A wait that ends with no code while the
/// record still says running is a launcher that went between its answer and
/// its write of the record: read the record again until it has the code or
/// is not running, within [`SETTLE_MS`]. PodUtils/podrunner#49 measured the
/// "it is Up" answer with a payload that ends at once.
pub(super) fn settle(
    s: &podvm_image::Store,
    c: podvm_supervise::table::Container,
    code: Option<i32>,
) -> (podvm_supervise::table::Container, Option<i32>) {
    if code.is_some() || c.state != podvm_supervise::table::State::Running {
        return (c, code);
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(SETTLE_MS);
    let mut last = c;
    while std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
        let Ok(now) = podvm_supervise::get(s, &last.id) else {
            break;
        };
        if now.exit_code.is_some() || now.state != podvm_supervise::table::State::Running {
            let code = now.exit_code;
            return (now, code);
        }
        last = now;
    }
    (last, None)
}

/// How long `wait` reads the record again after the launcher went.
const SETTLE_MS: u64 = 10_000;

/// The usage of `rm` (TODO/cli.md T-1831: `-v` is in it).
pub(crate) const RM_USAGE: &str =
    "usage: podvm rm [-f|--force] [-v|--volumes] [--podvm-tier incus] <container> [container...]
  -f, --force    kill a running container first; a name that is not there is named, not
                 an error, as docker does
  -v, --volumes  taken and does nothing: podvm has no named or anonymous volume
  --podvm-tier incus  an instance of incus; of the other flags it takes -f only";

/// `podvm rm [-f] <container>...`
pub fn rm(args: &[String]) -> i32 {
    // ⭐ TODO/cli.md T-1330: bundled shorts expand before admission.
    let expanded = match crate::parity::expand("rm", args) {
        Ok(a) => a,
        Err(member) => return crate::parity::refuse_member("rm", &member, RM_USAGE),
    };
    let args: &[String] = &expanded;
    if let Some(c) = crate::parity::admit_all("rm", args, RM_USAGE) {
        return c;
    }
    let mut force = false;
    let mut names = Vec::new();
    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                println!("{RM_USAGE}");
                return 0;
            }
            "-f" | "--force" => force = true,
            "-v" | "--volumes" => {}
            other if other.starts_with('-') => {
                if let Err(c) = crate::parity::admit("rm", other, RM_USAGE) {
                    return c;
                }
                return crate::parity::no_arm("rm", other);
            }
            other => names.push(other.to_string()),
        }
    }
    if names.is_empty() {
        eprintln!("{RM_USAGE}");
        return EXIT_CLI_ERROR;
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    let mut code = 0;
    for want in &names {
        if let Some(c) = crate::windows::machine::rm_if(want, force) {
            code = if c == 0 { code } else { c }; // T-1870
            continue;
        }
        code = match remove_tree(&s, want, force) {
            Ok(None) => code,
            Ok(Some(c)) => c,
            // TODO/cli.md T-2079 (OR-B39-19): docker's `rm -f` names a
            // container that is not there and fails nothing for it.
            Err(e) if force && matches!(e.1, Some(pod_core::Class::NotFound)) => {
                eprintln!("podvm rm: {e}");
                code
            }
            Err(e) => fail_named("rm", want, e),
        };
    }
    code
}

/// `rm` of one container. ⛔ TODO/supervise.md T-1708: `rm -f` kills a running one
/// through its launcher; processes that stayed keep the record (exit 125, the count).
pub(super) fn remove_tree(
    s: &podvm_image::Store,
    want: &str,
    force: bool,
) -> Result<Option<i32>, podvm_supervise::Error> {
    let c = podvm_supervise::get(s, want)?;
    let killed_now = force && c.state == podvm_supervise::table::State::Running;
    let c = if killed_now {
        podvm_supervise::stop(s, &c.id, 0)?;
        podvm_supervise::get(s, &c.id)?
    } else {
        c
    };
    let stayed = match c.end.as_ref().map(|e| e.residual_processes) {
        Some(podvm_supervise::Residual::Count(n)) if n > 0 => Some(n),
        _ => None,
    };
    if let Some(n) = stayed {
        if killed_now {
            eprintln!(
                "podvm rm: {}: {n} process(es) of its tree stayed after SIGKILL, so \
                 podvm keeps the record (`podvm inspect --format '{{{{.End.Residual}}}}' \
                 {}`); a later rm removes it (TODO/supervise.md T-1708)",
                c.name, c.name
            );
            return Ok(Some(EXIT_RUNTIME_ERROR));
        }
        eprintln!(
            "podvm rm: {}: {n} process(es) of its tree stayed after its end, and podvm \
             no longer tracks them (TODO/supervise.md T-1708)",
            c.name
        );
    }
    let c = podvm_supervise::remove(s, &c.id, force)?;
    println!("{}", c.name);
    Ok(None)
}

/// `podvm cp <src> <dest>`, where one side is `<container>:<path>`.
pub fn cp(args: &[String]) -> i32 {
    // ⭐ TODO/cli.md T-1330: bundled shorts expand before admission.
    let expanded = match crate::parity::expand("cp", args) {
        Ok(a) => a,
        Err(member) => return crate::parity::refuse_member("cp", &member, CP_USAGE),
    };
    let args: &[String] = &expanded;
    // A lone `-` is the tar stream of `cp` (T-2108), not an option to admit.
    let flags: Vec<String> = args.iter().filter(|a| *a != "-").cloned().collect();
    if let Some(c) = crate::parity::admit_all("cp", &flags, CP_USAGE) {
        return c;
    }
    let o = match parse_cp(args) {
        Ok(o) => o,
        Err(c) => return c,
    };
    // TODO/machine.md T-1868: a full Windows machine copies through its agent.
    if let Some(c) = crate::windows::machine::cp_if(&o.a, &o.b, o.recursive) {
        return c;
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    // TODO/probe.md T-1911: a stopped tree of a subordinate id.
    if let Some(c) = crate::userns::cp_in_range(&s, &o.a, &o.b, args) {
        return c;
    }
    let (a, b) = (o.a.as_str(), o.b.as_str());
    let (from_named, to_named) = (named(a), named(b));
    match (from_named, to_named) {
        (true, true) => {
            eprintln!("podvm cp: copying between two named filesystems is not implemented");
            EXIT_CLI_ERROR // TODO/cli.md T-2108: a refusal after the parse
        }
        (false, false) => {
            eprintln!("podvm cp: one of the two paths has to be <name>:<path>");
            EXIT_CLI_ERROR
        }
        (true, false) => copy_named(&s, a, std::path::Path::new(b), true, o.recursive),
        (false, true) => copy_named(&s, b, std::path::Path::new(a), false, o.recursive),
    }
}

/// A side addresses a named filesystem: a container (`split`, no slashes)
/// or an image reference (slashes, tags and all).
fn named(arg: &str) -> bool {
    split(arg).is_some() || super::cp_image::split_image(arg).is_some()
}

/// Copy through a name that is a container first and an image second.
/// Container-first keeps every existing invocation resolving exactly as
/// before; the image half splits on the LAST colon, because a tag carries
/// one (`alpine:3.20:/etc/hosts`).
fn copy_named(
    s: &podvm_image::Store,
    arg: &str,
    outside: &std::path::Path,
    out_of: bool,
    recursive: bool,
) -> i32 {
    if let Some((name, inside)) = split(arg)
        && let Ok(c) = podvm_supervise::get(s, &name)
    {
        // T-1775: `cp` joins the mounts of a running payload of the
        // namespace rung, as `exec` does (T-1765); `setns` moves the cwd.
        // ⚠ A join, not `std::path::absolute`: that drops a last `/.`, which
        // names the contents of SRC (T-2079).
        // `-` is the tar stream, not a file of the cwd (T-2108).
        let outside = match std::env::current_dir() {
            Ok(cwd)
                if outside.is_relative()
                    && outside != std::path::Path::new(super::cp_tar::STREAM) =>
            {
                cwd.join(outside)
            }
            _ => outside.to_path_buf(),
        };
        if c.state == podvm_supervise::table::State::Running && c.rung == "namespace" {
            crate::exec::enter::join_mounts(c.pid);
        }
        return copy_one(
            std::path::Path::new(&c.rootfs),
            &inside,
            &outside,
            out_of,
            recursive,
        );
    }
    let (image, inside) = match arg.rsplit_once(':') {
        Some((image, inside)) if !image.is_empty() && !inside.is_empty() => (image, inside),
        _ => ("", ""),
    };
    let record = match s.find_one(image) {
        Ok(r) => r,
        Err(image_err) => {
            // TODO/supervise.md T-1805: one refusal, with the code of the
            // image error (1 for a name that nothing has), not two and 125.
            let Some((name, _)) = split(arg) else {
                eprintln!("podvm cp: one of the two paths has to be <name>:<path>");
                return EXIT_CLI_ERROR;
            };
            eprintln!("podvm cp: no container is named {name}, and {image_err}");
            return image_err.exit_code();
        }
    };
    // PodUtils/podvm#498, #563: a copy INTO an image wrote the extraction
    // that each later container copies, and that `extract --force` and
    // `save` lose. An image takes no write; a copy out of it stays.
    if !out_of {
        eprintln!(
            "podvm cp: {image} is an image, and a copy into an image is refused: it would \
             change the shared extraction, not the image. Copy into a container \
             (`podvm create --name NAME {image}`, then `podvm cp {} NAME:{inside}`)",
            outside.display()
        );
        return EXIT_CLI_ERROR;
    }
    // ⚠ The image lock for the whole copy: the rootfs below must not be
    // deleted between the extraction check and the last byte, which is
    // T-0204's hold and T-1322's query answering together.
    let _held = match s.hold(&record) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("podvm cp: {e}");
            return podvm_image::error::EXIT_RUNTIME_ERROR;
        }
    };
    if !podvm_extract::is_extracted(s, &record.manifest_digest)
        && let Err(c) = crate::run::extract_now("cp", s, &record, false)
    {
        return c;
    }
    let (rootfs, _) = podvm_extract::paths(s, &record.manifest_digest);
    copy_one(&rootfs, inside, outside, out_of, recursive)
}

/// `name:/path` where the name is not a Windows drive letter or a bare path.
pub(crate) fn split(arg: &str) -> Option<(String, String)> {
    let (name, path) = arg.split_once(':')?;
    if name.is_empty() || path.is_empty() || name.contains('/') {
        return None;
    }
    Some((name.to_string(), path.to_string()))
}
