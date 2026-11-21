//! TODO/cli.md T-2064 (OQ-13): a refused value says what is wrong with it.
//! One table for the family: each row is a value and the words that its
//! refusal must hold.

use super::parse::{access::dir_refusal, bound, values::utf8_line};

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
