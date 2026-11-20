//! The tests of T-1768 (hard links of `cp`) and T-1772 (`-w`).

use super::*;

/// TODO/image.md T-1768. A file of several names is copied once; each
/// other name is a hard link to the copy.
#[test]
fn copy_tree_keeps_hard_links() {
    use std::os::unix::fs::MetadataExt;
    let base = std::env::temp_dir().join(format!("podvm-cp-hl-{}", std::process::id()));
    let (src, dst) = (base.join("src"), base.join("dst"));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(src.join("bin")).unwrap();
    std::fs::write(src.join("bin/busybox"), b"elf").unwrap();
    for n in ["ls", "sh"] {
        std::fs::hard_link(src.join("bin/busybox"), src.join("bin").join(n)).unwrap();
    }
    assert_eq!(copy_tree(&src, &dst, &src, true), 0);
    let ino = |n: &str| std::fs::metadata(dst.join("bin").join(n)).unwrap().ino();
    assert_eq!(ino("ls"), ino("busybox"));
    assert_eq!(ino("sh"), ino("busybox"));
    assert_eq!(std::fs::metadata(dst.join("bin/sh")).unwrap().nlink(), 3);
    assert_eq!(std::fs::read(dst.join("bin/ls")).unwrap(), b"elf");
    let _ = std::fs::remove_dir_all(&base);
}

/// TODO/cli.md T-1772. A missing working directory is made in the rootfs,
/// as docker does; a link of the image on the way is never followed, and
/// a path in a volume is left to the volume.
#[test]
fn a_missing_workdir_is_made_and_never_through_a_link() {
    let base = std::env::temp_dir().join(format!("podvm-wd-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let root = base.join("rootfs");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(base.join("outside")).unwrap();
    std::os::unix::fs::symlink(base.join("outside"), root.join("out")).unwrap();
    let r = root.to_str().unwrap();
    crate::lifecycle::volume::make_workdir(r, "/new/dir", &[]);
    assert!(root.join("new/dir").is_dir());
    crate::lifecycle::volume::make_workdir(r, "/out/x", &[]);
    assert!(
        !base.join("outside/x").exists(),
        "a link of the image is not followed"
    );
    crate::lifecycle::volume::make_workdir(r, "/a/../b", &[]);
    assert!(!root.join("a").exists());
    let v = podvm_enter::userland::Volume {
        guest: "/data".into(),
        host: base.join("outside").to_string_lossy().into_owned(),
    };
    crate::lifecycle::volume::make_workdir(r, "/data/sub", &[v]);
    assert!(!root.join("data").exists(), "a volume serves its own paths");
    let _ = std::fs::remove_dir_all(&base);
}

/// TODO/cli.md T-2001: the mount point of each volume is made in the tree
/// of the run, never through a link of the image, and a part that is
/// there stays as it is.
#[test]
fn the_mount_point_of_a_volume_is_made_in_the_tree() {
    let base = std::env::temp_dir().join(format!("podvm-mp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let root = base.join("rootfs");
    std::fs::create_dir_all(root.join("etc")).unwrap();
    std::fs::create_dir_all(base.join("outside")).unwrap();
    std::fs::write(base.join("host.conf"), "x").unwrap();
    std::fs::write(root.join("etc/kept"), "image").unwrap();
    std::os::unix::fs::symlink(base.join("outside"), root.join("out")).unwrap();
    let dir = base.join("outside").to_string_lossy().into_owned();
    let file = base.join("host.conf").to_string_lossy().into_owned();
    let vol = |g: &str, h: &str| podvm_enter::userland::Volume {
        guest: g.into(),
        host: h.into(),
    };
    let vols = [
        vol("/data", &dir),
        vol("/deep/a/b", &dir),
        vol("/etc/app.conf", &file),
        vol("/etc/kept", &file),
        vol("/out/x", &dir),
        vol("/a/../b", &dir),
        vol("/dev/fuse", &dir),
        vol("/proc/x", &dir),
    ];
    crate::lifecycle::volume::make_mount_points(root.to_str().unwrap(), &vols);
    // (guest path, a directory, a file) after the call.
    for (path, dir, file) in [
        ("data", true, false),
        ("deep/a/b", true, false),
        ("etc/app.conf", false, true),
        ("etc/kept", false, true),
        ("a", false, false),
        ("b", false, false),
        ("dev", false, false),
        ("proc", false, false),
    ] {
        let m = std::fs::symlink_metadata(root.join(path));
        assert_eq!(m.as_ref().is_ok_and(|m| m.is_dir()), dir, "{path}");
        assert_eq!(m.as_ref().is_ok_and(|m| m.is_file()), file, "{path}");
    }
    assert_eq!(
        std::fs::read_to_string(root.join("etc/kept")).unwrap(),
        "image"
    );
    assert_eq!(
        std::fs::metadata(root.join("etc/app.conf")).unwrap().len(),
        0
    );
    assert!(
        !base.join("outside/x").exists(),
        "a link of the image is not followed"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// TODO/image.md T-1774. A shim file in place of a device of the rootfs,
/// or no node at all, maps to the host's device; a volume of the caller at
/// the path keeps the path.
#[test]
fn a_shimmed_device_maps_to_the_hosts() {
    use std::os::unix::fs::FileTypeExt;
    if !std::fs::metadata("/dev/urandom").is_ok_and(|m| m.file_type().is_char_device()) {
        return;
    }
    let base = std::env::temp_dir().join(format!("podvm-dev-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("dev")).unwrap();
    std::fs::write(base.join("dev/urandom"), vec![0u8; 16]).unwrap();
    let r = base.to_str().unwrap();
    let pairs = crate::lifecycle::volume::device_pairs(r, &[]);
    assert!(pairs
        .iter()
        .any(|v| v.guest == "/dev/urandom" && v.host == "/dev/urandom"));
    let mine = podvm_enter::userland::Volume {
        guest: "/dev/urandom".into(),
        host: "/tmp".into(),
    };
    let pairs = crate::lifecycle::volume::device_pairs(r, &[mine]);
    assert!(
        !pairs.iter().any(|v| v.guest == "/dev/urandom"),
        "the caller's volume wins"
    );
    let _ = std::fs::remove_dir_all(&base);
}
