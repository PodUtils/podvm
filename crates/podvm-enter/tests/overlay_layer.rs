//! TODO/enter.md T-2101: the layer of `-v HOST:GUEST:overlay`. A process of
//! one thread (this test has its own `main`) enters a user and a mount
//! namespace, as the launcher of `run -d` does, mounts the layer on a tree,
//! and writes, changes, renames and removes files of the lower layer
//! through it. The lower layer stays the same byte for byte and mode for
//! mode; a whiteout of the upper layer hides the removed file.
//!
//! A host that refuses the user namespace (an AppArmor rule of Ubuntu 24.04
//! for a user with no capability) skips with a line: a skip proves nothing,
//! and experiment 485 is the proof on a host that grants it.

use std::collections::BTreeMap;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use podvm_enter::overlay;
use podvm_enter::userland::Volume;

/// Each entry under `d`: its kind, mode and bytes.
fn snapshot(d: &Path) -> BTreeMap<PathBuf, (u32, Vec<u8>)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![d.to_path_buf()];
    while let Some(p) = stack.pop() {
        for e in std::fs::read_dir(&p).unwrap() {
            let e = e.unwrap();
            let m = std::fs::symlink_metadata(e.path()).unwrap();
            let bytes = if m.is_file() {
                std::fs::read(e.path()).unwrap()
            } else {
                Vec::new()
            };
            if m.is_dir() {
                stack.push(e.path());
            }
            out.insert(
                e.path().strip_prefix(d).unwrap().to_path_buf(),
                (m.mode(), bytes),
            );
        }
    }
    out
}

fn main() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let d = std::env::temp_dir().join(format!("podvm-ovl-layer-{}-{nanos}", std::process::id()));
    std::fs::create_dir(&d).expect("a new test directory");
    let (lower, rootfs) = (d.join("lower"), d.join("tree/rootfs"));
    std::fs::create_dir_all(lower.join("sub")).unwrap();
    std::fs::create_dir_all(&rootfs).unwrap();
    std::fs::write(lower.join("a"), b"alpha\n").unwrap();
    std::fs::write(lower.join("b"), b"beta\n").unwrap();
    std::fs::write(lower.join("sub/c"), b"gamma\n").unwrap();
    std::fs::set_permissions(lower.join("b"), std::fs::Permissions::from_mode(0o644)).unwrap();
    let before = snapshot(&lower);

    if let Err(e) = overlay::enter_own_namespace() {
        eprintln!("overlay_layer: skipped, this host gives no namespace of its own: {e}");
        let _ = std::fs::remove_dir_all(&d);
        return;
    }
    let host = std::fs::canonicalize(&lower)
        .unwrap()
        .to_string_lossy()
        .to_string();
    let v = Volume {
        guest: "/c".to_string(),
        host,
    };
    overlay::mount_all(&rootfs, std::slice::from_ref(&v), None).expect("the layer mounts");

    let c = rootfs.join("c");
    // A read sees the lower layer.
    assert_eq!(std::fs::read(c.join("a")).unwrap(), b"alpha\n");
    // A write, a change, a chmod, a rename and a removal.
    std::fs::write(c.join("new"), b"new\n").unwrap();
    std::fs::write(c.join("a"), b"changed\n").unwrap();
    std::fs::set_permissions(c.join("b"), std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::rename(c.join("b"), c.join("b2")).unwrap();
    std::fs::remove_file(c.join("sub/c")).unwrap();
    // The view: the upper layer over the lower one.
    assert_eq!(std::fs::read(c.join("a")).unwrap(), b"changed\n");
    assert_eq!(std::fs::read(c.join("new")).unwrap(), b"new\n");
    assert_eq!(std::fs::read(c.join("b2")).unwrap(), b"beta\n");
    assert!(!c.join("b").exists() && !c.join("sub/c").exists());
    assert_eq!(
        std::fs::metadata(c.join("b2")).unwrap().mode() & 0o777,
        0o600
    );
    // The lower layer: the same, byte for byte and mode for mode.
    assert_eq!(snapshot(&lower), before, "the lower layer changed");
    // The upper layer holds the changes, and a whiteout for the removed file.
    let upper = overlay::layer_dir(&rootfs, 0).unwrap().join("upper");
    assert_eq!(std::fs::read(upper.join("a")).unwrap(), b"changed\n");
    let wh = std::fs::symlink_metadata(upper.join("sub/c")).unwrap();
    assert!(
        wh.file_type().is_char_device() && wh.rdev() == 0,
        "no whiteout of sub/c"
    );
    // A guest path through a link of the image is refused.
    std::os::unix::fs::symlink("/", rootfs.join("l")).unwrap();
    let bad = Volume {
        guest: "/l/x".to_string(),
        host: v.host.clone(),
    };
    let e = overlay::mount_all(&rootfs, &[bad], None).unwrap_err();
    assert!(e.contains("symbolic link"), "{e}");

    // The layer goes before the removal, as the removal of a tree does.
    let target = std::ffi::CString::new(c.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: a NUL-ended path; MNT_DETACH is 2.
    assert_eq!(unsafe { umount2(target.as_ptr(), 2) }, 0);
    let work = overlay::layer_dir(&rootfs, 0).unwrap().join("work/work");
    if work.exists() {
        std::fs::set_permissions(&work, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    std::fs::remove_dir_all(&d).unwrap();
    println!("overlay_layer: PASS: write, change, chmod, rename and removal through the layer; lower unchanged");
}

unsafe extern "C" {
    fn umount2(target: *const std::ffi::c_char, flags: i32) -> i32;
}
