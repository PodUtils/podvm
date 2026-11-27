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

/// TODO/cli.md T-2070 (OR-B38-07): a copy of a tree onto an earlier copy
/// replaces a link of the destination by the link or the file of the source
/// and writes through none. One table: the kind of the source entry, and
/// the link that waits at its place (to a file outside the tree).
#[test]
fn a_copy_of_a_tree_replaces_a_link_of_the_destination() {
    let base = std::env::temp_dir().join(format!("podvm-cp-repl-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (src, root) = (base.join("src"), base.join("rootfs"));
    let dst = root.join("tmp/tree");
    let outside = base.join("outside");
    std::fs::create_dir_all(src.join("sub")).unwrap();
    std::fs::create_dir_all(dst.join("sub")).unwrap();
    std::fs::write(&outside, b"keep").unwrap();
    std::fs::write(src.join("file"), b"new").unwrap();
    std::os::unix::fs::symlink("file", src.join("lnk")).unwrap();
    std::os::unix::fs::symlink("../file", src.join("sub/lnk2")).unwrap();
    std::fs::create_dir_all(src.join("dir")).unwrap();
    // The first copy: each name of the source waits as a link to `outside`
    // (the second copy of a tree with links, and a link that the payload
    // made).
    for n in ["file", "lnk", "sub/lnk2"] {
        std::os::unix::fs::symlink(&outside, dst.join(n)).unwrap();
    }
    assert_eq!(copy_tree(&src, &dst, &root, false), 0);
    assert_eq!(
        std::fs::read(&outside).unwrap(),
        b"keep",
        "no write through"
    );
    assert!(!is_link(&dst.join("file")));
    assert_eq!(std::fs::read(dst.join("file")).unwrap(), b"new");
    for (n, to) in [("lnk", "file"), ("sub/lnk2", "../file")] {
        assert_eq!(
            std::fs::read_link(dst.join(n)).unwrap(),
            std::path::Path::new(to)
        );
    }
    // A second copy onto the first is the same tree again.
    assert_eq!(copy_tree(&src, &dst, &root, false), 0);
    // A link of the destination for a directory of the source, and a link
    // on the way, still refuse, and name the path of the container.
    std::fs::remove_dir(dst.join("dir")).unwrap();
    std::os::unix::fs::symlink(&outside, dst.join("dir")).unwrap();
    assert_ne!(copy_tree(&src, &dst, &root, false), 0);
    assert_eq!(std::fs::read(&outside).unwrap(), b"keep");
    let _ = std::fs::remove_dir_all(&base);
}

/// TODO/cli.md T-2070 (OR-B38-14): a line of `cp` names a path of the
/// container, never a path of the store.
#[test]
fn a_line_of_cp_names_the_path_of_the_container() {
    use std::path::Path;
    let root = Path::new("/store/trees/abc/rootfs");
    for (p, want) in [
        ("/store/trees/abc/rootfs", "/"),
        ("/store/trees/abc/rootfs/tmp/x", "/tmp/x"),
        ("/home/me/file", "/home/me/file"),
        ("/store/trees/abc/rootfs2/x", "/store/trees/abc/rootfs2/x"),
    ] {
        assert_eq!(
            super::super::cp_root::shown(root, Path::new(p)),
            want,
            "{p}"
        );
    }
}
