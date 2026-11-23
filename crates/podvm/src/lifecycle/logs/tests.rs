//! TODO/cli.md T-2046: the parse of `logs` and the copy of a log, each
//! held by one table.

use super::*;

fn v(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

/// Each value of `--limit-bytes` and `--tail`, in both spellings: a valid
/// one is the number, and each other one is a flag error (125).
#[test]
fn the_values_of_logs_are_parsed_by_one_rule() {
    let max = LIMIT_BYTES_MAX.to_string();
    let past = (LIMIT_BYTES_MAX + 1).to_string();
    // (flag, value, the number, or None for a refusal)
    let cases: &[(&str, &str, Option<u64>)] = &[
        ("--limit-bytes", "1", Some(1)),
        ("--limit-bytes", "1000", Some(1000)),
        ("--limit-bytes", "007", Some(7)),
        ("--limit-bytes", &max, Some(LIMIT_BYTES_MAX)),
        ("--limit-bytes", &past, None),
        ("--limit-bytes", "18446744073709551616", None),
        ("--limit-bytes", "0", None),
        ("--limit-bytes", "-1", None),
        ("--limit-bytes", "+5", None),
        ("--limit-bytes", "1k", None),
        ("--limit-bytes", "1.5", None),
        ("--limit-bytes", " 5", None),
        ("--limit-bytes", "", None),
        ("--limit-bytes", "x", None),
        ("--tail", "0", Some(0)),
        ("--tail", "5", Some(5)),
        ("--tail", "18446744073709551615", Some(u64::MAX)),
        ("--tail", "18446744073709551616", None),
        ("--tail", "-1", None),
        ("--tail", "ALL", None),
        ("--tail", "", None),
    ];
    for &(flag, raw, want) in cases {
        let joined = format!("{flag}={raw}");
        for argv in [v(&[flag, raw, "c1"]), v(&[&joined, "c1"])] {
            match (parse_logs(&argv), want) {
                (Ok(o), Some(n)) => {
                    let got = if flag == "--tail" { o.tail } else { o.limit };
                    assert_eq!(got, Some(n), "{argv:?}");
                    assert_eq!(o.want, "c1", "{argv:?}");
                }
                (Err(c), None) => assert_eq!(c, EXIT_FLAG_ERROR, "{argv:?}"),
                (r, _) => panic!("{argv:?} gave {r:?}, want {want:?}"),
            }
        }
        // The message names the flag and the value.
        if want.is_none() {
            let why = value(flag, raw).unwrap_err();
            assert!(
                why.starts_with(flag) && why.contains(&format!("{raw:?}")),
                "{why}"
            );
        }
    }
    // A flag with no value is refused by name.
    for flag in ["--limit-bytes", "--tail"] {
        assert_eq!(parse_logs(&v(&["c1", flag])).unwrap_err(), EXIT_FLAG_ERROR);
    }
}

/// The combinations of `--limit-bytes`: each one that `logs` takes, with
/// what it gives. `--since`, `--until`, `-t` and `--details` have no row
/// in the parity table, so `admit` refuses them before this parser.
#[test]
fn limit_bytes_composes_with_each_flag_of_logs() {
    /// The argv, and the follow, the tail and the limit that it gives.
    type Case<'a> = (&'a [&'a str], bool, Option<u64>, Option<u64>);
    let cases: &[Case] = &[
        (&["--limit-bytes", "10", "c1"], false, None, Some(10)),
        (&["-f", "--limit-bytes", "10", "c1"], true, None, Some(10)),
        (
            &["c1", "--tail", "3", "--limit-bytes=4"],
            false,
            Some(3),
            Some(4),
        ),
        (
            &["--follow", "--tail=0", "--limit-bytes", "1", "c1"],
            true,
            Some(0),
            Some(1),
        ),
        // The last value of a flag wins, as docker reads a flag given twice.
        (
            &["--limit-bytes", "9", "--limit-bytes", "2", "c1"],
            false,
            None,
            Some(2),
        ),
    ];
    for (argv, follow, tail, limit) in cases {
        let o = parse_logs(&v(argv)).unwrap();
        assert_eq!(
            (o.follow, o.tail, o.limit),
            (*follow, *tail, *limit),
            "{argv:?}"
        );
    }
    for flag in ["--since", "--until", "-t", "--timestamps", "--details"] {
        assert!(
            crate::parity::admit("logs", flag, LOGS_USAGE).is_err(),
            "{flag}"
        );
    }
}

/// The help names the bound of the buffer and the range of the flag. The
/// incus tier reads in pieces too, so the help names no bound of the size
/// of a log (podcrates `v0.81.0`, PodUtils/podvm#631).
#[test]
fn the_help_states_the_bounds() {
    for want in [
        "--limit-bytes N",
        "64 KiB",
        "of any size, read\n                   in pieces of 64 KiB",
        &LIMIT_BYTES_MAX.to_string(),
    ] {
        assert!(LOGS_USAGE.contains(want), "{want}");
    }
    assert!(!LOGS_USAGE.contains("MiB"), "a bound of the size of a log");
}

/// The copy of a log for each `--tail` and `--limit-bytes`: the bytes of
/// the slice of the whole read.
#[test]
fn the_copy_of_a_log_is_the_slice_of_the_whole_read() {
    let text = b"one\ntwo\nthree\nfour\n";
    let dir = std::env::temp_dir().join(format!("podvm-logs-{}", podvm_supervise::new_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("log");
    std::fs::write(&path, text).unwrap();
    let cases: &[(Option<u64>, Option<u64>, &[u8])] = &[
        (None, None, text),
        (None, Some(5), b"one\nt"),
        (Some(2), None, b"three\nfour\n"),
        (Some(2), Some(3), b"thr"),
        (Some(0), Some(3), b""),
        (Some(9), Some(1000), text),
    ];
    for &(tail, limit, want) in cases {
        let o = LogsArgs {
            follow: false,
            tail,
            limit,
            want: "c1".into(),
        };
        let mut f = std::fs::File::open(&path).unwrap();
        let mut out = Vec::new();
        let n = copy_log(&mut f, text.len() as u64, &o, &mut out).unwrap();
        assert_eq!(out, want, "{tail:?} {limit:?}");
        assert_eq!(n, want.len() as u64);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// TODO/cli.md T-2061: docker's `--tail all` is the whole log, as the
/// default is, and a later `--tail N` or `all` wins, as in docker.
#[test]
fn tail_all_is_the_whole_log() {
    for argv in [
        v(&["--tail", "all", "c1"]),
        v(&["--tail=all", "c1"]),
        v(&["--tail", "5", "--tail", "all", "c1"]),
    ] {
        assert_eq!(parse_logs(&argv).unwrap().tail, None, "{argv:?}");
    }
    let last = parse_logs(&v(&["--tail", "all", "--tail", "5", "c1"])).unwrap();
    assert_eq!(last.tail, Some(5));
}
