//! PodUtils/podvm#531 (TODO/cli.md T-2088): the commit of a release build of
//! podvm-dev.

/// PodUtils/podvm#531: one rule for each answer of `build-state.py
/// commit`: a commit only with exit 0 and a commit word on the first
/// line; else none, and the build reads git.
#[test]
fn only_a_commit_word_with_exit_0_is_the_commit() {
    let c = "0123456789abcdef0123456789abcdef01234567";
    let dirty = format!("{c}-dirty");
    #[rustfmt::skip]
    let cases: &[(i32, &str, Option<&str>)] = &[
        (0, c, Some(c)),
        (0, &format!("{c}\n"), Some(c)),
        (0, &format!("  {dirty}  \nmore"), Some(&dirty)),
        (0, "unknown", Some("unknown")),
        (0, "0123abc", Some("0123abc")),
        (2, "build-state: podvm-buildstate is not built; cargo build -p podvm-buildstate", None),
        (0, "build-state: podvm-buildstate is not built; cargo build -p podvm-buildstate", None),
        (1, c, None),
        (0, "", None),
        (0, "012345", None),
        (0, "0123456789ABCDEF0123", None),
        (0, &"a".repeat(65), None),
        (0, "-dirty", None),
        (0, "\u{fffd}", None),
    ];
    for (rc, text, want) in cases {
        assert_eq!(
            super::commit_of(*rc, text).as_deref(),
            *want,
            "{rc} {text:?}"
        );
    }
}
