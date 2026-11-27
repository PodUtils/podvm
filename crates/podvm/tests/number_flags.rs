//! TODO/gate.md T-1947: each number that a flag takes is refused outside its
//! range with one true line and the flag-error exit (125), as the binary
//! gives it: never a panic, a wrap or a silent clamp. The unit test of
//! `number` holds the table of each rule; this one pins the line that the
//! user reads, for one value of each family (overflowing, negative, not a
//! number, past a bound, below a bound).

use std::process::{Command, Stdio};

fn podvm(args: &[&str], dir: &std::path::Path) -> (Option<i32>, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_podvm"))
        .args(args)
        .env("PODVM_STORE", dir)
        .env("HOME", dir)
        .env("PODVM_SPARE_TREE", "0")
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn a_number_out_of_its_range_is_one_line_and_125() {
    let d = std::env::temp_dir().join(format!("podvm-t1947-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let big = "99999999999999999999";
    // (the arguments, the words that the line holds: the verb, the flag, the
    // value, the bound)
    let rows: Vec<(Vec<&str>, Vec<&str>)> = vec![
        (
            vec!["run", "--timeout", big, "i"],
            vec!["podvm run:", "--timeout", big, "18446744073709551"],
        ),
        (
            vec!["run", "--timeout", "0", "i"],
            vec!["--timeout", "\"0\""],
        ),
        (
            vec!["run", "--pids-limit", "-1", "i"],
            vec!["--pids-limit", "\"-1\"", "4194304"],
        ),
        (
            vec!["run", "--pids-limit", big, "i"],
            vec!["--pids-limit", big, "too large"],
        ),
        (
            vec!["run", "--cpus", "nan", "i"],
            vec!["--cpus", "\"nan\"", "1024"],
        ),
        (
            vec!["run", "--cpus", "0.001", "i"],
            vec!["--cpus", "\"0.001\"", "0.01"],
        ),
        (
            vec!["run", "--memory", "99999999999999999999g", "i"],
            vec!["--memory", "too large", "9223372036854775807"],
        ),
        (
            vec!["run", "-m", "1k", "i"],
            vec!["--memory", "\"1k\"", "6291456"],
        ),
        (
            vec!["run", "--storage-opt", "size=1m", "i"],
            vec!["--storage-opt size", "67108864"],
        ),
        (
            vec!["run", "--podvm-mem", "-1", "i"],
            vec!["--podvm-mem", "\"-1\""],
        ),
        (
            vec!["run", "--podvm-cpus", "65", "i"],
            vec!["--podvm-cpus", "\"65\"", "64"],
        ),
        (
            vec!["run", "--tmpfs", "/t:size=-1", "i"],
            vec!["--tmpfs size", "\"-1\""],
        ),
        (
            vec![
                "run",
                "--network",
                "broker",
                "--allow",
                "h",
                "--ports",
                "70000",
                "i",
            ],
            vec!["--ports", "\"70000\"", "65535"],
        ),
        (
            vec!["run", "-p", "70000:80", "i"],
            vec!["-p", "\"70000\"", "65535"],
        ),
        (
            vec!["run", "--ulimit", "nofile=99999999999999999999", "i"],
            vec!["--ulimit", big, "from 0 to"],
        ),
        (
            vec!["stop", "-t", "-2", "x"],
            vec!["podvm stop:", "-t", "\"-2\"", "from 0 to"],
        ),
        // `-t -1` (docker's "wait for ever") has its own line (T-2108).
        (
            vec!["stop", "-t", "-1", "x"],
            vec!["podvm stop:", "-t -1", "wait for ever", "-t 3600"],
        ),
        (vec!["stop", "-t", "abc", "x"], vec!["-t", "\"abc\""]),
        (vec!["stop", "-t", big, "x"], vec!["-t", big, "too large"]),
        (vec!["stop", "-t"], vec!["-t needs a value"]),
        (
            vec!["kill", "-s", big, "x"],
            vec!["--signal", big, "1 to 64"],
        ),
        (vec!["kill", "-s", "-1", "x"], vec!["--signal", "\"-1\""]),
        (vec!["logs", "--tail", "-1", "x"], vec!["--tail", "\"-1\""]),
        (
            vec!["logs", "--limit-bytes", big, "x"],
            vec!["--limit-bytes", big, "9223372036854775807"],
        ),
        (
            vec!["windows", "run", "--max-bytes", "0"],
            vec!["--max-bytes", "\"0\""],
        ),
        (
            vec!["windows", "run", "--podvm-cpus", "0"],
            vec!["--podvm-cpus", "\"0\"", "64"],
        ),
        (
            vec!["notebook", "n.ipynb", "--persist", "/p", "--cycles", "0"],
            vec!["--cycles", "\"0\"", "1000"],
        ),
    ];
    for (args, words) in rows {
        let (rc, err) = podvm(&args, &d);
        assert_eq!(rc, Some(125), "{args:?}: {err}");
        assert!(
            !err.contains("internal error") && !err.contains("panicked"),
            "{args:?}: {err}"
        );
        let line = err
            .lines()
            .find(|l| l.starts_with("podvm "))
            .unwrap_or_else(|| panic!("{args:?}: no line of podvm in {err:?}"));
        for w in words {
            assert!(line.contains(w), "{args:?}: {line:?} lacks {w:?}");
        }
    }
    let _ = std::fs::remove_dir_all(&d);
}
