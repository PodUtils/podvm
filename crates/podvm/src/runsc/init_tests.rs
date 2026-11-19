use super::*;

fn job<'a>(rootfs: &'a Path, volumes: &'a [podvm_enter::userland::Volume]) -> Job<'a> {
    Job {
        verb: "run",
        store_root: Path::new("/store"),
        rootfs,
        argv: vec!["sh".into(), "-c".into(), "exit 7".into()],
        env: vec!["PATH=/bin".into(), "A=b c".into()],
        workdir: "/w".into(),
        stdin: None,
        volumes,
        ro: &[],
        read_only: false,
        user: (33, 34),
        init: Some("/store/rootfs/x/.meta.jsonl".into()),
    }
}

/// The process of the bundle is podvm's first step, as root with the
/// capabilities of a container; the command of the run is not in the
/// bundle; systemd finds each of its mount points; a volume comes after
/// the tmpfs on `/tmp`.
#[test]
fn the_bundle_starts_the_first_step_and_holds_the_mount_points_of_systemd() {
    let vols = vec![podvm_enter::userland::Volume {
        host: "/h/t".into(),
        guest: "/tmp/t".into(),
    }];
    let s = spec(
        &job(Path::new("/store/trees/x/rootfs"), &vols),
        Path::new("/usr/bin/podvm"),
        Path::new("/store/rootfs/x/.meta.jsonl"),
    );
    assert_eq!(
        s["process"]["args"],
        serde_json::json!([
            "/.podvm/podvm",
            "guest-init",
            "--pid1",
            "--owners",
            "/.podvm/meta.jsonl",
            "--mask",
            "systemd-sysctl.service"
        ])
    );
    assert_eq!(s["process"]["user"]["uid"], 0);
    assert_eq!(s["process"]["env"], serde_json::json!(["container=podvm"]));
    let caps = s["process"]["capabilities"]["bounding"].as_array().unwrap();
    for held in ["CAP_SYS_ADMIN", "CAP_SETUID", "CAP_CHOWN"] {
        assert!(caps.iter().any(|c| c == held), "{held}");
    }
    for out in [
        "CAP_NET_ADMIN",
        "CAP_SYS_MODULE",
        "CAP_SYS_RAWIO",
        "CAP_SYS_TIME",
    ] {
        assert!(!caps.iter().any(|c| c == out), "{out}");
    }
    let m = s["mounts"].as_array().unwrap();
    let at = |d: &str| m.iter().position(|x| x["destination"] == d);
    for d in [
        "/proc",
        "/dev",
        "/sys",
        "/dev/shm",
        "/dev/pts",
        "/sys/fs/cgroup",
        "/run",
        "/tmp",
    ] {
        assert!(at(d).is_some(), "{d}");
    }
    assert_eq!(m[at("/sys/fs/cgroup").unwrap()]["type"], "cgroup");
    assert_eq!(m[at("/.podvm/podvm").unwrap()]["source"], "/usr/bin/podvm");
    assert_eq!(
        m[at("/.podvm/meta.jsonl").unwrap()]["source"],
        "/store/rootfs/x/.meta.jsonl"
    );
    assert!(at("/tmp").unwrap() < at("/tmp/t").unwrap());
    assert!(at("/dev").unwrap() < at("/dev/shm").unwrap());
}

/// The arguments of the run keep those of the tier, with the cgroup2
/// of the Sentry, set-user-ID programs and the overlay in a file, each
/// before the verb; the exec carries the user, the directory and each
/// variable of the run.
#[test]
fn the_arguments_of_the_run_and_of_the_exec() {
    let a = args(Path::new("/s"), Path::new("/b"), "podvm-1");
    let verb = a.iter().position(|x| x == "run").unwrap();
    for flag in [
        "--overlay2=root:self",
        "--in-sandbox-cgroup=v2",
        "--allow-suid",
        "--rootless",
    ] {
        assert!(a.iter().position(|x| x == flag).unwrap() < verb, "{flag}");
    }
    assert!(!a.contains(&"--overlay2=none".to_string()));
    assert_eq!(a.last().unwrap(), "podvm-1");
    let j = job(Path::new("/r"), &[]);
    let e = exec_args(
        Path::new("/s"),
        "podvm-1",
        Some(j.user),
        &j.workdir,
        &j.env,
        &j.argv,
    );
    assert_eq!(
        e,
        [
            "--rootless",
            "--root=/s",
            "exec",
            "--user=33:34",
            "--cwd=/w",
            "--env=PATH=/bin",
            "--env=A=b c",
            "podvm-1",
            "sh",
            "-c",
            "exit 7"
        ]
    );
    let root = exec_args(Path::new("/s"), "i", None, "", &[], &["true".to_string()]);
    assert_eq!(
        root,
        ["--rootless", "--root=/s", "exec", "--cwd=/", "i", "true"]
    );
}
