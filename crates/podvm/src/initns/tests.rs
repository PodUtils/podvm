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

#[test]
fn the_arguments_of_the_supervisor_come_back_as_they_went() {
    let binds = [podvm_enter::VolumeBind {
        host: "/srv/t".into(),
        guest: "/tmp/t".into(),
        read_only: true,
        host_base: None,
    }];
    let args = host_args(
        "/store/trees/a/rootfs",
        Path::new("/sys/fs/cgroup/d/podvm-init-7"),
        "/work",
        Some((33, 34)),
        &binds,
        &v(&["sh", "-c", "exit 7"]),
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
    assert_eq!(spec.argv, v(&["sh", "-c", "exit 7"]));
    let tmp = spec.tmp.unwrap();
    assert_eq!(tmp.len(), 1);
    assert!(tmp[0].read_only && tmp[0].guest == "/tmp/t" && tmp[0].host == "/srv/t");
    // Root keeps its ids: no --ids.
    let root = host_args("/r", Path::new("/c"), "/", Some((0, 0)), &[], &v(&["true"]));
    assert!(!root.contains(&"--ids".to_string()));
}

#[test]
fn a_wrong_argument_of_the_supervisor_is_named() {
    for (args, word) in [
        (v(&["--root", "/r"]), "no command"),
        (v(&["--ids", "x", "--", "true"]), "UID:GID"),
        (v(&["--bind", "/etc:/x", "--", "true"]), "--bind"),
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
    mark_env(&mut env, false, true, true);
    assert_eq!(env, v(&["A=1"]));
    mark_env(&mut env, true, false, false);
    assert_eq!(env, v(&["A=1", MARK]));
    mark_env(&mut env, true, true, true);
    assert_eq!(
        env,
        v(&["A=1", MARK, "PODVM_INIT_LOG=1", "PODVM_INIT_DISCARD=1"])
    );
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
