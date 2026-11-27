//! TODO/enter.md T-2107: the devices of the launcher's namespace. A process
//! of one thread (this test has its own `main`) takes the entry of the
//! launcher of `run -d` with an `:overlay` volume (`overlay::prepare`) over a
//! tree whose `/dev` holds regular files, as `pod-rootfs` leaves them where
//! `mknod` is refused. Where the entry is in a user namespace, each of
//! `pod_enter::DEVICES` is then a character device that works (`/dev/zero`
//! reads zeros, `/dev/urandom` reads past 1 MiB, `/dev/null` takes a write),
//! and the layer is still mounted. As root of the initial user namespace the
//! entry binds nothing (`mknod` works there), and the files stay.
//!
//! A host that refuses the user namespace skips with a line: a skip proves
//! nothing, and the experiments 485 and 489 are the proof on a host that
//! grants it.

use std::io::{Read, Write};
use std::os::unix::fs::FileTypeExt;

use podvm_enter::overlay;
use podvm_enter::userland::Volume;

fn main() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let d = std::env::temp_dir().join(format!("podvm-ovl-dev-{}-{nanos}", std::process::id()));
    let (lower, rootfs) = (d.join("lower"), d.join("tree/rootfs"));
    std::fs::create_dir_all(&lower).unwrap();
    std::fs::create_dir_all(rootfs.join("dev")).unwrap();
    std::fs::write(lower.join("a"), b"alpha\n").unwrap();
    for name in pod_enter::DEVICES {
        // The tty of the host may be missing: the entry then says so.
        std::fs::write(rootfs.join("dev").join(name), b"stub\n").unwrap();
    }
    let v = Volume {
        guest: "/c".to_string(),
        host: std::fs::canonicalize(&lower)
            .unwrap()
            .to_string_lossy()
            .to_string(),
    };
    let lines = match overlay::prepare(&rootfs, std::slice::from_ref(&v), None) {
        Ok(lines) => lines,
        Err(e) => {
            eprintln!("overlay_devices: skipped, this host gives no namespace of its own: {e}");
            let _ = std::fs::remove_dir_all(&d);
            return;
        }
    };
    let map = std::fs::read_to_string("/proc/self/uid_map").unwrap();
    let in_userns = !pod_hostprobe::userns::is_initial_map(&map);
    let kind = |n: &str| {
        std::fs::symlink_metadata(rootfs.join("dev").join(n))
            .unwrap()
            .file_type()
    };

    // The layer is there in each case.
    assert_eq!(std::fs::read(rootfs.join("c/a")).unwrap(), b"alpha\n");
    if in_userns {
        for name in ["null", "zero", "full", "random", "urandom"] {
            assert!(
                kind(name).is_char_device(),
                "/dev/{name} is no character device: {lines:?}"
            );
        }
        let mut zeros = [1u8; 4];
        std::fs::File::open(rootfs.join("dev/zero"))
            .unwrap()
            .read_exact(&mut zeros)
            .unwrap();
        assert_eq!(zeros, [0; 4]);
        // More than the 1 MiB of a regular file that would stand in.
        let mut rnd = std::fs::File::open(rootfs.join("dev/urandom")).unwrap();
        let n = std::io::copy(
            &mut Read::by_ref(&mut rnd).take(2 << 20),
            &mut std::io::sink(),
        )
        .unwrap();
        assert_eq!(n, 2 << 20, "/dev/urandom ended early");
        std::fs::OpenOptions::new()
            .write(true)
            .open(rootfs.join("dev/null"))
            .unwrap()
            .write_all(b"x")
            .unwrap();
        // `/dev/tty` binds where the host has one, and says so where it has none.
        let host_tty = std::fs::metadata("/dev/tty").is_ok_and(|m| m.file_type().is_char_device());
        assert_eq!(kind("tty").is_char_device(), host_tty, "{lines:?}");
        if !host_tty {
            assert!(lines.iter().any(|l| l.contains("/dev/tty")), "{lines:?}");
        }
        // The ptmx of the new devpts instance, where the namespace has `/dev/pts`.
        if std::path::Path::new("/dev/pts").is_dir() {
            assert!(kind("ptmx").is_char_device(), "/dev/ptmx: {lines:?}");
            let ptmx = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(rootfs.join("dev/ptmx"));
            assert!(
                ptmx.is_ok(),
                "/dev/ptmx does not open: {:?} {lines:?}",
                ptmx.err()
            );
        }
    } else {
        for name in pod_enter::DEVICES {
            assert!(
                kind(name).is_file(),
                "/dev/{name} changed for root: {lines:?}"
            );
        }
        assert!(lines.is_empty(), "{lines:?}");
    }

    // Take the mounts off before the removal, as the removal of a tree does.
    for rel in [
        "c",
        "dev/null",
        "dev/zero",
        "dev/full",
        "dev/random",
        "dev/urandom",
        "dev/tty",
        "dev/ptmx",
    ] {
        let t = std::ffi::CString::new(rootfs.join(rel).as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: a NUL-ended path; MNT_DETACH is 2. A path with no mount fails, and that is fine.
        unsafe { umount2(t.as_ptr(), 2) };
    }
    let work = overlay::layer_dir(&rootfs, 0).unwrap().join("work/work");
    if work.exists() {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&work, std::fs::Permissions::from_mode(0o700));
    }
    std::fs::remove_dir_all(&d).unwrap();
    println!(
        "overlay_devices: PASS: {} (user namespace: {in_userns})",
        if in_userns {
            "the devices of the host and the layer"
        } else {
            "no bind as root, the layer"
        }
    );
}

unsafe extern "C" {
    fn umount2(target: *const std::ffi::c_char, flags: i32) -> i32;
}
