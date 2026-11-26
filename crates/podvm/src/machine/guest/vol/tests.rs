//! TODO/machine.md T-2103: the mapped volumes of the machine tier.

/// TODO/machine.md T-2103: a `:mapped` volume, writable or read-only, is
/// a share with `security_model=mapped-xattr`, mounted with
/// `access=client`; its argument goes to the guest and back whole. One
/// table: the guest paths, the modes, and what the emulator and the
/// subcommand get.
#[test]
fn a_mapped_volume_keeps_the_owners_in_attributes() {
    use super::{arg, parse, virtfs, vols, Vol};
    let g = |p: &str, ro| (p.to_string(), ro);
    let all = vols(
        &[g("/a", false), g("/b", true), g("/c", false), g("/d", true)],
        &["/a".to_string(), "/b".to_string()],
    );
    let cases = [
        (
            ",security_model=mapped-xattr,fmode=0644,dmode=0755",
            "/a:mapped",
        ),
        (
            ",security_model=mapped-xattr,fmode=0644,dmode=0755,readonly=on",
            "/b:mapped:ro",
        ),
        (",security_model=none", "/c"),
        (",security_model=none,readonly=on", "/d:ro"),
    ];
    for (v, (model, word)) in all.iter().zip(cases) {
        let h = std::path::Path::new("/h");
        assert_eq!(
            virtfs(h, v),
            format!("local,path=/h,mount_tag={}{model}", v.tag)
        );
        assert_eq!(arg(v), format!("{}={word}", v.tag));
        assert_eq!(parse(&arg(v)).as_ref(), Some(v), "{word}");
    }
    let s = super::super::init::script("n", &["true".into()], &[], "/", true, &all, None, None);
    assert!(
        s.contains("'podvm0=/a:mapped' 'podvm1=/b:mapped:ro'"),
        "{s}"
    );
    // A guest path with `:` is no argument of the subcommand.
    assert_eq!(parse("podvm0=/x:y"), None);
    assert!(super::MAPPED_OPTIONS.ends_with(",access=client"));
    let none = Vol {
        mapped: false,
        ..all[0].clone()
    };
    assert!(!arg(&none).contains("mapped"));
}

/// TODO/machine.md T-2103: the probe of the attributes of HOST. A
/// writable volume sets and removes `user.podvm.probe`, and leaves no
/// attribute; a path that is not there is refused with its cause.
#[test]
fn the_probe_of_a_mapped_volume_leaves_no_attribute() {
    let d = std::env::temp_dir().join(format!(
        "podvm-mapped-{}",
        crate::machine::guest::init::nonce().unwrap()
    ));
    std::fs::create_dir(&d).unwrap();
    // A file system with no user attributes (an old tmpfs) is refused,
    // and a test on it proves nothing of the write: it ends here.
    if super::probe_xattrs(&d, false).is_ok() {
        assert!(super::probe_xattrs(&d, true).is_ok());
        let c = std::ffi::CString::new(d.to_str().unwrap()).unwrap();
        unsafe extern "C" {
            fn llistxattr(path: *const std::ffi::c_char, list: *mut u8, size: usize) -> isize;
        }
        // SAFETY: NUL-ended path; a size of 0 asks for the length.
        assert_eq!(
            unsafe { llistxattr(c.as_ptr(), std::ptr::null_mut(), 0) },
            0
        );
    }
    let gone = d.join("gone");
    assert!(super::probe_xattrs(&gone, false).is_err());
    let _ = std::fs::remove_dir_all(&d);
}
