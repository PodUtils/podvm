//! TODO/machine.md T-2008: the sweep of the store, the wait for a loaded
//! guest, and the two scripts of a guest of a state. The save against a
//! stand-in of the monitor is in `tests_monitor`.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::time::{Duration, SystemTime};

use super::super::vol;
use super::tests::{put, scratch};
use super::*;

/// The store keeps the states of one image that were used last, and no
/// partial file of a save that ended.
#[test]
fn the_sweep_keeps_the_last_states_of_an_image_and_no_old_partial_file() {
    let d = scratch("sweep");
    let dir = d.join("machine");
    let old = |p: &Path, secs: u64| {
        let f = std::fs::OpenOptions::new().write(true).open(p).unwrap();
        f.set_modified(SystemTime::now() - Duration::from_secs(secs))
            .unwrap();
    };
    let own = files(&d, "sha256:aa", "own");
    let mut same_image = Vec::new();
    for (i, print) in ["one", "two", "three"].iter().enumerate() {
        let f = files(&d, "sha256:aa", print);
        put(&f, print, b"state");
        std::fs::write(&f.lock, b"").unwrap();
        old(&f.meta, 100 * (i as u64 + 1));
        same_image.push(f);
    }
    // A state of the layout before T-2051: its record is JSON.
    let old_layout = dir.join("state-aa-000000000000000000000000.json");
    std::fs::write(&old_layout, b"{}").unwrap();
    std::fs::write(old_layout.with_extension("vm"), b"state").unwrap();
    old(&old_layout, 10_000);
    let other_image = files(&d, "sha256:bb", "one");
    put(&other_image, "one", b"state");
    old(&other_image.meta, 10_000);
    let own_part = own.state.with_extension("vm.part-0123");
    let fresh_part = same_image[0].state.with_extension("vm.part-4567");
    let old_part = same_image[0].state.with_extension("vm.part-89ab");
    let base = dir.join("base-aa-v3.cpio");
    // The lock of a state that went (a marked one), old and new.
    let old_lock = files(&d, "sha256:cc", "gone").lock;
    let new_lock = files(&d, "sha256:cc", "now").lock;
    for p in [
        &own_part,
        &fresh_part,
        &old_part,
        &base,
        &old_lock,
        &new_lock,
    ] {
        std::fs::write(p, b"x").unwrap();
    }
    old(&old_part, BAD_HOLDS.as_secs() + 60);
    old(&old_lock, BAD_HOLDS.as_secs() + 60);
    sweep(&dir, &own);
    assert!(!own_part.exists(), "a partial file of this state stays");
    assert!(!old_part.exists(), "an old partial file stays");
    assert!(
        fresh_part.exists(),
        "the partial file of a save that can run went"
    );
    assert!(base.exists(), "a file that is no state went");
    assert!(!old_lock.exists(), "the old lock of no state stays");
    assert!(new_lock.exists(), "a lock that a save can hold went");
    assert!(
        other_image.state.exists(),
        "the state of another image went"
    );
    assert!(
        !old_layout.exists() && !old_layout.with_extension("vm").exists(),
        "a state of the old layout stays"
    );
    // KEEP is 2: this state and the one that was used last.
    assert!(same_image[0].state.exists() && same_image[0].meta.exists());
    for f in &same_image[1..] {
        for p in [&f.state, &f.meta, &f.lock] {
            assert!(!p.exists(), "{} stays", p.display());
        }
    }
    // A use moves a state to the front.
    put(&same_image[2], "three", b"state");
    old(&same_image[2].meta, 5000);
    touch(&same_image[2]);
    sweep(&dir, &own);
    assert!(
        same_image[2].state.exists(),
        "the state that was used now went"
    );
    assert!(!same_image[0].state.exists());
    let _ = std::fs::remove_dir_all(&d);
}

/// The wait for a loaded guest: it starts a guest that was saved stopped,
/// and it does not take another status for a running guest.
#[test]
fn the_wait_for_a_loaded_guest_ends_only_when_it_runs() {
    for (statuses, want) in [
        (
            vec!["inmigrate", "inmigrate", "running"],
            Ok(vec!["query-status"; 3]),
        ),
        (
            vec!["inmigrate", "paused", "running"],
            Ok(vec!["query-status", "query-status", "cont", "query-status"]),
        ),
        (
            vec!["inmigrate", "internal-error"],
            Err("\"internal-error\""),
        ),
        (vec!["inmigrate"], Err("qmp")),
    ] {
        let d = scratch("running");
        let sd = crate::test_socket::SocketDir::new("t2008");
        let sock = sd.join("qmp");
        let l = UnixListener::bind(&sock).unwrap();
        let script = statuses.clone();
        let server = std::thread::spawn(move || {
            let (s, _) = l.accept().unwrap();
            let mut w = s.try_clone().unwrap();
            w.write_all(b"{\"QMP\": {}}\n").unwrap();
            let mut seen = Vec::new();
            let mut next = script.into_iter();
            for line in BufReader::new(s).lines().map_while(Result::ok) {
                let v: serde_json::Value = serde_json::from_str(&line).unwrap();
                let verb = v["execute"].as_str().unwrap().to_string();
                let answer = if verb == "query-status" {
                    match next.next() {
                        Some(st) => serde_json::json!({"return": {"status": st}}),
                        // The emulator ended.
                        None => break,
                    }
                } else {
                    serde_json::json!({"return": {}})
                };
                if verb != "qmp_capabilities" {
                    seen.push(verb);
                }
                if w.write_all(format!("{answer}\n").as_bytes()).is_err() {
                    break;
                }
            }
            seen
        });
        let got = running(&sock);
        let seen = server.join().unwrap();
        match want {
            Ok(verbs) => {
                assert_eq!(got, Ok(()), "{statuses:?}");
                assert_eq!(seen, verbs, "{statuses:?}");
            }
            Err(text) => {
                let e = got.unwrap_err();
                assert!(e.contains(text), "{text:?} is not in {e:?}");
            }
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}

/// The guest of a state and the job of a run: the script of the guest has
/// nothing of a run, and the job has no word of the guest's first lines.
/// An argument that holds words of the script stays as it is.
#[test]
fn the_script_of_the_guest_and_the_job_of_a_run_are_two_texts() {
    let t = init::template("TNONCE", true);
    for want in [
        "mount -t proc proc /proc",
        "/podvm/podvm guest-net\n/podvm/podvm guest-9p\n",
        "/podvm/podvm guest-init --hold /podvm/hold --cgroup auto\nh=$?\n",
        "/podvm/podvm guest-job TNONCE $h /podvm/job.sh || {",
        ". /podvm/job.sh\n",
    ] {
        assert!(t.contains(want), "{want:?} is not in the script:\n{t}");
    }
    assert!(!init::template("T", false).contains("guest-net\n"));
    assert!(init::template("T", false).contains("/podvm/podvm guest-9p\n"));
    let argv: Vec<String> = [
        "sh",
        "-c",
        "exec \"$@\"; /podvm/podvm guest-9p x",
        "sh",
        "true",
    ]
    .map(String::from)
    .to_vec();
    let env = vec![
        "A=the \"$@\" of a variable".to_string(),
        crate::initns::MARK.to_string(),
    ];
    let vols = vol::vols(&[("/data".to_string(), false), ("/ro".to_string(), true)]);
    let j = init::job("RNONCE", &argv, &env, "/w", &vols, None, None);
    for want in [
        "/podvm/podvm guest-9p --hold /podvm/hold 'podvm0=/data' 'podvm1=/ro:ro' 2>/podvm-9p.err",
        "export A='the \"$@\" of a variable'\n",
        "echo PODVM-READY-RNONCE\n",
        "set -- 'sh' '-c' 'exec \"$@\"; /podvm/podvm guest-9p x' 'sh' 'true'\n",
        "/podvm/podvm guest-init --attach /podvm/hold --cwd \"$PWD\" -- \"$@\" >/podvm-out 2>/podvm-err\n",
        "echo PODVM-EXIT-RNONCE $rc $o\n",
    ] {
        assert!(j.contains(want), "{want:?} is not in the job:\n{j}");
    }
    for not in ["#!/bin/sh", "mount -t", "guest-net", "--cgroup auto"] {
        assert!(!j.contains(not), "{not:?} is in the job:\n{j}");
    }
    // The same rule for a guest that boots its init with the run (T-2002).
    let plain = init::with_image_init(
        init::script("N", &argv, &env, "/w", false, &[], None, None),
        true,
    );
    assert!(plain.contains("set -- 'sh' '-c' 'exec \"$@\"; /podvm/podvm guest-9p x' 'sh' 'true'\n"));
    assert!(plain.contains("export A='the \"$@\" of a variable'\n"));
    assert!(plain.contains("--cgroup auto --cwd \"$PWD\" -- \"$@\" >/podvm-out"));
}

/// TODO/machine.md T-2008: a failure of the save names its cause. A
/// request with no answer within its bound is not a refusal and not an end
/// of the connection: the line says what the emulator does then. The text
/// of `pod_guest` (the request, the text of the emulator) stays whole.
#[test]
fn a_failure_of_the_save_names_the_request_and_the_reason() {
    use pod_core::{Class, Failure, Stage};
    for (class, text, lock) in [
        (
            Class::Timeout,
            "the save did not end within 180s: qmp: no answer to {\"execute\":\"query-migrate\"} within 175.0s",
            true,
        ),
        (
            Class::Protocol,
            "the save failed: Unable to write to file: No space left on device",
            false,
        ),
        (
            Class::Unavailable,
            "the emulator cannot save this guest: \"State blocked by non-migratable CPU device (invtsc flag)\"",
            false,
        ),
        (Class::Unavailable, "qmp closed the connection", false),
    ] {
        let line = super::save::explain(&Failure::new(Stage::Host, class, text));
        assert!(line.contains(text), "the text of the crate is not in {line:?}");
        assert_eq!(line.contains("holds its lock"), lock, "{line:?}");
    }
}
