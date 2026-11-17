//! The forms that `expand` keeps (PodUtils/podvm#479, #488, #492), out of
//! `tests.rs` for the limit of 500 lines of a source file.

use super::*;

/// PodUtils/podvm#492, #479: `--tls-verify=false` reaches the parser
/// whole, and `--strict=VALUE` only where the parser reads a value.
#[test]
fn tls_verify_and_a_strict_value_keep_their_form() {
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        expand("pull", &v(&["--tls-verify=false", "img"])),
        Ok(v(&["--tls-verify=false", "img"]))
    );
    assert_eq!(
        expand("run", &v(&["--strict=all", "img"])),
        Ok(v(&["--strict=all", "img"]))
    );
    assert_eq!(
        expand("doctor", &v(&["--strict=0"])),
        Err("--strict=0".to_string())
    );
    assert_eq!(expand("doctor", &v(&["--strict=false"])), Ok(v(&[])));
}

/// PodUtils/podvm#488: `--` ends the options of each verb; `run` keeps it
/// for its parser, and a `--` of the payload after the image stays.
#[test]
fn a_double_dash_ends_the_options() {
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(expand("pull", &v(&["--", "img"])), Ok(v(&["img"])));
    assert_eq!(
        expand("images", &v(&["-q", "--", "alpine"])),
        Ok(v(&["-q", "alpine"]))
    );
    assert_eq!(
        expand("exec", &v(&["--", "c1", "sh", "--", "x"])),
        Ok(v(&["c1", "sh", "--", "x"]))
    );
    assert_eq!(
        expand("exec", &v(&["c1", "--", "x"])),
        Ok(v(&["c1", "--", "x"]))
    );
    assert_eq!(expand("run", &v(&["--", "img"])), Ok(v(&["--", "img"])));
}
