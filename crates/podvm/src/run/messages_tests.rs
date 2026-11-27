//! TODO/cli.md T-2064 (OQ-13): a refused value says what is wrong with it.
//! One table for the family: each row is a value and the words that its
//! refusal must hold.

use super::parse::{access::dir_refusal, bound, envfile::utf8_line};

#[test]
fn a_refused_value_says_what_is_wrong_with_it() {
    let max = bound::MAX.to_string();
    // (the refusal, the words it must hold, the words it must not)
    let rows: Vec<(String, Vec<&str>, Vec<&str>)> = vec![
        (
            bound::refusal("99999999999999999999"),
            vec!["too large", &max],
            vec!["not \""],
        ),
        (bound::refusal("0"), vec!["not \"0\""], vec!["too large"]),
        (
            bound::refusal("two"),
            vec!["not \"two\""],
            vec!["too large"],
        ),
        (bound::refusal(""), vec!["not \"\""], vec!["too large"]),
        (bound::refusal("-5"), vec!["not \"-5\""], vec!["too large"]),
        (
            dir_refusal(std::path::Path::new("/nonexistent-podvm-dir/sub")),
            vec!["does not exist"],
            vec!["takes no file"],
        ),
        (
            dir_refusal(std::path::Path::new("/")),
            vec!["takes no file of the caller"],
            vec!["does not exist"],
        ),
    ];
    for (text, has, hasnt) in rows {
        for w in has {
            assert!(text.contains(w), "{text:?} lacks {w:?}");
        }
        for w in hasnt {
            assert!(!text.contains(w), "{text:?} holds {w:?}");
        }
    }
    // The line and the byte of a file that is not UTF-8.
    assert_eq!(utf8_line(b"A=1\nB=\xff\n"), (2, 6));
    assert_eq!(utf8_line(b"\xff"), (1, 0));
    assert_eq!(utf8_line(b"A=\xc3\xa9\nB=\xc3("), (2, 7));
}

/// TODO/cli.md T-2070 (OR-B38-11, OR-B40-12): the lines of an env file, as
/// docker reads them. One table: the text, the host, and the entries or the
/// refusal.
#[test]
fn an_env_file_is_read_as_docker_reads_it() {
    use super::parse::envfile::env_lines;
    let host = |n: &str| (n == "D").then(|| format!("{n}=fromhost"));
    let ok = |t: &str| env_lines(t, &host);
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    // A name with no `=` takes the host value, or gives nothing.
    assert_eq!(ok("D\n"), Ok(v(&["D=fromhost"])));
    assert_eq!(ok("NOHOST\nA=1\n"), Ok(v(&["A=1"])));
    assert_eq!(ok("  D  \r\nA=\r\n"), Ok(v(&["D=fromhost", "A="])));
    // A UTF-8 byte order mark on line 1 is not part of the first name.
    assert_eq!(ok("\u{feff}D\n"), Ok(v(&["D=fromhost"])));
    assert_eq!(ok("\u{feff}A=1\nB=2"), Ok(v(&["A=1", "B=2"])));
    // The mark counts on line 1 only; a comment and a blank line give nothing.
    assert_eq!(ok("\u{feff}# c\n\nA=1"), Ok(v(&["A=1"])));
    // Refused by line: no name, and white space inside a bare name.
    assert_eq!(ok("A=1\n=x\n").unwrap_err().0, 2);
    let e = ok("A=1\nB C\n").unwrap_err();
    assert_eq!(e.0, 2);
    assert!(e.1.contains("white space") && e.1.contains("B C"), "{e:?}");
}
