//! TODO/machine.md T-2076 (PodUtils/podrunner#345): the usage file of a
//! stopped instance and the files of a cgroup, against one table each.

use super::inspect::usage_of_file;
use super::usage::{bytes, cpu_usec_v1, cpu_usec_v2, Measure, MAX};
use pod_incus::{FileContent, FileKind};

/// A case, the answer of incusd, CpuSeconds, PeakMemoryBytes, OomKills
/// and a word of the reason.
type Row = (
    &'static str,
    (FileKind, FileContent),
    Option<f64>,
    Option<i64>,
    Option<u64>,
    Option<&'static str>,
);

/// One rule for each answer of incusd for the usage file: a value is a
/// number only where the file is a file of version 1 that gives it; each
/// other answer gives a dash and names why.
#[test]
fn the_usage_of_a_stopped_instance_comes_from_its_file() {
    let file = |s: &str| (FileKind::File, FileContent::Bytes(s.as_bytes().to_vec()));
    let big = format!("{{\"v\":1,\"why\":[\"{}\"]}}", "x".repeat(MAX));
    // (case, answer, CpuSeconds, PeakMemoryBytes, OomKills, a word of why)
    let rows: Vec<Row> = vec![
        (
            "values",
            file(r#"{"v":1,"cpu_usec":2300000,"peak_bytes":67108864,"oom_kills":1,"why":[]}"#),
            Some(2.3),
            Some(67_108_864),
            Some(1),
            None,
        ),
        (
            "zero values are values",
            file(r#"{"v":1,"cpu_usec":0,"peak_bytes":0,"oom_kills":0}"#),
            Some(0.0),
            Some(0),
            Some(0),
            None,
        ),
        (
            "a value the init could not read",
            file(
                r#"{"v":1,"cpu_usec":5,"peak_bytes":null,"oom_kills":0,"why":["memory.peak: gone"]}"#,
            ),
            Some(0.000005),
            None,
            Some(0),
            Some("memory.peak: gone"),
        ),
        (
            "an empty file",
            file(""),
            None,
            None,
            None,
            Some("not JSON"),
        ),
        (
            "not JSON",
            file("cpu 3"),
            None,
            None,
            None,
            Some("not JSON"),
        ),
        (
            "not an object",
            file("[1]"),
            None,
            None,
            None,
            Some("not a JSON object"),
        ),
        (
            "no version",
            file(r#"{"cpu_usec":1}"#),
            None,
            None,
            None,
            Some("no version"),
        ),
        (
            "another version",
            file(r#"{"v":2,"cpu_usec":1}"#),
            None,
            None,
            None,
            Some("version 2"),
        ),
        (
            "a negative count",
            file(r#"{"v":1,"cpu_usec":-1}"#),
            None,
            None,
            None,
            Some("cpu_usec is not a count"),
        ),
        (
            "a text count",
            file(r#"{"v":1,"oom_kills":"1"}"#),
            None,
            None,
            None,
            Some("oom_kills is not a count"),
        ),
        (
            "a fraction",
            file(r#"{"v":1,"peak_bytes":1.5}"#),
            None,
            None,
            None,
            Some("peak_bytes is not a count"),
        ),
        (
            "a peak above i64",
            file(r#"{"v":1,"cpu_usec":1,"peak_bytes":18446744073709551615,"oom_kills":0}"#),
            Some(0.000001),
            None,
            Some(0),
            Some("gives no value"),
        ),
        (
            "too large",
            file(&big),
            None,
            None,
            None,
            Some("more than 4096"),
        ),
        (
            "a directory",
            (FileKind::Directory, FileContent::Entries(vec!["x".into()])),
            None,
            None,
            None,
            Some("is a directory"),
        ),
        (
            "a link",
            (
                FileKind::Symlink,
                FileContent::Bytes(b"/tmp/forged".to_vec()),
            ),
            None,
            None,
            None,
            Some("is a symlink"),
        ),
    ];
    for (case, (kind, content), cpu, peak, oom, why) in rows {
        let (u, w) = usage_of_file(kind, &content);
        assert_eq!(u.cpu_s, cpu, "{case}");
        assert_eq!(u.peak_bytes, peak, "{case}");
        assert_eq!(u.oom_kills, oom, "{case}");
        match why {
            Some(word) => assert!(
                w.as_deref().is_some_and(|w| w.contains(word)),
                "{case}: {w:?}"
            ),
            None => assert_eq!(w, None, "{case}"),
        }
    }
}

/// The file that the init writes reads back as the same measure, and a
/// control character of a reason does not reach the terminal.
#[test]
fn the_usage_file_reads_back() {
    let m = Measure {
        cpu_usec: Some(2_345_678),
        peak_bytes: Some(70_000_000),
        oom_kills: Some(0),
        why: vec!["a\u{1b}[2Jb".into()],
    };
    let back = Measure::parse(&m.to_json()).unwrap();
    assert_eq!(back.cpu_usec, m.cpu_usec);
    assert_eq!(back.peak_bytes, m.peak_bytes);
    assert_eq!(back.oom_kills, m.oom_kills);
    assert_eq!(back.why, vec!["a?[2Jb".to_string()]);
    let none = Measure::parse(&Measure::default().to_json()).unwrap();
    assert_eq!(none, Measure::default());
}

/// The texts of the files of a cgroup, v2 and v1: one rule for each file.
#[test]
fn the_files_of_a_cgroup_give_their_counts() {
    let oom = podvm_supervise::report::oom_kills_in;
    // (file, text, value)
    let rows: &[(&str, &str, Option<u64>)] = &[
        (
            "cpu.stat",
            "usage_usec 2300123\nuser_usec 2000000\nsystem_usec 300123\n",
            Some(2_300_123),
        ),
        ("cpu.stat", "user_usec 1\n", None),
        ("cpu.stat", "usage_usec x\n", None),
        ("cpu.stat", "", None),
        ("cpuacct.usage", "2300123456\n", Some(2_300_123)),
        ("cpuacct.usage", "0\n", Some(0)),
        ("cpuacct.usage", "-5\n", None),
        ("memory.peak", "67108864\n", Some(67_108_864)),
        ("memory.peak", "max\n", None),
        (
            "memory.events",
            "low 0\nhigh 0\nmax 9\noom 2\noom_kill 2\noom_group_kill 0\n",
            Some(2),
        ),
        ("memory.events", "low 0\n", None),
        (
            "memory.oom_control",
            "oom_kill_disable 0\nunder_oom 0\noom_kill 3\n",
            Some(3),
        ),
        (
            "memory.oom_control",
            "oom_kill_disable 0\nunder_oom 0\n",
            None,
        ),
    ];
    for (file, text, want) in rows {
        let got = match *file {
            "cpu.stat" => cpu_usec_v2(text),
            "cpuacct.usage" => cpu_usec_v1(text),
            "memory.peak" => bytes(text),
            _ => oom(text),
        };
        assert_eq!(got, *want, "{file}: {text:?}");
    }
}
