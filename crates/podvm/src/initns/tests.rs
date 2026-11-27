use super::*;

fn v(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

#[test]
fn the_flag_is_read_in_both_forms_of_a_run_only() {
    for (argv, want) in [
        (v(&["podvm", "run", "--podvm-init=image", "img"]), true),
        (v(&["podvm", "run", "--podvm-init", "image", "img"]), true),
        (v(&["podvm", "run", "--podvm-init=podvm", "img"]), false),
        (v(&["podvm", "run", "--podvm-init", "podvm", "img"]), false),
        (v(&["podvm", "run", "img"]), false),
        (v(&["podvm", "create", "--podvm-init=image", "img"]), false),
        (v(&["podvm", "run", "--podvm-init"]), false),
        (v(&["podvm"]), false),
    ] {
        assert_eq!(asked(&argv), want, "{argv:?}");
    }
}

/// One table for the decision: each part that the host can lack gives its
/// own reason, in the order user namespace, range, cgroup.
#[test]
fn each_missing_part_of_the_host_gives_its_reason() {
    let dir = || Ok(PathBuf::from("/sys/fs/cgroup/user.slice"));
    let none = || Err("none: no directory can be written".to_string());
    for (userns, range, cgroup, want) in [
        (true, true, dir(), None),
        (false, true, dir(), Some("PODVM_USERNS")),
        (true, false, dir(), Some("subordinate id range")),
        (true, true, none(), Some("delegates no cgroup2 subtree")),
        (false, false, none(), Some("PODVM_USERNS")),
        (true, false, none(), Some("subordinate id range")),
    ] {
        let got = refusal(&Host {
            userns,
            range,
            cgroup,
        });
        match want {
            None => assert_eq!(got, None),
            Some(word) => assert!(got.as_deref().is_some_and(|g| g.contains(word)), "{got:?}"),
        }
    }
}

/// The plan of a foreground run on the init route: the fields that the
/// supervisor reads, each other one empty.
fn plan_of(
    cwd: &str,
    volumes: Vec<podvm_enter::VolumeBind>,
    limits: Vec<podvm_enter::Limit>,
    no_new_privs: bool,
    argv: &[&str],
) -> podvm_enter::Plan {
    podvm_enter::Plan {
        argv: v(argv),
        env: Vec::new(),
        working_dir: cwd.to_string(),
        fds: podvm_enter::Fds::default(),
        banner: String::new(),
        path_dirs: Vec::new(),
        names: podvm_enter::NAMES,
        limits,
        no_new_privs,
        die_with_parent: true,
        confine: None,
        hostname: None,
        volumes,
        userns_ids: None,
        pid_namespace: false,
        relay_title: None,
        tmpfs: Vec::new(),
    }
}

#[test]
fn the_arguments_of_the_supervisor_come_back_as_they_went() {
    let bind = |guest: &str, host: &str, read_only| podvm_enter::VolumeBind {
        host: host.into(),
        guest: guest.into(),
        read_only,
        host_base: None,
    };
    let nofile = podvm_enter::ulimit("nofile=64:128").unwrap();
    let nproc = podvm_enter::ulimit("nproc=unlimited").unwrap();
    let plan = plan_of(
        "/work",
        vec![
            bind("/tmp/t", "/srv/t", true),
            bind("/dev/xnull", "/dev/null", false),
        ],
        vec![nofile, nproc],
        true,
        &["sh", "-c", "exit 7"],
    );
    let args = host_args(
        "/store/trees/a/rootfs",
        Path::new("/sys/fs/cgroup/d/podvm-init-7"),
        Some((33, 34)),
        &[4, 27, 100000],
        &plan,
    );
    assert_eq!(args[0], VERB);
    let spec = parse(&args[1..]).unwrap();
    assert_eq!(spec.root, PathBuf::from("/store/trees/a/rootfs"));
    assert_eq!(
        spec.cgroup,
        Some(PathBuf::from("/sys/fs/cgroup/d/podvm-init-7"))
    );
    assert_eq!(spec.cwd, "/work");
    assert_eq!(spec.ids, Some((33, 34)));
    assert_eq!(spec.groups, vec![4, 27, 100000]);
    assert_eq!(spec.argv, v(&["sh", "-c", "exit 7"]));
    // TODO/enter.md T-2017: the limits, the bit and a device on /dev.
    assert_eq!(spec.limits, vec![nofile, nproc]);
    assert!(spec.no_new_privs);
    let tmp = spec.tmp.unwrap();
    assert_eq!(tmp.len(), 2);
    assert!(tmp[0].read_only && tmp[0].guest == "/tmp/t" && tmp[0].host == "/srv/t");
    assert!(!tmp[1].read_only && tmp[1].guest == "/dev/xnull" && tmp[1].host == "/dev/null");
    // Root keeps its ids: no --ids, and no group list without them; no
    // limit and no bit where the run asks for none.
    let plan = plan_of("/", Vec::new(), Vec::new(), false, &["true"]);
    let root = host_args("/r", Path::new("/c"), Some((0, 0)), &[4], &plan);
    for flag in ["--ids", "--groups", harden::ULIMIT, harden::NO_NEW_PRIVS] {
        assert!(!root.contains(&flag.to_string()), "{flag}: {root:?}");
    }
}

/// TODO/enter.md T-2002: the command of `--user NAME` gets the groups
/// that the image gives the user, and only those.
#[test]
fn the_groups_of_a_user_come_from_the_files_of_the_image() {
    let passwd = "root:x:0:0:root:/root:/bin/sh\n\
                  # a comment:x:33:33::/:/bin/sh\n\
                  http:x:33:33::/srv/http:/usr/bin/nologin\n\
                  www:x:33:33:the same ids::\n\
                  alias:x:33:99:another primary group::\n\
                  broken:x:notanumber:33::\n\
                  short\n";
    let group = "root:x:0:root\n\
                 http:x:33:\n\
                 adm:x:4:syslog,http\n\
                 video:x:44: http ,other\n\
                 web:x:120:www\n\
                 again:x:4:http\n\
                 alias-only:x:99:alias\n\
                 primary-listed:x:33:http\n\
                 nomembers:x:50\n\
                 badgid:x:x:http\n\
                 #commented:x:60:http\n\
                 :x:61:http\n\
                 prefix:x:70:httpd,xhttp\n";
    for (ids, want) in [
        ((33, 33), vec![4, 44, 120]),
        // `--user http:99`: the names with the ids 33 and 99 are `alias` only.
        ((33, 99), vec![]),
        ((0, 0), vec![]),
        // A uid that the image does not name.
        ((1234, 1234), vec![]),
    ] {
        assert_eq!(groups::groups_of(passwd, group, ids), want, "{ids:?}");
    }
    for (p, g) in [("", group), (passwd, ""), ("\u{0}:::", "\u{fffd}:::::")] {
        assert_eq!(groups::groups_of(p, g, (33, 33)), Vec::<u32>::new());
    }
    // More groups than a command gets: the list is cut, not refused.
    let many: String = (1000..2000).map(|g| format!("g{g}:x:{g}:http\n")).collect();
    assert_eq!(groups::groups_of(passwd, &many, (33, 33)).len(), 256);
}

#[test]
fn a_wrong_argument_of_the_supervisor_is_named() {
    for (args, word) in [
        (v(&["--root", "/r"]), "no command"),
        (v(&["--ids", "x", "--", "true"]), "UID:GID"),
        (v(&["--groups", "4,x", "--", "true"]), "GID,GID"),
        (v(&["--groups", "", "--", "true"]), "GID,GID"),
        (v(&["--groups", "-1", "--", "true"]), "GID,GID"),
        (
            v(&["--hold", "/f", "--", "true"]),
            "--hold takes no command",
        ),
        (
            v(&["--hold", "/f", "--attach", "/f"]),
            "--hold takes no command",
        ),
        (v(&["--bind", "/etc:/x", "--", "true"]), "--bind"),
        (v(&["--ulimit", "nofile", "--", "true"]), "--ulimit"),
        (v(&["--ulimit", "nofile=2:1", "--", "true"]), "--ulimit"),
        (v(&["--ulimit"]), "takes a value"),
        (v(&["--what", "--", "true"]), "no argument"),
        (v(&["--cwd"]), "takes a value"),
    ] {
        let e = parse(&args).unwrap_err();
        assert!(e.contains(word), "{args:?}: {e}");
    }
}

#[test]
fn only_the_flag_sets_the_mark() {
    let mut env = v(&["A=1", "PODVM_INIT=image", "PODVM_INIT_LOG=1"]);
    mark_env(&mut env, false, true, true, true);
    assert_eq!(env, v(&["A=1"]));
    mark_env(&mut env, true, false, false, false);
    assert_eq!(env, v(&["A=1", MARK]));
    mark_env(&mut env, true, true, true, false);
    assert_eq!(
        env,
        v(&["A=1", MARK, "PODVM_INIT_LOG=1", "PODVM_INIT_DISCARD=1"])
    );
    mark_env(&mut env, true, false, false, true);
    assert_eq!(env, v(&["A=1", MARK, "PODVM_INIT_TTY=1"]));
}

/// TODO/enter.md T-2002: the supervisor of a detached run gets the state
/// file, the cgroups and the tree of the record, and the command after
/// `--`, also where a word of the command looks like a flag.
#[test]
fn the_supervisor_of_a_detached_run_gets_the_record() {
    let mut c: podvm_supervise::table::Container = serde_json::from_value(serde_json::json!({
        "id": "abc", "name": "n", "argv": ["sh", "--state", "-c", "true"], "env": [],
        "working_dir": "", "created_at": "", "started_at": null, "finished_at": null,
        "state": "created", "pid": null, "launcher_pid": null, "exit_code": null,
        "noticed": null, "image": "img", "manifest_digest": "sha256:0",
        "rootfs": "/store/trees/t/rootfs", "rung": "namespace"
    }))
    .expect("a record of the table");
    c.working_dir = "/srv".to_string();
    // TODO/enter.md T-2017: the limits and the bit of the record.
    c.of.hardening.ulimits = v(&["nofile=123"]);
    c.of.hardening.no_new_privs = true;
    let job = podvm_supervise::launcher::InitJob {
        container: &c,
        pass: &[],
        state: "/store/containers/abc/init.state".into(),
    };
    let a = detach::args(&job, std::path::Path::new("/cg/podvm-init-1"));
    assert_eq!(&a[..2], [VERB, detach::FLAG]);
    let spec = parse(&a[2..]).expect("the supervisor reads its own arguments");
    assert_eq!(
        spec.state.as_deref(),
        Some(std::path::Path::new("/store/containers/abc/init.state"))
    );
    assert_eq!(
        spec.cgroup.as_deref(),
        Some(std::path::Path::new("/cg/podvm-init-1"))
    );
    assert_eq!(spec.root, std::path::PathBuf::from("/store/trees/t/rootfs"));
    assert_eq!(spec.cwd, "/srv");
    assert_eq!(spec.argv, v(&["sh", "--state", "-c", "true"]));
    assert!(!spec.tty && spec.hold.is_none() && spec.attach.is_none());
    assert_eq!(spec.limits, c.of.hardening.limits());
    assert!(spec.no_new_privs);
}

/// TODO/enter.md T-2002: which run keeps an init for a container that
/// stays, and on which rung: each other case is a refusal by name.
#[test]
fn only_the_namespace_rung_keeps_the_init_of_a_container_that_stays() {
    for (init, tier, detach, verb, rung, refused) in [
        (true, false, true, "run", "namespace", false),
        (true, false, false, "create", "namespace", false),
        (true, false, true, "run", "userland", true),
        (true, false, true, "run", "chroot", true),
        (true, false, false, "create", "userland", true),
        // A foreground run has its own routes; a tier has its own check.
        (true, false, false, "run", "userland", false),
        (true, true, true, "run", "userland", false),
        (false, false, true, "run", "userland", false),
    ] {
        let why = stays::wrong_rung(stays::stays(init, tier, detach, verb), rung);
        assert_eq!(
            why.is_some(),
            refused,
            "{init} {tier} {detach} {verb} {rung}"
        );
        if let Some(w) = why {
            assert!(w.contains(rung), "{w}");
        }
    }
}

#[test]
fn the_cgroup2_mount_point_is_read_from_mountinfo() {
    let hybrid = "30 25 0:26 / /sys/fs/cgroup ro,nosuid shared:9 - tmpfs tmpfs ro,mode=755\n\
                  31 30 0:27 / /sys/fs/cgroup/unified rw,nosuid shared:10 - cgroup2 cgroup2 rw\n\
                  32 30 0:28 / /sys/fs/cgroup/systemd rw shared:11 - cgroup cgroup rw,name=systemd\n";
    assert_eq!(
        cgroup::mount_point(hybrid).as_deref(),
        Some("/sys/fs/cgroup/unified")
    );
    let v1 = "30 25 0:26 / /sys/fs/cgroup ro - tmpfs tmpfs ro\n";
    assert_eq!(cgroup::mount_point(v1), None);
    assert_eq!(cgroup::mount_point(""), None);
}

#[test]
fn a_guest_of_an_init_gets_the_cpus_of_the_host_up_to_four() {
    for (host, want) in [(0, 1), (1, 1), (2, 2), (4, 4), (64, 4)] {
        assert_eq!(guest_cpus(host), want, "{host}");
    }
}

/// A base is the rest of a killed run only where the kernel counts no
/// process in it and it is older than each boot.
#[test]
fn a_base_is_stale_with_no_process_and_an_age() {
    use std::time::Duration;
    let (old, new) = (Duration::from_secs(601), Duration::from_secs(599));
    for (events, age, want) in [
        ("populated 0\nfrozen 0\n", old, true),
        ("populated 0\nfrozen 0\n", new, false),
        ("populated 1\nfrozen 0\n", old, false),
        ("", old, false),
        ("populated 01\n", old, false),
    ] {
        assert_eq!(cgroup::stale(events, age), want, "{events:?} {age:?}");
    }
}

/// TODO/enter.md T-2004: the call of the user manager, and the read of the
/// move from `/proc/self/cgroup`.
#[test]
fn the_scope_of_the_user_manager_is_asked_for_and_read() {
    let a = scope::call_args("podvm-init-ab.scope", 4242);
    let line = a.join(" ");
    assert!(
        line.starts_with("--user --no-pager call org.freedesktop.systemd1 "),
        "{line}"
    );
    assert!(
        line.ends_with(
            "StartTransientUnit ssa(sv)a(sa(sv)) podvm-init-ab.scope fail 2 \
             PIDs au 1 4242 Delegate b true 0"
        ),
        "{line}"
    );
    let own = "0::/user.slice/user-1000.slice/user@1000.service/app.slice/podvm-init-ab.scope\n";
    for (text, want) in [
        (own, true),
        (
            "1:cpu:/x\n0::/user.slice/user-1000.slice/session-3.scope\n",
            false,
        ),
        ("0::/a/podvm-init-ab.scope/child\n", false),
        ("0::/a/xpodvm-init-ab.scope\n", false),
        ("", false),
    ] {
        assert_eq!(scope::moved(text, "podvm-init-ab.scope"), want, "{text:?}");
    }
    assert_eq!(scope::user_bus(Some("/nonexistent-podvm-dir"), 1000), None);
    assert_eq!(scope::user_bus(Some("relative"), 4_000_000_000), None);
}

/// TODO/enter.md T-2010: a run keeps nothing only with `--rm` and with
/// each volume read-only.
#[test]
fn a_run_discards_only_with_rm_and_read_only_volumes() {
    let vol = |g: &str| podvm_enter::userland::Volume {
        host: "/h".into(),
        guest: g.into(),
    };
    let ro = v(&["/a"]);
    for (rm, volumes, want) in [
        (true, vec![], true),
        (false, vec![], false),
        (true, vec![vol("/a")], true),
        (true, vec![vol("/a"), vol("/b")], false),
        (false, vec![vol("/a")], false),
    ] {
        assert_eq!(discards(rm, &volumes, &ro), want, "{rm} {}", volumes.len());
    }
}

/// TODO/enter.md T-2002: the pid that the caller has for the init comes
/// from the `Pid:` line of a pidfd; 0 and -1 are a process that the
/// `/proc` of the caller cannot see, and no pid.
#[test]
fn the_pid_of_the_caller_is_read_from_the_fdinfo_of_a_pidfd() {
    for (text, want) in [
        (
            "pos:\t0\nflags:\t02000002\nmnt_id:\t16\nPid:\t4242\nNSpid:\t4242\t2\n",
            Some(4242),
        ),
        ("Pid:\t0\n", None),
        ("Pid:\t-1\n", None),
        ("Pid:\tx\n", None),
        ("NSpid:\t7\n", None),
        ("", None),
    ] {
        assert_eq!(detach::caller_pid(text), want, "{text:?}");
    }
}

#[test]
fn only_the_identity_map_is_the_initial_user_namespace() {
    for (map, want) in [
        ("         0          0 4294967295\n", false),
        (
            "         0       1000          1\n         1     100000      65536\n",
            true,
        ),
        ("0 1000 1", true),
        ("", true),
    ] {
        assert_eq!(boot::in_user_namespace(map), want, "{map:?}");
    }
}

#[test]
fn a_wait_status_gives_the_code_or_128_and_the_signal() {
    for (status, want) in [
        (0, 0),
        (7 << 8, 7),
        (255 << 8, 255),
        (9, 137),
        (15, 143),
        (2, 130),
    ] {
        assert_eq!(boot::code_of(status), want, "{status}");
    }
}

/// TODO/cli.md T-2087 (OR-B40-06): `--init` holds where the run has a PID
/// namespace and is refused elsewhere; a run with no `--init` passes on
/// each rung.
#[test]
fn init_holds_where_the_run_has_a_pid_namespace() {
    // (--init, a PID namespace, rung, refused)
    let rows = [
        (true, true, "namespace", false),
        (true, false, "userland", true),
        (true, false, "chroot", true),
        (false, false, "userland", false),
        (false, true, "namespace", false),
    ];
    for (init, pid, rung, refused) in rows {
        let got = stays::check_init("run", init, pid, rung).is_err();
        assert_eq!(got, refused, "{init} {pid} {rung}");
    }
}

/// TODO/cli.md T-2087 (OR-B40-14): `--network host` holds where the entry
/// opens no network namespace, and is refused where one opens.
#[test]
fn network_host_holds_where_no_network_namespace_opens() {
    // (--network host, a network namespace, refused)
    let rows = [
        (true, false, false),
        (true, true, true),
        (false, true, false),
        (false, false, false),
    ];
    for (host, net, refused) in rows {
        let got = stays::check_net("run", host, net, "namespace").is_err();
        assert_eq!(got, refused, "{host} {net}");
    }
}
