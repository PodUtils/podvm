//! TODO/machine.md T-2008: the rules of a saved state of a guest, each as
//! one table: what a state serves, what its fingerprint holds, which files
//! are a state, what a run does with them, and what a save leaves behind.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use super::*;

pub(super) fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "podvm-t2008-{name}-{}-{}",
        std::process::id(),
        init::nonce().unwrap()
    ));
    std::fs::create_dir_all(d.join("machine")).unwrap();
    d
}

/// A program that answers `--version` as an emulator does.
fn emulator(dir: &Path, version: &str) -> PathBuf {
    let p = dir.join("qemu");
    std::fs::write(
        &p,
        format!("#!/bin/sh\necho 'QEMU emulator version {version}'\n"),
    )
    .unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[test]
fn the_variable_takes_three_values() {
    for (var, want) in [
        (None, Ok(Some(Transport::File))),
        (Some("1"), Ok(Some(Transport::File))),
        (Some("pipe"), Ok(Some(Transport::Pipe))),
        (Some("0"), Ok(None)),
    ] {
        assert_eq!(wanted(var), want, "{var:?}");
    }
    for bad in ["", "yes", "off", "2", "file", "PIPE", " 1"] {
        let e = wanted(Some(bad)).unwrap_err();
        assert!(e.contains("the values are 0, 1 and pipe"), "{bad:?}: {e}");
    }
}

/// A state serves a run only where the saved guest has each part of the
/// run. Each other run boots, and the reason names the part.
#[test]
fn a_state_serves_only_the_run_that_its_guest_can_hold() {
    let base = Asks {
        init_image: true,
        native: true,
        pinned_kernel: true,
        volumes: SLOTS,
        stdin: super::super::job::MAX_STDIN,
        ..Asks::default()
    };
    assert_eq!(refusal(&base), None, "the run that a state serves");
    let cases: Vec<(Asks, &str)> = vec![
        (
            Asks {
                init_image: false,
                ..base.clone()
            },
            "no --podvm-init=image",
        ),
        (
            Asks {
                native: false,
                ..base.clone()
            },
            "another architecture",
        ),
        (
            Asks {
                pinned_kernel: false,
                ..base.clone()
            },
            "PODVM_MACHINE_KERNEL",
        ),
        (
            Asks {
                tty: true,
                ..base.clone()
            },
            "-t",
        ),
        (
            Asks {
                detach: true,
                ..base.clone()
            },
            "run -d",
        ),
        (
            Asks {
                channel: true,
                ..base.clone()
            },
            "--podvm-channel",
        ),
        (
            Asks {
                engine: true,
                ..base.clone()
            },
            "--podvm-engine",
        ),
        (
            Asks {
                pids: true,
                ..base.clone()
            },
            "--pids-limit",
        ),
        (
            Asks {
                broker: true,
                ..base.clone()
            },
            "--network broker",
        ),
        (
            Asks {
                copies: true,
                ..base.clone()
            },
            "a copy into the guest",
        ),
        (
            Asks {
                volumes: SLOTS + 1,
                ..base.clone()
            },
            "volume devices",
        ),
        (
            Asks {
                stdin: super::super::job::MAX_STDIN + 1,
                ..base.clone()
            },
            "bytes of stdin",
        ),
    ];
    for (asks, want) in cases {
        let why = refusal(&asks).unwrap_or_else(|| panic!("{want}: the run is served"));
        assert!(why.contains(want), "{want:?} is not in {why:?}");
    }
}

/// Each part of the shape of a guest is in the fingerprint: a change of
/// one gives another text and other files, so a stale state is not found.
#[test]
fn each_part_of_the_shape_changes_the_fingerprint_and_the_files() {
    let d = scratch("print");
    let qemu = emulator(&d, "9.9.9 (test)");
    let kernel = d.join("vmlinuz");
    std::fs::write(&kernel, b"kernel").unwrap();
    let line = vec!["-m".to_string(), "0M".to_string()];
    let shape = |digest: &'static str,
                 accel: Accel,
                 qemu: &Path,
                 kernel: &Path,
                 mem: Option<u64>,
                 line: &[String],
                 script: &'static str| {
        fingerprint(&Shape {
            digest,
            accel,
            qemu,
            kernel,
            mem,
            line,
            script,
        })
        .unwrap()
    };
    let base = shape("sha256:aa", Accel::Tcg, &qemu, &kernel, None, &line, "s");
    assert_eq!(
        base,
        shape("sha256:aa", Accel::Tcg, &qemu, &kernel, None, &line, "s"),
        "the same shape gives the same text"
    );
    for want in [
        "qemu QEMU emulator version 9.9.9 (test)",
        "cpu tcg",
        "mem of-the-initrd",
    ] {
        assert!(base.contains(want), "{want:?} is not in {base}");
    }
    let other_line = vec!["-m".to_string(), "0M".to_string(), "-smp".to_string()];
    let mut others = vec![
        (
            "the digest",
            shape("sha256:ab", Accel::Tcg, &qemu, &kernel, None, &line, "s"),
        ),
        (
            "the accelerator",
            shape("sha256:aa", Accel::Kvm, &qemu, &kernel, None, &line, "s"),
        ),
        (
            "the memory",
            shape(
                "sha256:aa",
                Accel::Tcg,
                &qemu,
                &kernel,
                Some(1 << 30),
                &line,
                "s",
            ),
        ),
        (
            "the line",
            shape(
                "sha256:aa",
                Accel::Tcg,
                &qemu,
                &kernel,
                None,
                &other_line,
                "s",
            ),
        ),
        (
            "the script",
            shape("sha256:aa", Accel::Tcg, &qemu, &kernel, None, &line, "t"),
        ),
    ];
    // A file that changes in place: its length is in the text.
    std::fs::write(&kernel, b"kernel 2").unwrap();
    others.push((
        "the kernel file",
        shape("sha256:aa", Accel::Tcg, &qemu, &kernel, None, &line, "s"),
    ));
    std::fs::write(&kernel, b"kernel").unwrap();
    let qemu2 = emulator(&d.join("machine"), "9.9.9 (test)");
    others.push((
        "the emulator file",
        shape("sha256:aa", Accel::Tcg, &qemu2, &kernel, None, &line, "s"),
    ));
    let qemu3 = emulator(&d, "10.0.0 (test)");
    others.push((
        "the emulator version",
        shape("sha256:aa", Accel::Tcg, &qemu3, &kernel, None, &line, "s"),
    ));
    let own = files(&d, "sha256:aa", &base);
    for (what, text) in &others {
        assert_ne!(*text, base, "{what} is not in the fingerprint");
        let f = files(&d, "sha256:aa", text);
        assert_ne!(f.state, own.state, "{what}: the same state file");
        assert_ne!(f.nonce, own.nonce, "{what}: the same nonce");
    }
    // A part that cannot be read is an error, and the run boots.
    let gone = fingerprint(&Shape {
        digest: "sha256:aa",
        accel: Accel::Tcg,
        qemu: &d.join("no-emulator"),
        kernel: &kernel,
        mem: None,
        line: &line,
        script: "s",
    });
    assert!(gone.is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_sidecar_is_read_whole_or_not_at_all() {
    let meta = Meta {
        fingerprint: "a\nb\n".to_string(),
        mem_mib: 3072,
        bytes: 5,
        transport: Transport::Pipe,
    };
    assert_eq!(Meta::parse(&meta.text()), Ok(meta.clone()));
    let good: serde_json::Value = serde_json::from_str(&meta.text()).unwrap();
    for (member, value, want) in [
        (
            "schema",
            serde_json::json!("podvm-machine-state.v0"),
            "schema",
        ),
        ("fingerprint", serde_json::json!(7), "no fingerprint"),
        ("mem_mib", serde_json::json!(0), "no memory size"),
        ("mem_mib", serde_json::json!(-4), "no memory size"),
        ("mem_mib", serde_json::json!(1u64 << 40), "no memory size"),
        ("bytes", serde_json::json!(0), "no length"),
        ("bytes", serde_json::json!("5"), "no length"),
        ("transport", serde_json::json!("tcp"), "no transport"),
    ] {
        let mut v = good.clone();
        v[member] = value;
        let e = Meta::parse(&v.to_string()).unwrap_err();
        assert!(e.contains(want), "{member}: {want:?} is not in {e:?}");
    }
    for text in ["", "{", "[]", "null", "\u{0}"] {
        assert!(Meta::parse(text).is_err(), "{text:?} was read as a sidecar");
    }
}

/// Write a whole state of `print` into the files.
pub(super) fn put(f: &Files, print: &str, bytes: &[u8]) {
    std::fs::write(&f.state, bytes).unwrap();
    let meta = Meta {
        fingerprint: print.to_string(),
        mem_mib: 512,
        bytes: bytes.len() as u64,
        transport: Transport::File,
    };
    std::fs::write(&f.meta, meta.text()).unwrap();
}

/// What a run does with the files of its fingerprint: it loads only a
/// whole state of the same text; each other case saves a new one or boots.
#[test]
fn a_run_loads_only_a_whole_state_of_its_own_fingerprint() {
    let d = scratch("plan");
    let print = "podvm-machine-state.v1\nqemu 1\n";
    let f = files(&d, "sha256:aa", print);
    let kind = |p: &Plan| match p {
        Plan::Load(_) => "load".to_string(),
        Plan::Save { .. } => "save".to_string(),
        Plan::Boot(why) => format!("boot: {why}"),
    };
    // No state.
    assert_eq!(kind(&plan(&f, print, 1)), "save");
    // A whole state.
    put(&f, print, b"state");
    assert_eq!(kind(&plan(&f, print, 1)), "load");
    // The record of another fingerprint under this name.
    put(&f, "podvm-machine-state.v1\nqemu 2\n", b"state");
    assert_eq!(kind(&plan(&f, print, 1)), "save");
    let e = valid(&f, print).unwrap_err();
    assert!(e.contains("\"qemu 2\"") && e.contains("\"qemu 1\""), "{e}");
    // A file that a crash cut, or that grew.
    for bytes in [&b"sta"[..], b"state and more"] {
        put(&f, print, b"state");
        std::fs::write(&f.state, bytes).unwrap();
        assert_eq!(kind(&plan(&f, print, 1)), "save", "{bytes:?}");
    }
    // A record with no file, and a file with no record.
    put(&f, print, b"state");
    std::fs::remove_file(&f.state).unwrap();
    assert_eq!(kind(&plan(&f, print, 1)), "save");
    put(&f, print, b"state");
    std::fs::remove_file(&f.meta).unwrap();
    assert_eq!(kind(&plan(&f, print, 1)), "save");
    // A record that is no JSON.
    put(&f, print, b"state");
    std::fs::write(&f.meta, b"{").unwrap();
    assert_eq!(kind(&plan(&f, print, 1)), "save");
    // Another run holds the lock of the save: this run boots.
    let held = podvm_image::store::Lock::try_exclusive(&f.lock)
        .unwrap()
        .unwrap();
    assert_eq!(
        kind(&plan(&f, print, 1)),
        "boot: another run saves the state now"
    );
    drop(held);
    // No room for the state.
    let no_room = kind(&plan(&f, print, u64::MAX / 4));
    assert!(
        no_room.starts_with("boot: no room for a state"),
        "{no_room}"
    );
    // The marker of a failed state. Each failure removes the files and
    // counts; the run after one or two failures saves again, and after
    // three in a row the run boots.
    for strike in 1..=STRIKES {
        put(&f, print, b"state");
        mark_bad(&f, "the emulator ended\nline 2");
        assert!(!f.state.exists() && !f.meta.exists());
        let after = kind(&plan(&f, print, 1));
        if strike < STRIKES {
            assert_eq!(after, "save", "after {strike} failures");
        } else {
            assert!(
                after.starts_with(
                    "boot: 3 runs in a row could not use a saved state (the last one: the \
                     emulator ended line 2)"
                ),
                "{after}"
            );
        }
    }
    // A load that works ends the count.
    put(&f, print, b"state");
    std::fs::write(&f.bad, b"2\nonce\n").unwrap();
    touch(&f);
    assert!(!f.bad.exists());
    mark_bad(&f, "again");
    assert_eq!(
        strikes(&f, SystemTime::now()),
        Some((1, "again".to_string()))
    );
    // A marker that is no count holds: no run pays for a state behind it.
    for text in ["", "many\nwhy\n", "\u{0}"] {
        std::fs::write(&f.bad, text).unwrap();
        assert!(marked_bad(&f, SystemTime::now()).is_some(), "{text:?}");
    }
    std::fs::write(&f.bad, b"3\nthe emulator ended line 2\n").unwrap();
    // The marker holds for one day.
    assert!(marked_bad(&f, SystemTime::now() + BAD_HOLDS - Duration::from_secs(60)).is_some());
    assert!(marked_bad(&f, SystemTime::now() + BAD_HOLDS + Duration::from_secs(60)).is_none());
    // A path that QEMU cannot take.
    let comma = files(&d.join("a,b"), "sha256:aa", print);
    let c = kind(&plan(&comma, print, 1));
    assert!(c.contains("has a comma"), "{c}");
    let _ = std::fs::remove_dir_all(&d);
}
