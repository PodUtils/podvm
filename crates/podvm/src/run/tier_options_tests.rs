//! TODO/gate.md T-1854: each option of `run` is honoured or refused by name
//! on each tier of a guest. An option that the parser takes and a tier
//! drops is the defect of T-1808 (the machine tier ran its payload as root
//! with each capability where `--user` and `--cap-drop ALL` asked otherwise).

use super::*;

fn v(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

/// Each option of a guest tier: its arguments, and whether the machine and
/// the incus tier refuse it by name (true) or honour it (false).
const TABLE: &[(&[&str], bool, bool)] = &[
    (&["--podvm-confine", "read=/tmp"], true, true),
    (&["--user", "1000"], true, true),
    (&["--hostname", "unit1"], true, true),
    (&["--add-host", "a:1.2.3.4"], true, true),
    (&["--read-only"], true, true),
    (&["--tmpfs", "/mnt/x"], true, true),
    (&["--ulimit", "nofile=512:512"], true, true),
    (&["--security-opt", "no-new-privileges"], true, true),
    (&["--cap-drop", "ALL"], true, true),
    (&["--label", "a=b"], true, true),
    (&["--timeout", "5"], false, true),
    (&["-t"], false, true),
    (&["-e", "A=1"], false, false),
    (&["-w", "/tmp"], false, false),
    (&["-v", "/tmp:/data"], false, false),
    (&["--podvm-net", "none"], false, false),
    (&["--podvm-volume-base", "/tmp"], true, true),
];

#[test]
fn tier_option_table_holds_for_the_machine_and_incus_tiers() {
    for (flags, machine_refuses, incus_refused) in TABLE {
        let mut a = flags.to_vec();
        a.extend_from_slice(&["img", "true"]);
        let o = parse::parse("run", &v(&a)).unwrap_or_else(|c| panic!("{flags:?} parse: {c}"));
        let name = flags[0];
        let m = machine::unhonoured_flags(&o);
        assert_eq!(
            m.iter().any(|f| f.contains(name)),
            *machine_refuses,
            "machine tier, {flags:?}: {m:?}"
        );
        let i = machine::incus_unhonoured(&o);
        assert_eq!(
            i.iter().any(|f| f.contains(name)),
            *incus_refused,
            "incus tier, {flags:?}: {i:?}"
        );
    }
}

/// ⛔ Each field of `Opts`, named here with no `..`: a field added to `Opts`
/// fails the build of this test until its row in [`TABLE`] (or the note
/// beside it) says what a guest tier does with it.
#[allow(dead_code)]
fn every_field_of_opts_is_classified(o: Opts) {
    let Opts {
        rm: _,                // a guest is disposable on each tier
        detach: _,            // incus keeps the instance; machine refuses -d
        name: _,              // incus names the instance
        env: _,               // TABLE: -e
        workdir: _,           // TABLE: -w
        entrypoint: _,        // the argv of each tier
        platform: _,          // the image of each tier
        pull: _,              // the image of each tier
        insecure: _,          // the pull
        tls_verify: _,        // the pull
        tty: _,               // TABLE: -t
        image: _,             // each tier
        command: _,           // each tier
        user: _,              // TABLE: --user
        hostname: _,          // TABLE: --hostname
        tier: _,              // the dispatch
        limits: _,            // incus keys; each other tier refuses (T-1897)
        qemu_args: _,         // machine only; incus refuses (incus_tier)
        mem: _,               // machine only; incus refuses
        cpus: _,              // machine only; incus refuses
        net_none: _,          // TABLE: --podvm-net
        interactive: _,       // the stdin of each tier
        channel: _,           // machine and incus
        sealed: _,            // machine only; incus refuses
        engine: _,            // machine only; incus refuses
        volume_base: _,       // a bind of the namespace rung; each guest tier refuses
        volume_given: _,      // the check of --podvm-volume-base at the parse
        volume_base_given: _, // the check of --podvm-volume-base at the parse
        profile: _,           // the parse
        sets: _,              // the parse
        wrap: _,              // the argv of each tier
        drops: _,             // the profile
        needs: _,             // the profile
        broker: _,            // machine; incus refuses
        confine: _,           // TABLE: --podvm-confine
        ports: _,             // incus refuses (incus_refuses)
        allow: _,             // the broker
        publish: _,           // a detached Windows guest only (T-1869)
        devices: _,           // incus refuses
        unsafe_host_paths: _, // the rootfs rungs
        volumes: _,           // TABLE: -v
        timeout: _,           // TABLE: --timeout
        read_only: _,         // TABLE: --read-only
        ro_volumes: _,        // -v ...:ro on each tier
        tmpfs: _,             // TABLE: --tmpfs
        tmp_size: _,          // with --tmpfs
        hardening: _,         // TABLE: --ulimit, --security-opt, --cap-drop, --label
        ask: _,               // TABLE: --add-host
    } = o;
}

/// TODO/machine.md T-1889, T-1890: an image of an architecture with no
/// guest refuses by name on the machine tier; linux/amd64 and linux/arm64
/// boot.
#[test]
fn machine_foreign_guest_refuses_another_architecture() {
    let p = |a: &str| Platform {
        os: "linux".into(),
        arch: a.into(),
        variant: None,
    };
    assert!(super::machine::foreign_guest(&p("amd64")).is_none());
    assert!(super::machine::foreign_guest(&p("arm64")).is_none());
    let e = super::machine::foreign_guest(&p("riscv64")).unwrap();
    assert!(e.contains("linux/riscv64") && e.contains("T-1890"), "{e}");
}

/// TODO/enter.md T-1908: `--hostname` takes a host name, in both spellings.
#[test]
fn hostname_flag_takes_a_host_name() {
    for a in [&["--hostname", "unit1"][..], &["--hostname=unit1"][..]] {
        let mut a = a.to_vec();
        a.extend_from_slice(&["img", "true"]);
        let o = parse::parse("run", &v(&a)).unwrap_or_else(|c| panic!("{a:?}: {c}"));
        assert_eq!(o.hostname.as_deref(), Some("unit1"));
    }
    for bad in ["-x", "a b", "a_b", "", &"a".repeat(64)] {
        let r = parse::parse("run", &v(&["--hostname", bad, "img", "true"]));
        assert_eq!(r.err(), Some(EXIT_FLAG_ERROR), "{bad:?}");
    }
}
