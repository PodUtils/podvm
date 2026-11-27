//! TODO/gate.md T-1947: one table for each number that a flag or a variable
//! takes. Each row is a [`Rule`]; its values (below, at each edge, inside,
//! above, overflowing, not a number) go through the rule and through the
//! production parser of each verb that takes the flag, and both must agree.
//! A rule with no probe, a flag with no row in the parity table and a
//! variable that `podvm man` does not list fail the test.

use super::*;

fn v(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

/// One production parser: the verb that holds the flag, and whether it takes
/// the text.
type Probe = (&'static str, fn(&str) -> bool);

fn run_takes(args: impl Fn(&str) -> Vec<String>) -> impl Fn(&str) -> bool {
    move |x| crate::run::parse_probe("run", &args(x)).is_ok()
}

fn ssh_takes(flag: &'static str, x: &str) -> bool {
    let mut a = v(&["--kernel", "k", "--initramfs", "i", "--user-key", "u"]);
    a.extend(v(&[flag, x]));
    crate::machine::ssh::parse(&a).is_ok()
}

fn windows_takes(flag: &'static str, x: &str) -> bool {
    crate::windows::args::parse(&v(&["run", flag, x])).is_ok()
}

fn notebook_takes(flag: &'static str, x: &str) -> bool {
    crate::notebook::parse(&v(&["--persist", "/p", flag, x, "nb.ipynb"])).is_ok()
}

fn env_takes(rule: &'static Rule, x: &str) -> bool {
    from_env("test", rule, Some(std::ffi::OsStr::new(x))).is_ok()
}

/// The production parsers of each rule. A new rule needs a row here.
fn probes(flag: &str) -> Vec<Probe> {
    match flag {
        "--timeout" => vec![("run", |x| run_takes(|x| v(&["--timeout", x, "img"]))(x))],
        "-t" => vec![("stop", |x| crate::lifecycle::grace_of(x).is_ok())],
        "--podvm-timeout" => vec![
            ("machine ssh", |x| ssh_takes("--podvm-timeout", x)),
            ("windows", |x| windows_takes("--podvm-timeout", x)),
        ],
        "--podvm-cpus" => vec![
            ("run", |x| run_takes(|x| v(&["--podvm-cpus", x, "img"]))(x)),
            ("windows", |x| windows_takes("--podvm-cpus", x)),
        ],
        "--podvm-mem" => vec![
            ("run", |x| run_takes(|x| v(&["--podvm-mem", x, "img"]))(x)),
            ("run", |x| {
                run_takes(|x| v(&[&format!("--podvm-mem={x}"), "img"]))(x)
            }),
            ("exec", |x| {
                crate::exec::parse_probe(&v(&["--podvm-mem", x, "c", "true"])).is_ok()
            }),
            ("exec", |x| {
                crate::exec::parse_probe(&v(&[&format!("--podvm-mem={x}"), "c", "true"])).is_ok()
            }),
            ("machine ssh", |x| ssh_takes("--podvm-mem", x)),
            ("windows", |x| windows_takes("--podvm-mem", x)),
        ],
        "--pids-limit" => vec![("run", |x| run_takes(|x| v(&["--pids-limit", x, "img"]))(x))],
        "--cpus" => vec![("run", |x| run_takes(|x| v(&["--cpus", x, "img"]))(x))],
        "--memory" => vec![
            ("run", |x| run_takes(|x| v(&["--memory", x, "img"]))(x)),
            ("run", |x| run_takes(|x| v(&["-m", x, "img"]))(x)),
        ],
        "--storage-opt size" => vec![("run", |x| {
            run_takes(|x| v(&["--storage-opt", &format!("size={x}"), "img"]))(x)
        })],
        "--tmpfs size" => vec![("run", |x| {
            run_takes(|x| v(&["--tmpfs", &format!("/t:size={x}"), "img"]))(x)
        })],
        "--ports" => vec![("run", |x| {
            run_takes(|x| v(&["--network", "broker", "--ports", x, "--allow", "h", "img"]))(x)
        })],
        "-p port" => vec![
            ("run", |x| {
                run_takes(|x| v(&["-p", &format!("{x}:80"), "img"]))(x)
            }),
            ("run", |x| {
                run_takes(|x| v(&["-p", &format!("1:{x}"), "img"]))(x)
            }),
        ],
        "--podvm-channel port" => vec![("run", |x| {
            run_takes(|x| v(&["--podvm-channel", &format!("/s:127.0.0.1:{x}"), "img"]))(x)
        })],
        "--allow-connect" => vec![("lab", |x| {
            crate::lab::host::run_flags(&v(&["--allow-connect", x, "rest"])).is_ok()
        })],
        "--signal" => vec![("kill", |x| crate::lifecycle::signal_number(x).is_some())],
        "--tail" => vec![
            ("logs", |x| {
                crate::lifecycle::parse_logs(&v(&["--tail", x, "c"])).is_ok()
            }),
            ("logs", |x| {
                crate::lifecycle::parse_logs(&v(&[&format!("--tail={x}"), "c"])).is_ok()
            }),
        ],
        "--limit-bytes" => vec![
            ("logs", |x| {
                crate::lifecycle::parse_logs(&v(&["--limit-bytes", x, "c"])).is_ok()
            }),
            ("logs", |x| {
                crate::lifecycle::parse_logs(&v(&[&format!("--limit-bytes={x}"), "c"])).is_ok()
            }),
        ],
        "--max-bytes" => vec![("windows", |x| windows_takes("--max-bytes", x))],
        "--cycles" => vec![("notebook", |x| notebook_takes("--cycles", x))],
        "--kill-after" => vec![("notebook", |x| notebook_takes("--kill-after", x))],
        "--restart-kernel-after" => {
            vec![("notebook", |x| notebook_takes("--restart-kernel-after", x))]
        }
        "PODVM_INIT_BOOT_TIMEOUT" => vec![("", |x| {
            crate::incus::systemd::bound_of(Some(std::ffi::OsStr::new(x))).is_ok()
        })],
        "PODVM_WINDOWS_INSTALL_MEM" => vec![("", |x| env_takes(&INSTALL_MEM, x))],
        "PODVM_WINDOWS_INSTALL_CPUS" => vec![("", |x| env_takes(&INSTALL_CPUS, x))],
        "PODVM_WINDOWS_INSTALL_DEADLINE" => vec![("", |x| env_takes(&INSTALL_DEADLINE, x))],
        other => panic!("{other}: a rule with no production parser in the table"),
    }
}

/// The values of a rule in its own spelling: the text that is a value of
/// the rule, and the text that is not.
fn values(r: &Rule) -> (Vec<String>, Vec<String>) {
    let spell = |n: u64| match r.kind {
        Kind::Milli => show(Kind::Milli, n),
        _ => n.to_string(),
    };
    let mut inside = vec![spell(r.lo), spell(r.hi), spell(r.lo + (r.hi - r.lo) / 2)];
    let mut outside: Vec<String> = Vec::new();
    // Below the range, and above it.
    if r.lo > 0 {
        outside.push(match r.kind {
            Kind::Milli => format!("0.{:03}", r.lo - 1),
            _ => (r.lo - 1).to_string(),
        });
    }
    if r.hi < u64::MAX {
        outside.push(match r.kind {
            Kind::Milli => format!("{}.001", r.hi / 1000),
            _ => (r.hi + 1).to_string(),
        });
    }
    // Overflowing: past 64 bits, and a size whose product passes them.
    outside.push("99999999999999999999999".into());
    outside.push("18446744073709551616".into());
    if r.kind == Kind::Bytes {
        outside.push("99999999999999999999g".into());
        outside.push("17179869184g".into());
    }
    // Not a number of the kind.
    for bad in [
        "abc",
        "-1",
        "+1",
        " 1",
        "1 ",
        "",
        "1e3",
        "0x10",
        "1_000",
        "\u{661}\u{662}",
        "NaN",
        "inf",
        "-0",
        "1/2",
        "--1",
        "1,5",
    ] {
        outside.push(bad.to_string());
    }
    outside.push(match r.kind {
        Kind::Milli => "1.".to_string(),
        _ => "1.5".to_string(),
    });
    if r.kind == Kind::Bytes {
        outside.extend(["1x", "g", "1ib", "1kk"].map(String::from));
    }
    // A text of the grammar that the range refuses, or accepts, is decided by
    // the rule, so the lists above are filtered by it.
    inside.retain(|t| parse(r, t).is_ok());
    outside.retain(|t| parse(r, t).is_err());
    // A list of ports is a list: an empty one and `1,5` are not one number.
    if matches!(r.flag, "--ports" | "--allow-connect") {
        outside.retain(|t| !t.is_empty() && !t.contains(','));
    }
    // The boot bound of the variable drops the white space at each end (a
    // variable often ends in a newline); no other number does.
    if r.flag == "PODVM_INIT_BOOT_TIMEOUT" {
        outside.retain(|t| t.trim() == t);
    }
    (inside, outside)
}

#[test]
fn each_number_flag_has_one_bounded_parser() {
    let mut seen = std::collections::BTreeSet::new();
    for r in RULES {
        assert!(seen.insert(r.flag), "{}: two rules", r.flag);
        let (inside, outside) = values(r);
        let (lo, hi) = (show(r.kind, r.lo), show(r.kind, r.hi));
        // The edges are values of the rule, and the table has each class.
        assert!(parse(r, &show(r.kind, r.lo)).is_ok(), "{}: lo", r.flag);
        assert!(parse(r, &show(r.kind, r.hi)).is_ok(), "{}: hi", r.flag);
        assert!(
            outside.iter().any(|t| t == "-1"),
            "{}: -1 is a value",
            r.flag
        );
        assert!(
            outside.iter().any(|t| t == "abc"),
            "{}: not a number",
            r.flag
        );
        assert!(
            outside.iter().any(|t| t == "99999999999999999999999"),
            "{}: overflow",
            r.flag
        );
        if r.lo > 0 {
            assert!(outside.len() > 18, "{}: a value below the range", r.flag);
        }
        // The rule: a value of the rule is its number, and each other value
        // is one line that names the flag, the range and the value.
        for t in &inside {
            assert_eq!(check(r, t).map(|_| ()), Ok(()), "{} {t:?}", r.flag);
        }
        for t in &outside {
            let line = check(r, t).unwrap_err();
            let why = parse(r, t).unwrap_err();
            for part in [r.flag, &lo, &hi, &format!("{t:?}")] {
                assert!(
                    line.contains(part),
                    "{} {t:?}: {line:?} lacks {part:?}",
                    r.flag
                );
            }
            assert!(!line.contains('\n'), "{line:?}");
            assert_eq!(
                line.contains("too large"),
                why == Edge::High,
                "{} {t:?}: {line:?}",
                r.flag
            );
        }
        // The production parsers: the same accept and refuse.
        for (verb, takes) in probes(r.flag) {
            for t in &inside {
                assert!(
                    takes(t),
                    "{} {verb}: {t:?} is in the range and is refused",
                    r.flag
                );
            }
            for t in &outside {
                assert!(
                    !takes(t),
                    "{} {verb}: {t:?} is out of the range and is taken",
                    r.flag
                );
            }
        }
    }
}

/// The numbers of the rules land where the verbs use them: the value of a
/// flag is the number of the rule, in the unit of the rule.
#[test]
fn the_value_is_the_number_in_the_unit_of_its_rule() {
    for (rule, text, want) in [
        (&TIMEOUT, "30", 30u64),
        (&MEMORY, "512m", 512 << 20),
        (&MEMORY, "1GiB", 1 << 30),
        (&MEMORY, "1gb", 1 << 30),
        (&MEMORY, "6291456", 6 << 20),
        (&PODVM_MEM, "1T", 1 << 40),
        (&STORAGE_SIZE, "64m", 64 << 20),
        (&TMPFS_SIZE, "0", 0),
        (&TMPFS_SIZE, "100b", 100),
        (&CPUS, "1.5", 1500),
        (&CPUS, "0.5", 500),
        (&CPUS, "0.333333", 333),
        (&CPUS, "0.01", 10),
        (&CPUS, "2", 2000),
        (&CPUS, "1024", 1_024_000),
        (&PIDS_LIMIT, "4194304", PIDS_MAX),
        (&STOP_GRACE, "0", 0),
    ] {
        assert_eq!(check(rule, text), Ok(want), "{} {text}", rule.flag);
    }
    assert_eq!(show(Kind::Milli, 10), "0.01");
    assert_eq!(show(Kind::Milli, 1500), "1.5");
    assert_eq!(show(Kind::Milli, 1_024_000), "1024");
}

/// A flag with a rule is a row of the parity table in each verb that takes
/// it, and a variable with a rule is in the list of `podvm man`.
#[test]
fn each_rule_is_in_the_parity_table_or_the_variables() {
    for r in RULES {
        let name = r.flag.split(' ').next().unwrap();
        for (verb, _) in probes(r.flag) {
            if r.flag.starts_with("PODVM_") {
                let row = crate::environ::USER
                    .iter()
                    .flat_map(|g| g.vars)
                    .find(|x| x.name == r.flag);
                let row = row.unwrap_or_else(|| panic!("{} is not a variable of podvm", r.flag));
                assert!(
                    row.values.contains(&r.hi.to_string()),
                    "{}: its row of `podvm man` names the range {}",
                    r.flag,
                    r.hi
                );
            } else {
                // `-p port`, `--tmpfs size`: the flag is the first word.
                let verb = if verb == "lab" { "lab" } else { verb };
                assert!(
                    crate::parity::flag(verb, name).is_some(),
                    "{name} has no row in the parity table of {verb}"
                );
            }
        }
    }
}

/// The help of a verb names the range of each flag that it takes a number
/// for, in the numbers of the rule (the help, the parser and the table are
/// one list).
#[test]
fn the_help_names_each_range() {
    let run = crate::run::usage("run");
    let rows: Vec<(&Rule, &str, &str)> = vec![
        (&CPUS, "run", &run),
        (&PIDS_LIMIT, "run", &run),
        (&TIMEOUT, "run", &run),
        (&PODVM_CPUS, "run", &run),
        (&PODVM_MEM, "run", &run),
        (&STORAGE_SIZE, "run", &run),
        (&PODVM_MEM, "exec", crate::exec::EXEC_USAGE),
        (&MEMORY, "update", crate::incus::update::USAGE),
        (&CPUS, "update", crate::incus::update::USAGE),
        (&PIDS_LIMIT, "update", crate::incus::update::USAGE),
        (&LIMIT_BYTES, "logs", crate::lifecycle::LOGS_USAGE),
        (&CYCLES, "notebook", crate::notebook::USAGE),
        (&KILL_AFTER, "notebook", crate::notebook::USAGE),
    ];
    for (r, verb, help) in rows {
        for n in [r.lo, r.hi] {
            let n = show(r.kind, n);
            assert!(
                help.contains(&n),
                "the help of {verb} lacks {n} of {}",
                r.flag
            );
        }
    }
}
