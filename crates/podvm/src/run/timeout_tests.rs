use super::*;

fn v(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

/// TODO/cli.md T-1655. `--timeout` takes whole seconds over 0, in both
/// spellings, and carries them to the run; 0, a fraction and a word are
/// flag errors.
#[test]
fn the_timeout_takes_whole_seconds_over_zero() {
    assert_eq!(
        parse("run", &v(&["--timeout", "2", "img"]))
            .unwrap()
            .timeout,
        Some(2)
    );
    assert_eq!(
        parse("run", &v(&["--timeout=30", "img"])).unwrap().timeout,
        Some(30)
    );
    assert_eq!(parse("run", &v(&["img"])).unwrap().timeout, None);
    for bad in ["0", "1.5", "two", "-1", ""] {
        assert_eq!(
            parse("run", &v(&["--timeout", bad, "img"])).unwrap_err(),
            EXIT_FLAG_ERROR,
            "{bad:?}"
        );
    }
}

/// TODO/cli.md T-1655. `--ulimit` and `--security-opt no-new-privileges`
/// reach the record of the run; a bad limit and each other security option
/// are flag errors.
#[test]
fn the_limits_of_the_hardening_line_parse() {
    let o = parse(
        "run",
        &v(&[
            "--ulimit",
            "nofile=64",
            "--ulimit=fsize=1024:2048",
            "--security-opt",
            "no-new-privileges",
            "img",
        ]),
    )
    .unwrap();
    assert_eq!(o.hardening.ulimits, v(&["nofile=64", "fsize=1024:2048"]));
    assert!(o.hardening.no_new_privs);
    assert_eq!(o.hardening.limits().len(), 2);
    let o = parse("run", &v(&["--security-opt=no-new-privileges:true", "img"])).unwrap();
    assert!(o.hardening.no_new_privs);
    assert!(
        parse("run", &v(&["--cap-drop", "ALL", "img"]))
            .unwrap()
            .hardening
            .no_new_privs
    );
    assert!(
        parse("run", &v(&["--cap-drop=all", "img"]))
            .unwrap()
            .hardening
            .no_new_privs
    );
    assert_eq!(
        parse("run", &v(&["--cap-drop", "CHOWN", "img"])).unwrap_err(),
        EXIT_FLAG_ERROR
    );
    assert_eq!(
        parse("run", &v(&["--cap-add", "NET_ADMIN", "img"])).unwrap_err(),
        EXIT_FLAG_ERROR
    );
    assert!(!parse("run", &v(&["img"])).unwrap().hardening.no_new_privs);
    for bad in [
        &["--ulimit", "files=1", "img"][..],
        &["--ulimit", "nofile=2:1", "img"][..],
        &["--security-opt", "seccomp=unconfined", "img"][..],
        &["--security-opt", "label=disable", "img"][..],
    ] {
        assert_eq!(
            parse("run", &v(bad)).unwrap_err(),
            EXIT_FLAG_ERROR,
            "{bad:?}"
        );
    }
}

/// TODO/cli.md T-1827: the last value of a resource wins, `-1` takes the
/// hard limit that podvm has, and a hard value over it is refused with
/// both numbers.
#[test]
fn ulimit_plan_keeps_the_last_value_and_resolves_minus_one() {
    use super::parse::values::ulimit_plan;
    let own = |_: u64| Some(1_048_576);
    let got = ulimit_plan(&v(&["nofile=64", "nproc=10", "nofile=128"]), &own).unwrap();
    assert_eq!(got, v(&["nproc=10", "nofile=128"]));
    let got = ulimit_plan(&v(&["nofile=-1"]), &own).unwrap();
    assert_eq!(got, v(&["nofile=1048576"]));
    let e = ulimit_plan(&v(&["nofile=64:1048577"]), &own).unwrap_err();
    assert!(e.contains("1048577") && e.contains("1048576"), "{e}");
    let unlimited = |_: u64| None;
    let got = ulimit_plan(&v(&["core=-1"]), &unlimited).unwrap();
    assert_eq!(got, v(&["core=-1"]));
}

/// TODO/cli.md T-1828: `--add-host` takes a host name and an address,
/// `-e` a name, `--env-file` keeps the quotes, and a repeated label key
/// keeps its last value.
#[test]
fn run_values_refuse_and_keep_as_docker_does() {
    use super::parse::envfile::read_env_file;
    use super::parse::values::{env_arg, push_label};
    use crate::complete::check_add_host;
    let e = check_add_host("foo:notanip").unwrap_err();
    assert!(e.contains("notanip"), "{e}");
    assert!(check_add_host("foo:1.2.3.4\nbar").is_err());
    assert!(check_add_host("a b:1.2.3.4").is_err());
    assert!(check_add_host("foo:1.2.3.4").is_ok());
    assert!(check_add_host("foo:::1").is_ok());
    assert!(check_add_host("foo:[fe80::1]").is_ok());
    assert!(check_add_host("gw:host-gateway").is_ok());
    assert!(env_arg("run", "=V").is_err());
    assert!(env_arg("run", "A=1").unwrap().is_some());
    let f = std::env::temp_dir().join(format!("podvm-envfile-{}", std::process::id()));
    std::fs::write(&f, "B=\"two words\"\nC='x'\n").unwrap();
    let got = read_env_file("run", f.to_str().unwrap()).unwrap();
    let _ = std::fs::remove_file(&f);
    assert_eq!(got, v(&["B=\"two words\"", "C='x'"]));
    let mut labels = Vec::new();
    push_label(&mut labels, "a=b".into());
    push_label(&mut labels, "z=1".into());
    push_label(&mut labels, "a=c".into());
    assert_eq!(labels, v(&["z=1", "a=c"]));
}

/// TODO/cli.md T-1831: the help says what the verbs do.
#[test]
fn help_text_says_what_run_create_and_rm_do() {
    let create = super::usage("create");
    assert!(
        !create.contains("--report") && !create.contains("--detach"),
        "{create}"
    );
    let run = super::usage("run");
    assert!(run.contains("--label") && run.contains("--detach"), "{run}");
    assert!(!run.contains("until TODO/cli.md T-1761"), "{run}");
    assert!(!run.contains("T-1761"), "{run}");
    assert!(crate::lifecycle::RM_USAGE.contains("-v"));
    assert!(super::parse("create", &v(&["-d", "img"])).is_err());
    assert!(super::parse("run", &v(&["-d", "img"])).is_ok());
}
