//! TODO/machine.md T-2103: the mapped volumes of the machine tier.

/// TODO/machine.md T-2103: a `:mapped` volume, writable or read-only, is
/// an `-fsdev` with `security_model=mapped-xattr` (and a `,` of HOST
/// doubled, as QEMU reads it), mounted with
/// `access=client`; its argument goes to the guest and back whole. One
/// table: the guest paths, the modes, and what the emulator and the
/// subcommand get.
#[test]
fn a_mapped_volume_keeps_the_owners_in_attributes() {
    use super::{arg, argv, parse, vols, Vol};
    let g = |p: &str, ro| (p.to_string(), ro);
    let all = vols(
        &[g("/a", false), g("/b", true), g("/c", false), g("/d", true)],
        &["/a".to_string(), "/b".to_string()],
    );
    let model = "security_model=mapped-xattr,fmode=0644,dmode=0755";
    let fsdev = |t: &str, ro: &str| {
        vec![
            "-fsdev".to_string(),
            format!("local,id={t},path=/h,,x,{model}{ro}"),
            "-device".to_string(),
            format!("virtio-9p-pci,fsdev={t},mount_tag={t}"),
        ]
    };
    let virtfs = |t: &str, ro: &str| {
        vec![
            "-virtfs".to_string(),
            format!("local,path=/h,,x,mount_tag={t},security_model=none{ro}"),
        ]
    };
    let cases = [
        (fsdev("podvm0", ""), "/a:mapped"),
        (fsdev("podvm1", ",readonly=on"), "/b:mapped:ro"),
        (virtfs("podvm2", ""), "/c"),
        (virtfs("podvm3", ",readonly=on"), "/d:ro"),
    ];
    for (v, (want, word)) in all.iter().zip(cases) {
        assert_eq!(argv(std::path::Path::new("/h,x"), v), want, "{word}");
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
