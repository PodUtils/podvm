use super::*;

pub(super) fn command_of(c: &Container) -> String {
    // TODO/supervise.md T-1800: the width counts characters, and the cut
    // falls between two of them. A byte index cut a multibyte character in
    // two, and the release build aborted on the panic.
    // TODO/supervise.md T-1858: the cut counts the columns of the terminal.
    crate::width::cut(&c.argv.join(" "), 20)
}

pub(super) fn ps_field_names() -> Vec<&'static str> {
    vec![
        "ID",
        "Names",
        "Image",
        "Command",
        "CreatedAt",
        "Status",
        "State",
        "Ports",
        "Pid",
    ]
}

/// The column names of `ps --format 'table ...'` that are not the words of
/// the field's name: docker's (TODO/cli.md T-2087, OR-B40-15).
pub(super) const PS_HEADERS: &[(&str, &str)] = &[("ID", "CONTAINER ID")];

/// `ps --format`: each container through the template (TODO/cli.md T-2087).
pub(super) fn ps_template(t: &format::Template, list: &[Container], no_trunc: bool) -> i32 {
    let rows: Vec<_> = list
        .iter()
        .map(|c| format::Data::text(&ps_fields(c, no_trunc)))
        .collect();
    match t.render_list(&rows, PS_HEADERS) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(bad) => {
            eprintln!("podvm ps: {bad}");
            EXIT_CLI_ERROR
        }
    }
}

pub(super) fn ps_fields(c: &Container, no_trunc: bool) -> Vec<(&'static str, String)> {
    vec![
        ("ID", if no_trunc { c.id.clone() } else { c.short_id() }),
        ("Names", c.name.clone()),
        ("Image", c.image.clone()),
        ("Command", c.argv.join(" ")),
        ("CreatedAt", c.created_at.clone()),
        ("Status", c.status()),
        ("State", c.state.word().to_string()),
        // ⛔ Always empty, and the usage says why rather than leaving a reader
        // to wonder whether podvm forgot.
        ("Ports", String::new()),
        (
            "Pid",
            c.pid.map(|p| p.to_string()).unwrap_or_else(|| "0".into()),
        ),
    ]
}

/// The usage of `stop`. TODO/cli.md T-2100 (OR-B41-17): it names `-t` as
/// refused with `--podvm-tier incus`, as the tier refuses it.
pub(crate) const STOP_USAGE: &str =
    "usage: podvm stop [-t seconds] [--podvm-tier incus] <container> [container...]
  --podvm-tier incus  an instance of incus; -t is refused";

/// `podvm stop [-t N] <container>...`
pub fn stop(args: &[String]) -> i32 {
    // ⭐ TODO/cli.md T-1330: `stop -t5` reads as `-t 5` before admission.
    let expanded = match crate::parity::expand("stop", args) {
        Ok(a) => a,
        Err(member) => return crate::parity::refuse_member("stop", &member, STOP_USAGE),
    };
    let args: &[String] = &expanded;
    // TODO/cli.md T-2108 (OR-B42-08): before the admission, which would
    // take the `-1` after `-t` for a flag of its own.
    if let Some(line) = wait_forever(args) {
        eprintln!("{line}");
        return EXIT_FLAG_ERROR;
    }
    if let Some(c) = crate::parity::admit_all("stop", args, STOP_USAGE) {
        return c;
    }
    let mut grace = STOP_GRACE_MS;
    let mut names = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                println!("{STOP_USAGE}");
                return 0;
            }
            "-t" | "--time" | "--timeout" => match it.next() {
                Some(v) => match grace_of(v) {
                    Ok(ms) => grace = ms,
                    Err(text) => {
                        eprintln!("podvm stop: {text}");
                        return EXIT_FLAG_ERROR;
                    }
                },
                None => {
                    eprintln!("podvm stop: -t needs a value: whole seconds from 0");
                    return EXIT_FLAG_ERROR;
                }
            },
            other if other.starts_with('-') => {
                if let Err(c) = crate::parity::admit("stop", other, STOP_USAGE) {
                    return c;
                }
                return crate::parity::no_arm("stop", other);
            }
            other => names.push(other.to_string()),
        }
    }
    if names.is_empty() {
        eprintln!("{STOP_USAGE}");
        return EXIT_CLI_ERROR;
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    let mut code = 0;
    for want in &names {
        // TODO/machine.md T-1870: a detached Windows or DOS machine.
        if let Some(r) = crate::windows::machine::find(want) {
            let c = crate::windows::machine::stop(&r.name);
            if c != 0 {
                code = c;
            }
            continue;
        }
        // TODO/image.md T-1767: the caller prints the name, so `restart`,
        // which also starts, prints it once.
        match stop_one(&s, "stop", want, grace) {
            Ok(name) => println!("{name}"),
            Err(c) => code = c,
        }
    }
    code
}

/// `stop -t -1` asks, as docker reads it, to wait for ever for the payload
/// to end after SIGTERM. podvm refuses: every wait of podvm has an upper
/// bound (TODO/RULES.md section 8), and a stop that cannot end holds the
/// caller of a script for ever. The line says what happened and what to
/// give instead; `args` are the arguments after `expand`.
pub(crate) fn wait_forever(args: &[String]) -> Option<String> {
    let asked = args
        .windows(2)
        .any(|w| matches!(w[0].as_str(), "-t" | "--time" | "--timeout") && w[1] == "-1");
    asked.then(|| {
        "podvm stop: -t -1 asks to wait for ever, and podvm refuses it: every wait of podvm \
         has a bound, and a stop that cannot end holds its caller. Give the seconds that \
         the payload may take, as -t 3600 (TODO/cli.md T-2108)"
            .to_string()
    })
}

/// The grace of `stop -t SECONDS` in milliseconds, or the line that refuses
/// the value: the rule `STOP_GRACE` of [`crate::number`] (T-1691, T-1947).
/// Zero is valid: it sends SIGKILL at once, as `docker stop -t 0` does
/// (call C12).
pub(crate) fn grace_of(v: &str) -> Result<u64, String> {
    crate::number::check(&crate::number::STOP_GRACE, v).map(|s| s * 1000)
}

/// Stop one container: the per-name half of `stop`, shared with
/// `restart` (TODO/cli.md T-1331). Returns the name on success.
fn stop_one(s: &podvm_image::Store, verb: &str, want: &str, grace: u64) -> Result<String, i32> {
    match podvm_supervise::stop(s, want, grace) {
        Ok((c, killed)) => {
            if killed && grace == 0 {
                // T-1691 (call C12): `-t 0` asked for the SIGKILL, as docker.
                eprintln!("podvm {verb}: {} was killed at once (-t 0)", c.name);
            } else if killed {
                // ⛔ Said, never silent. A caller that asked for a graceful
                // stop and got a SIGKILL has to be able to tell.
                eprintln!(
                    "podvm {verb}: {} did not exit within {} s of SIGTERM and was killed",
                    c.name,
                    grace / 1000
                );
            }
            Ok(c.name)
        }
        Err(e) => Err(fail(verb, e)),
    }
}

/// `podvm restart <container>...`
pub(super) const RESTART_USAGE: &str = "usage: podvm restart <container> [container...]";

/// `podvm restart`: stop-then-start in one verb (TODO/cli.md T-1331).
///
/// A composite, not a policy: each container stops with the default
/// grace and starts again, and a failure names which half failed, so a
/// caller can tell a container that never stopped from one that
/// stopped and never came back.
pub fn restart(args: &[String]) -> i32 {
    // ⭐ TODO/cli.md T-1330: bundled shorts expand before admission
    // (this verb takes none today; the call keeps the rule complete).
    let expanded = match crate::parity::expand("restart", args) {
        Ok(a) => a,
        Err(member) => return crate::parity::refuse_member("restart", &member, RESTART_USAGE),
    };
    let args: &[String] = &expanded;
    if let Some(c) = crate::parity::admit_all("restart", args, RESTART_USAGE) {
        return c;
    }
    let mut names = Vec::new();
    for want in args {
        match want.as_str() {
            "-h" | "--help" => {
                println!("{RESTART_USAGE}");
                return 0;
            }
            other if other.starts_with('-') => {
                if let Err(c) = crate::parity::admit("restart", other, RESTART_USAGE) {
                    return c;
                }
                return crate::parity::no_arm("restart", other);
            }
            other => names.push(other.to_string()),
        }
    }
    if names.is_empty() {
        eprintln!("{RESTART_USAGE}");
        return EXIT_CLI_ERROR;
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    let mut code = 0;
    for want in &names {
        match stop_one(&s, "restart", want, STOP_GRACE_MS) {
            Ok(_) => match start_one(&s, "restart", want) {
                Ok(_) => {}
                Err(c) => {
                    eprintln!("podvm restart: {want}: stopped, then the start half failed");
                    code = c;
                }
            },
            Err(c) => {
                // TODO/supervise.md T-1805: a missing name is said once.
                if c != EXIT_CLI_ERROR {
                    eprintln!("podvm restart: {want}: the stop half failed; not started");
                }
                code = c;
            }
        }
    }
    code
}

/// `podvm kill [-s SIG] <container>...`
pub fn kill(args: &[String]) -> i32 {
    // ⭐ TODO/cli.md T-1330: bundled shorts expand before admission.
    let expanded = match crate::parity::expand("kill", args) {
        Ok(a) => a,
        Err(member) => {
            return crate::parity::refuse_member(
                "kill",
                &member,
                "usage: podvm kill [-s SIGNAL] [--podvm-tier incus] <container> [container...]",
            )
        }
    };
    let args: &[String] = &expanded;
    if let Some(c) = crate::parity::admit_all(
        "kill",
        args,
        "usage: podvm kill [-s SIGNAL] [--podvm-tier incus] <container> [container...]",
    ) {
        return c;
    }
    let mut sig = 9;
    let mut names = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                println!(
                    "usage: podvm kill [-s SIGNAL] [--podvm-tier incus] <container> [container...]"
                );
                return 0;
            }
            "-s" | "--signal" => match it.next() {
                Some(v) => match signal_number(v) {
                    Some(n) => sig = n,
                    None => {
                        eprintln!("podvm kill: {}", signal_refusal(v));
                        return EXIT_FLAG_ERROR;
                    }
                },
                None => {
                    eprintln!("podvm kill: -s needs a signal");
                    return EXIT_FLAG_ERROR;
                }
            },
            other if other.starts_with('-') => {
                if let Err(c) = crate::parity::admit(
                    "kill",
                    other,
                    "usage: podvm kill [-s SIGNAL] [--podvm-tier incus] <container> [container...]",
                ) {
                    return c;
                }
                return crate::parity::no_arm("kill", other);
            }
            other => names.push(other.to_string()),
        }
    }
    if names.is_empty() {
        eprintln!("usage: podvm kill [-s SIGNAL] [--podvm-tier incus] <container> [container...]");
        return EXIT_CLI_ERROR;
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    let mut code = 0;
    for want in &names {
        // TODO/machine.md T-2042: a machine, through its keeper.
        if let Some(r) = crate::windows::machine::find(want) {
            match super::target::kill(&r, sig) {
                Ok(()) => println!("{}", r.name),
                Err(c) => code = c,
            }
            continue;
        }
        match podvm_supervise::kill(&s, want, sig) {
            Ok(c) => println!("{}", c.name),
            Err(e) => code = fail_named("kill", want, e),
        }
    }
    code
}
