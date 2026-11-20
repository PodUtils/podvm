use super::*;

#[test]
fn a_spec_parses_and_renders_back() {
    let s = "read=/workspace,/etc/hostname;write=/workspace/out;connect=443";
    let a = parse(s).unwrap();
    assert_eq!(a.read, ["/workspace", "/etc/hostname"]);
    assert_eq!(a.write, ["/workspace/out"]);
    assert_eq!(a.connect, Some(vec![443]));
    assert_eq!(render(&a), s);
    assert_eq!(parse("").unwrap(), Ask::default());
}

#[test]
fn a_bad_part_is_named() {
    assert!(parse("read=etc").unwrap_err().contains("not absolute"));
    assert!(parse("connect=0").unwrap_err().contains("1 to 65535"));
    assert!(parse("listen=80").unwrap_err().contains("listen"));
    assert!(parse("read").unwrap_err().contains("KEY=VALUE"));
}

#[test]
fn the_ruleset_names_the_system_paths_that_exist_and_each_asked_path() {
    let a = parse("read=/workspace;exec=/opt/tool;connect=443").unwrap();
    let s = spec(&a, &|p| p != "/lib32");
    let paths: Vec<&str> = s.paths.iter().map(|r| r.path.as_str()).collect();
    assert!(paths.contains(&"/usr") && paths.contains(&"/proc"));
    assert!(
        !paths.contains(&"/lib32"),
        "a path the image lacks has no rule"
    );
    assert!(
        !paths
            .iter()
            .any(|p| p.starts_with("/etc") && !SYSTEM_READ.contains(p)),
        "/etc is not given, beyond the files of the loader (T-1764)"
    );
    let ws = s.paths.iter().find(|r| r.path == "/workspace").unwrap();
    assert!(ws.access.contains(Access::READ) && !ws.access.contains(Access::WRITE));
    let tool = s.paths.iter().find(|r| r.path == "/opt/tool").unwrap();
    assert!(tool.access.contains(Access::EXECUTE));
    assert_eq!(s.connect, Some(vec![443]));
    assert!(!s.deny.is_empty(), "the default deny list of pod-confine");
}

#[test]
fn the_loader_reads_its_cache_under_each_spec() {
    // TODO/enter.md T-1764: python of /usr/local/lib needs ld.so.cache.
    let ask = parse("read=/usr;write=/tmp").unwrap();
    let s = spec(&ask, &|p| p == "/etc/ld.so.cache" || p == "/usr");
    let has = |p: &str| {
        s.paths
            .iter()
            .any(|r| r.path == p && r.access == Access::READ)
    };
    assert!(has("/etc/ld.so.cache"), "{:?}", s.paths);
    assert!(
        !has("/etc/ld-musl-x86_64.path"),
        "a file that the image lacks gets no rule"
    );
}

#[test]
fn an_optional_path_gets_a_rule_only_where_the_image_has_it() {
    let a = parse("read=?/etc/ssl,?/etc/pki,/etc/passwd").unwrap();
    let s = spec(&a, &|p| p != "/etc/pki");
    let paths: Vec<&str> = s.paths.iter().map(|r| r.path.as_str()).collect();
    assert!(paths.contains(&"/etc/ssl") && paths.contains(&"/etc/passwd"));
    assert!(!paths
        .iter()
        .any(|p| p.contains("pki") || p.starts_with('?')));
    assert!(parse("read=?etc").is_err());
}

#[test]
fn a_guest_path_maps_through_the_volume_or_the_rootfs() {
    let names = crate::NAMES;
    let v = crate::userland::Volume {
        guest: "/workspace".into(),
        host: "/h/proj".into(),
    };
    let maps = crate::userland::Maps::new(&names, "/s/root", &[v]).unwrap();
    assert_eq!(host_of(&maps, "/workspace"), "/h/proj");
    assert_eq!(host_of(&maps, "/workspace/out/a"), "/h/proj/out/a");
    assert_eq!(host_of(&maps, "/etc/passwd"), "/s/root/etc/passwd");
    assert_eq!(host_of(&maps, "/workspacex"), "/s/root/workspacex");
}

#[test]
fn only_a_rung_with_a_chroot_carries_the_layer() {
    use podvm_probe::select::Rung;
    assert!(rung_carries(Rung::Namespace) && rung_carries(Rung::Chroot));
    assert!(!rung_carries(Rung::Userland) && !rung_carries(Rung::Supervise));
}

/// A tree of the test: a fresh directory under the temporary directory.
fn scratch(tag: &str) -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("podvm-enter-{}-{n}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn a_link_of_the_image_that_starts_with_a_slash_resolves_inside_it() {
    let root = scratch("abs-link-in");
    std::fs::create_dir_all(root.join("bin")).unwrap();
    std::fs::write(root.join("bin/busybox"), b"x").unwrap();
    std::os::unix::fs::symlink("/bin/busybox", root.join("bin/sh")).unwrap();
    let maps = crate::userland::Maps::of_root(root.to_str().unwrap());
    let s = host_spec(&parse("read=/bin/sh").unwrap(), &maps).unwrap();
    let want = format!("{}/bin/busybox", root.display());
    let got: Vec<&String> = s.paths.iter().map(|r| &r.path).collect();
    assert!(s.paths.iter().any(|r| r.path == want), "{got:?}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_link_that_names_a_host_path_is_a_path_the_image_lacks() {
    let root = scratch("abs-link-out");
    std::fs::create_dir_all(root.join("etc")).unwrap();
    std::os::unix::fs::symlink("/bin/sh", root.join("etc/host-only")).unwrap();
    let maps = crate::userland::Maps::of_root(root.to_str().unwrap());
    let s = host_spec(&parse("read=?/etc/host-only").unwrap(), &maps).unwrap();
    let got: Vec<&String> = s.paths.iter().map(|r| &r.path).collect();
    assert!(
        !s.paths.iter().any(|r| r.path.contains("host-only")),
        "{got:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_volume_pair_of_one_device_node_maps_to_the_host_node() {
    // The device nodes of the host are volume pairs whose guest is the
    // node itself. A guest path that names the volume root leaves an
    // empty remainder, which used to come out as `<node>/` and was
    // then refused by the kernel with ENOTDIR.
    let root = scratch("vol-root");
    let vol = scratch("vol-host");
    let node = vol.join("zero");
    std::fs::write(&node, b"x").unwrap();
    let v = crate::userland::Volume {
        guest: "/dev/zero".into(),
        host: node.to_str().unwrap().to_string(),
    };
    let maps = crate::userland::Maps::new(&crate::NAMES, root.to_str().unwrap(), &[v]).unwrap();
    let want = node.to_str().unwrap().to_string();
    assert_eq!(host_resolved(&maps, "/dev/zero"), Some(want.clone()));
    // The optional form asks the resolver, not the fallback of a
    // required path, so a rule here means the resolution succeeded.
    let s = host_spec(&parse("read=?/dev/zero").unwrap(), &maps).unwrap();
    let got: Vec<&String> = s.paths.iter().map(|r| &r.path).collect();
    assert!(s.paths.iter().any(|r| r.path == want), "{got:?}");
    assert!(!s.paths.iter().any(|r| r.path.ends_with('/')), "{got:?}");
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&vol);
}

/// TODO/enter.md T-1888: a required rule on a link to a path that only
/// the host has refuses, and names the guest path.
#[test]
fn confine_required_link_out_of_the_tree_refuses_by_guest_path() {
    let root = scratch("abs-link-req");
    std::fs::create_dir_all(root.join("etc")).unwrap();
    std::os::unix::fs::symlink("/bin/sh", root.join("etc/host-only")).unwrap();
    let maps = crate::userland::Maps::of_root(root.to_str().unwrap());
    let e = host_spec(&parse("read=/etc/host-only").unwrap(), &maps).unwrap_err();
    assert!(e.contains("\"/etc/host-only\""), "{e}");
    assert!(e.contains("T-1888"), "{e}");
    let _ = std::fs::remove_dir_all(&root);
}

/// TODO/enter.md T-2013: `exec=` for the first program of an entry with
/// no chroot: the program by its path, by `PATH` and by a relative name,
/// the program of the dynamic loader, and the interpreter of a script.
#[test]
fn the_first_program_takes_the_rule_of_exec() {
    let root = scratch("first");
    for d in ["bin", "lib64", "sc", "tmp"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    for f in ["bin/sh", "lib64/ld-linux-x86-64.so.2", "sc/m", "tmp/m"] {
        std::fs::write(root.join(f), b"x").unwrap();
    }
    let maps = crate::userland::Maps::of_root(root.to_str().unwrap());
    let ask = parse("read=/sc;write=/tmp;exec=/bin/sh,/lib64").unwrap();
    let plan = pod_enter::Plan {
        argv: Vec::new(),
        env: Vec::new(),
        working_dir: "/tmp".to_string(),
        fds: pod_enter::Fds::default(),
        banner: String::new(),
        path_dirs: vec!["/bin".to_string(), "/sc".to_string()],
        names: crate::NAMES,
        limits: Vec::new(),
        no_new_privs: false,
        die_with_parent: false,
        confine: Some(host_spec(&ask, &maps).unwrap()),
        hostname: None,
        volumes: Vec::new(),
        userns_ids: None,
        pid_namespace: false,
    };
    let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let ld = "/lib64/ld-linux-x86-64.so.2";
    // (argv, the rule holds)
    for (argv, holds) in [
        (v(&["/bin/sh", "-c", "true"]), true),
        (v(&["sh"]), true),
        (v(&["/sc/m"]), false),
        (v(&["m"]), false),
        (v(&["./m"]), false),
        (v(&[ld, "/sc/m"]), false),
        (v(&[ld, "--library-path", "/tmp", "/tmp/m"]), false),
        (v(&[ld, "m"]), false),
        (v(&[ld, "/bin/sh"]), true),
        (v(&[ld, "--version"]), true),
        (v(&["/nowhere/x"]), true),
        (Vec::new(), true),
    ] {
        let got = check_first(&plan, &maps, &[&argv]);
        assert_eq!(got.is_ok(), holds, "{argv:?}: {got:?}");
        if let Err(e) = got {
            assert!(
                matches!(e, pod_enter::Error::CannotInvoke(_)),
                "{argv:?}: {e:?}"
            );
        }
    }
    // The interpreter of a `#!` script: the argv of the kernel.
    let kernel = v(&["/sc/m", "/bin/sh"]);
    assert!(check_first(&plan, &maps, &[&v(&["/bin/sh"]), &kernel]).is_err());
    // No confinement: each program runs.
    let open = pod_enter::Plan {
        confine: None,
        ..plan
    };
    assert!(check_first(&open, &maps, &[&v(&["/sc/m"])]).is_ok());
    let _ = std::fs::remove_dir_all(&root);
}

/// TODO/enter.md T-2014: a rule on a guest path that holds a volume gives
/// the same access to the host path of each volume under it, at a
/// component boundary, as the namespace rung holds a mount under the path
/// of a rule.
#[test]
fn a_rule_holds_each_volume_under_its_guest_path() {
    let root = scratch("vols-root");
    for d in ["etc", "usr", "s", "data/deep"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    let host = scratch("vols-host");
    for d in ["sc", "in", "x", "local"] {
        std::fs::create_dir_all(host.join(d)).unwrap();
    }
    std::fs::write(host.join("f"), b"x").unwrap();
    let h = |p: &str| host.join(p).to_str().unwrap().to_string();
    let vols: Vec<crate::userland::Volume> = [
        ("/sc", h("sc")),
        ("/sc/in", h("in")),
        ("/data/deep/x", h("x")),
        ("/etc/f", h("f")),
        ("/usr/local/v", h("local")),
        ("/gone", h("absent")),
    ]
    .into_iter()
    .map(|(g, h)| crate::userland::Volume {
        guest: g.into(),
        host: h,
    })
    .collect();
    let maps = crate::userland::Maps::new(&crate::NAMES, root.to_str().unwrap(), &vols).unwrap();
    let (r, rw, rx) = (
        Access::READ,
        Access::READ | Access::WRITE,
        Access::READ | Access::EXECUTE,
    );
    // (spec, each volume host with the access it must get, each volume
    // host with no rule)
    for (ask, want, none) in [
        (
            "read=/",
            vec![("sc", r), ("in", r), ("x", r), ("f", r), ("local", rx)],
            vec!["absent"],
        ),
        ("read=/data", vec![("x", r)], vec!["sc", "in", "f"]),
        ("read=/sc", vec![("sc", r), ("in", r)], vec!["x", "f"]),
        ("read=/s", vec![], vec!["sc", "in"]),
        ("write=/data/deep", vec![("x", rw)], vec!["sc"]),
        ("exec=/sc", vec![("sc", rx), ("in", rx)], vec!["x"]),
        ("read=/etc", vec![("f", r)], vec!["sc"]),
        ("read=/sc/.", vec![("sc", r), ("in", r)], vec!["x"]),
        ("", vec![("local", rx)], vec!["sc", "in", "x", "f"]),
    ] {
        let s = host_spec(&parse(ask).unwrap(), &maps).unwrap();
        let got: Vec<(&String, Access)> = s.paths.iter().map(|r| (&r.path, r.access)).collect();
        for (p, access) in want {
            assert!(
                s.paths
                    .iter()
                    .any(|r| r.path == h(p) && r.access.contains(access)),
                "{ask}: {p} {access:?}: {got:?}"
            );
        }
        for p in none {
            assert!(
                !s.paths.iter().any(|r| r.path == h(p)),
                "{ask}: {p}: {got:?}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&host);
}
