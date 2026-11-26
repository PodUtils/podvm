use super::*;

/// One row for each shape of an entry: what it opens, each cell of the
/// banner, the "does NOT provide" words, and what `exec` shares. A text
/// that names a part that the entry opened is the fault of OQ-07.
#[test]
fn each_statement_of_an_entry_reads_its_facts() {
    struct Case {
        what: &'static str,
        rung: Rung,
        f: Facts,
        words: &'static str,
        ns: Option<&'static str>,
        pids: &'static str,
        never: &'static str,
        mode: &'static str,
        shares: &'static str,
    }
    let f = |userns_active, host_root, pid_init, hostname, proc_dir| Facts {
        userns_active,
        host_root,
        pid_init,
        hostname,
        proc_dir,
    };
    let cases = [
        Case {
            what: "a foreground run of uid 1000 (podvm's user namespace)",
            rung: Rung::Namespace,
            f: f(true, false, false, false, true),
            words: "user pid uts mount proc",
            ns: Some("user+pid+uts+mount"),
            pids: "private",
            never: "network isolation",
            mode: "join",
            shares: "filesystem+user+pid+uts+mount",
        },
        Case {
            what: "a foreground run of uid 0 (pod-enter's PID namespace, T-2054)",
            rung: Rung::Namespace,
            f: f(false, true, true, false, true),
            words: "pid mount proc",
            ns: Some("pid+mount"),
            pids: "private",
            never: "network isolation, or user isolation",
            mode: "join",
            shares: "filesystem+pid+mount",
        },
        Case {
            what: "a foreground run of uid 0 with --hostname",
            rung: Rung::Namespace,
            f: f(false, true, true, true, true),
            words: "pid uts mount proc",
            ns: Some("pid+uts+mount"),
            pids: "private",
            never: "network isolation, or user isolation",
            mode: "join",
            shares: "filesystem+pid+mount",
        },
        Case {
            what: "uid 0 with no PID namespace (a forced launch rung)",
            rung: Rung::Namespace,
            f: f(false, true, false, false, true),
            words: "mount proc",
            ns: Some("mount-only"),
            pids: "host-shared",
            never: "network isolation, or user isolation, or pid isolation",
            mode: "join",
            shares: "filesystem+mount",
        },
        Case {
            what: "PODVM_USERNS=0 of uid 1000: pod-enter's user and PID namespace",
            rung: Rung::Namespace,
            f: f(false, false, true, false, true),
            words: "user pid mount proc",
            ns: Some("user+pid+mount"),
            pids: "private",
            never: "network isolation",
            mode: "join",
            shares: "filesystem+user+pid+mount",
        },
        Case {
            what: "run -d of uid 1000",
            rung: Rung::Namespace,
            f: f(false, false, true, false, true),
            words: "user pid mount proc",
            ns: Some("user+pid+mount"),
            pids: "private",
            never: "network isolation",
            mode: "join",
            shares: "filesystem+user+pid+mount",
        },
        Case {
            what: "run -d of uid 0, an image with no /proc",
            rung: Rung::Namespace,
            f: f(false, true, true, false, false),
            words: "pid mount",
            ns: Some("pid+mount"),
            pids: "private",
            never: "network isolation, or user isolation, or a /proc filesystem",
            mode: "join",
            shares: "filesystem+pid+mount",
        },
        Case {
            what: "the chroot rung opens nothing",
            rung: Rung::Chroot,
            f: f(true, false, true, true, true),
            words: "",
            ns: None,
            pids: "host-shared",
            never: Rung::Chroot.must_never_claim(),
            mode: FRESH,
            shares: SHARES,
        },
        Case {
            what: "the userland rung opens nothing",
            rung: Rung::Userland,
            f: f(false, false, true, false, true),
            words: "",
            ns: None,
            pids: "host-shared",
            never: Rung::Userland.must_never_claim(),
            mode: FRESH,
            shares: SHARES,
        },
    ];
    for c in cases {
        let o = of_entry(c.rung, c.f);
        let rung = c.rung.word();
        assert_eq!(o.words().join(" "), c.words, "{}", c.what);
        assert_eq!(o.namespaces_cell().as_deref(), c.ns, "{}", c.what);
        assert_eq!(o.pids_cell(), c.pids, "{}", c.what);
        assert_eq!(o.never_claim(c.rung), c.never, "{}", c.what);
        assert_eq!(o.exec_mode(rung), c.mode, "{}", c.what);
        assert_eq!(o.exec_shares(rung), c.shares, "{}", c.what);
        assert_eq!(Opened::from_words(&o.words()), o, "{}: the record", c.what);
        assert_eq!(
            o.line().is_some(),
            o.has(Part::Proc) && !o.has(Part::Pid),
            "{}: the /proc of the host",
            c.what
        );
    }
}

#[test]
fn the_manual_names_each_row_and_each_shape() {
    let s = section();
    for r in ROWS {
        assert!(s.contains(r.gives), "{}", r.word);
    }
    for (shape, f) in SHAPES {
        let o = of_entry(Rung::Namespace, *f);
        assert!(s.contains(shape), "{shape}");
        assert!(
            s.contains(&format!("opens: {}", o.words().join(" "))),
            "{shape}"
        );
    }
}

#[test]
fn an_unknown_word_of_a_record_is_left_out() {
    let w = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(Opened::from_words(&w(&[])), Opened::NONE);
    assert_eq!(
        Opened::from_words(&w(&["mount", "cgroup", "pid"])),
        Opened::of(&[Part::Pid, Part::Mount])
    );
}
