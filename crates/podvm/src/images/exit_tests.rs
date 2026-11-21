//! TODO/image.md T-1842: the input errors of the image verbs exit 1, as
//! docker 29.9.2 does (measured 2026-11-14), and a bad host of
//! `--insecure-registry` is refused with the value named (podcrates
//! `v0.45.0`, `Refusal::Host`, podcrates@T-585).

use podvm_image::error::{Error, EXIT_CLI_ERROR};

#[test]
fn image_exit_codes_of_input_errors_are_1() {
    assert_eq!(
        Error::NoSuchImage("nosuch".into()).exit_code(),
        EXIT_CLI_ERROR
    );
    assert_eq!(
        Error::Reference("Bad:Name".into()).exit_code(),
        EXIT_CLI_ERROR
    );
    let e = podvm_image::transport::resolve(&["bad host!".to_string()], None).unwrap_err();
    assert_eq!(e.exit_code(), EXIT_CLI_ERROR, "{e}");
    assert!(e.to_string().contains("bad host!"), "{e}");
    assert!(podvm_image::transport::resolve(&["registry.test:5000".to_string()], None).is_ok());
}

/// TODO/cli.md T-2062 (OQ-10): one rule for `load` and `import`. A failure
/// to read the archive that the caller named is a refusal after the parse
/// (1, `podvm-probe::exit`); a failure of the store is podvm's (125).
#[test]
fn a_refused_archive_exits_with_the_code_of_the_table() {
    use podvm_image::error::EXIT_RUNTIME_ERROR;
    let archive = std::path::Path::new("/in/a.tar");
    let io = |path: &str| Error::io(path, std::io::Error::from_raw_os_error(13));
    let rows: [(Error, i32); 6] = [
        (
            Error::Oci("a.tar is compressed (gzip)".into()),
            EXIT_CLI_ERROR,
        ),
        (
            Error::DigestMismatch {
                what: "layer".into(),
                want: "sha256:a".into(),
                got: "sha256:b".into(),
            },
            EXIT_CLI_ERROR,
        ),
        (io("/in/a.tar"), EXIT_CLI_ERROR),
        (io("/store/staging/x"), EXIT_RUNTIME_ERROR),
        (Error::Store("the index".into()), EXIT_RUNTIME_ERROR),
        (Error::NoSuchImage("x".into()), EXIT_CLI_ERROR),
    ];
    for (e, want) in rows {
        assert_eq!(super::archive_exit::archive_exit(&e, archive), want, "{e}");
    }
}
