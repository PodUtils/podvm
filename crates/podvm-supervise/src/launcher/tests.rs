/// ⛔ T-0603, asserted rather than commented. `PR_SET_PDEATHSIG` fires on
/// the creating THREAD's exit, so the path that clones, chroots and execs
/// has to be the process.
///
/// ⚠ **This asserts on the SOURCE, and the first version of it asserted on
/// a thread count and was flaky.** Counting `/proc/self/task` before and
/// after does not work under `cargo test`, which runs tests in threads of
/// one process and changes that count for reasons that have nothing to do
/// with podvm: it failed once in about five runs, which is a test whose
/// name claimed more than it checked. The constraint is that these two
/// crates spawn no thread at all, and that is what is read here. The
/// runtime half is clause 3 of `experiments/230-lifecycle-loop.sh`, which
/// reads `/proc/<launcher>/task` of a real detached launcher.
#[test]
fn nothing_on_the_spawn_path_can_spawn_a_thread() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/");
    // ⛔ THE NEEDLES ARE ASSEMBLED AT RUN TIME AND NEVER WRITTEN OUT HERE.
    // This test reads its own file, so a literal `thread` + `::spawn` in
    // the assertion below is a match against itself: it failed exactly that
    // way on 2026-09-09, and it is the same trap podvm-plant carries
    // three times over.
    let needles = [
        format!("thread{}spawn", "::"),
        format!("thread{}Builder", "::"),
    ];
    let mut looked = 0;
    for crate_name in ["podvm-supervise", "podvm-enter"] {
        let dir = root.join(crate_name).join("src");
        for f in std::fs::read_dir(&dir).expect("the crate's src/") {
            let path = f.expect("an entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("a source file");
            looked += 1;
            for (n, line) in text.lines().enumerate() {
                // ⚠ The doc comment above says the words; a line that is a
                // comment is not code.
                if line.trim_start().starts_with("//") {
                    continue;
                }
                for needle in &needles {
                    assert!(
                        !line.contains(needle),
                        "{}:{} spawns a thread on the path that clones and execs",
                        path.display(),
                        n + 1
                    );
                }
            }
        }
    }
    assert!(looked >= 4, "only {looked} sources were read");
}

/// TODO/enter.md T-2002: the launcher starts the supervisor of an init of
/// the image only for a record of the namespace rung with the mark of the
/// flag: a value of the caller that looks like the mark starts none.
#[test]
fn only_a_record_of_the_namespace_rung_with_the_mark_boots_the_init() {
    for (rung, env, want) in [
        ("namespace", &["A=1", "PODVM_INIT=image"][..], true),
        ("namespace", &["A=1"][..], false),
        ("namespace", &["PODVM_INIT=images"][..], false),
        ("namespace", &["XPODVM_INIT=image"][..], false),
        ("namespace", &["PODVM_INIT_LOG=1"][..], false),
        ("userland", &["PODVM_INIT=image"][..], false),
        ("chroot", &["PODVM_INIT=image"][..], false),
        ("", &["PODVM_INIT=image"][..], false),
    ] {
        let c: crate::table::Container = serde_json::from_value(serde_json::json!({
            "id": "abc", "name": "n", "argv": ["true"], "env": env,
            "working_dir": "", "created_at": "", "started_at": null, "finished_at": null,
            "state": "created", "pid": null, "launcher_pid": null, "exit_code": null,
            "noticed": null, "image": "img", "manifest_digest": "sha256:0",
            "rootfs": "/r", "rung": rung
        }))
        .expect("a record of the table");
        assert_eq!(super::init_of_image(&c), want, "{rung} {env:?}");
    }
}

/// TODO/cli.md T-2070 (OR-B38-12): the exit code of a failed spawn rides in
/// the text that the launcher sends, and `start` takes it out again. One
/// table: a failure of each exit class, and texts that carry no valid mark.
#[test]
fn a_failed_spawn_keeps_the_exit_code_of_a_foreground_run() {
    use super::{spawn_failure, take_code};
    use podvm_enter::Error as E;
    let rows: Vec<(podvm_enter::Error, i32)> = vec![
        (
            E::NotFound("\"/nonexistent\": nothing to start".into()),
            127,
        ),
        (
            E::CannotInvoke("\"/etc/passwd\": not invocable".into()),
            126,
        ),
    ];
    for (err, code) in rows {
        let sentence = err.to_string();
        let mut text = spawn_failure(err);
        assert_ne!(text, sentence, "the mark is in the text");
        assert_eq!(take_code(&mut text), Some(code), "{text}");
        assert_eq!(text, sentence, "the sentence stays whole");
    }
    for plain in [
        "",
        "the payload could not be started",
        "\u{1}exit=x;y",
        "\u{1}exit=127",
    ] {
        let mut text = plain.to_string();
        assert_eq!(take_code(&mut text), None, "{plain:?}");
        assert_eq!(text, plain, "a text with no valid mark stays as it is");
    }
}
