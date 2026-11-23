//! TODO/machine.md T-2077: the `oom_score_adj` of PID 1 of an instance and
//! of its command, against one table.

use super::oom::{parse, set_in_child, Guard, MAX, MIN};
use std::path::PathBuf;

fn scratch() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let d = std::env::temp_dir().join(format!("podvm-t2077-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A case, the text before (None: no file), the path, the guard, and a
/// word of the reason.
type Row = (
    &'static str,
    Option<&'static str>,
    PathBuf,
    Guard,
    Option<&'static str>,
);

/// One rule for each file of PID 1: where the write of -1000 holds, PID 1
/// is immune and the command gets the value before (the value of the
/// kernel for one that is not a value, or that is -1000 already); where
/// the write fails or does not hold, PID 1 keeps its value, the command
/// gets 1000, and the reason names the file.
#[test]
fn pid1_takes_the_lowest_value_and_the_command_one_that_the_killer_takes_first() {
    let d = scratch();
    let file = |n: &str| d.join(n);
    let rows: Vec<Row> = vec![
        (
            "zero",
            Some("0\n"),
            file("a"),
            Guard::Immune { base: 0 },
            None,
        ),
        (
            "negative",
            Some("-500\n"),
            file("b"),
            Guard::Immune { base: -500 },
            None,
        ),
        (
            "positive",
            Some("300"),
            file("c"),
            Guard::Immune { base: 300 },
            None,
        ),
        (
            "immune already",
            Some("-1000\n"),
            file("d"),
            Guard::Immune { base: 0 },
            None,
        ),
        (
            "not a value",
            Some("x\n"),
            file("e"),
            Guard::Immune { base: 0 },
            None,
        ),
        (
            "out of range",
            Some("5000\n"),
            file("f"),
            Guard::Immune { base: 0 },
            None,
        ),
        (
            "no directory",
            None,
            d.join("none/oom_score_adj"),
            Guard::Last { base: 0 },
            Some("-1000"),
        ),
        (
            "a directory",
            None,
            d.clone(),
            Guard::Last { base: 0 },
            Some("-1000"),
        ),
        (
            "the write does not hold",
            None,
            PathBuf::from("/dev/null"),
            Guard::Last { base: 0 },
            Some("gave no value"),
        ),
    ];
    for (case, before, path, want, why) in &rows {
        if let Some(t) = before {
            std::fs::write(path, t).unwrap();
        }
        let (got, reason) = Guard::take(path);
        assert_eq!(got, *want, "{case}");
        match (why, &reason) {
            (None, None) => {
                assert_eq!(
                    std::fs::read_to_string(path).ok().and_then(|t| parse(&t)),
                    Some(MIN),
                    "{case}: PID 1 is not at -1000"
                );
            }
            (Some(w), Some(r)) => {
                assert!(r.contains(w), "{case}: {r}");
                assert!(r.contains(&path.display().to_string()), "{case}: {r}");
            }
            _ => panic!("{case}: reason {reason:?}, wanted {why:?}"),
        }
        let child = got.child();
        assert!((MIN + 1..=MAX).contains(&child), "{case}: {child}");
        match got {
            Guard::Immune { base } => assert!(got.must_set() && child == base, "{case}"),
            Guard::Last { .. } => assert!(!got.must_set() && child == MAX, "{case}"),
        }
        assert_eq!(
            got.child_text(),
            format!("{child}\n").into_bytes(),
            "{case}"
        );
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// The write in the child before the `exec`: a value that the kernel takes
/// is the value of the command; a value that it refuses stops the start
/// only where the write must hold. 1000 is a raise, which each process may
/// make.
#[test]
fn the_command_starts_with_its_value_or_not_at_all() {
    use std::os::unix::process::CommandExt;
    let run = |text: &'static [u8], must: bool| {
        let mut c = std::process::Command::new("/bin/cat");
        c.arg("/proc/self/oom_score_adj");
        // SAFETY: the closure allocates nothing.
        unsafe { c.pre_exec(move || set_in_child(text, must)) };
        c.output()
    };
    let out = run(b"1000\n", true).expect("a raise to 1000 is refused");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "1000");
    let out = run(b"not a value\n", false).expect("a refused write stopped the start");
    assert!(out.status.success());
    let e = run(b"not a value\n", true).expect_err("a refused write that must hold started");
    assert_eq!(e.raw_os_error(), Some(22), "{e}");
    for (text, want) in [
        ("0", Some(0)),
        (" -1000\n", Some(MIN)),
        ("1001", None),
        ("", None),
    ] {
        assert_eq!(parse(text), want, "{text:?}");
    }
}
