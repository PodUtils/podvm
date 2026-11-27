//! TODO/machine.md T-1994, part 2: the kernel of the image.

use super::*;
use crate::machine::guest::{arch::Arch, Request};

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "podvm-t1994-kernel-{name}-{}",
        crate::machine::guest::init::nonce().unwrap()
    ));
    std::fs::create_dir(&d).unwrap();
    d
}

fn request() -> Request<'static> {
    Request {
        verb: "run",
        keeper: None,
        pids: None,
        disk: None,
        arch: Arch::Amd64,
        loud: false,
        store_root: Path::new("/store"),
        rootfs: Path::new("/rootfs"),
        digest: "sha256:aa",
        argv: vec!["true".into()],
        env: Vec::new(),
        workdir: "/".into(),
        mem: None,
        cpus: None,
        broker: None,
        timeout: None,
        qemu_args: Vec::new(),
        copies: Vec::new(),
        volumes: Vec::new(),
        mapped: Vec::new(),
        stdin: None,
        tty: false,
        net_none: true,
        channel: None,
        sealed: None,
        engine: false,
    }
}

/// One table for the value of the variable, the pick of a release among
/// the kernels of an image, and each run that the route does not serve.
#[test]
fn the_choice_of_a_kernel_of_the_image_holds_or_refuses_by_name() {
    type W = Result<Option<Want>, &'static str>;
    let r = |s: &str| Some(Want::Release(s.to_string()));
    let values: [(Option<&str>, bool, W); 9] = [
        (None, false, Ok(None)),
        (Some(""), true, Ok(None)),
        (Some("0"), true, Ok(None)),
        (Some("1"), false, Ok(Some(Want::One))),
        (Some("6.12.1-amd64"), false, Ok(r("6.12.1-amd64"))),
        (Some("1"), true, Err("set one")),
        (Some("../x"), false, Err("the values are 0, 1")),
        (Some("a b"), false, Err("the values are 0, 1")),
        (Some(".."), false, Err("the values are 0, 1")),
    ];
    for (var, other, want) in values {
        match (want, wanted(var, other)) {
            (Ok(w), Ok(g)) => assert_eq!(w, g, "{var:?}"),
            (Err(w), Err(g)) => assert!(g.contains(w) && g.contains(VAR), "{var:?}: {g}"),
            (w, g) => panic!("{var:?} {other}: want {w:?}, got {g:?}"),
        }
    }
    let two = ["5.1".to_string(), "6.2".to_string()];
    let picks: [(Want, &[String], Result<&str, &str>); 5] = [
        (Want::One, &two[..1], Ok("5.1")),
        (Want::One, &[], Err("no kernel: no /boot/vmlinuz-RELEASE")),
        (Want::One, &two, Err("2 kernels (5.1, 6.2)")),
        (Want::Release("6.2".into()), &two, Ok("6.2")),
        (
            Want::Release("7.0".into()),
            &two,
            Err("no kernel of the release 7.0 (its kernels: 5.1, 6.2)"),
        ),
    ];
    for (w, have, want) in picks {
        match (want, pick(&w, have)) {
            (Ok(a), Ok(b)) => assert_eq!(a, b),
            (Err(a), Err(b)) => assert!(b.contains(a), "{w:?}: {b}"),
            (a, b) => panic!("{w:?}: want {a:?}, got {b:?}"),
        }
    }
    let mut runs: Vec<(Request, Option<&str>, Option<&str>)> = Vec::new();
    runs.push((request(), None, None));
    runs.push((request(), Some("disk"), None));
    runs.push((
        request(),
        Some("initramfs"),
        Some("PODVM_MACHINE_ROOT=initramfs"),
    ));
    let mut e = request();
    e.engine = true;
    runs.push((e, None, Some("--podvm-engine")));
    let mut a = request();
    a.arch = Arch::Arm64;
    runs.push((a, None, Some("linux/arm64")));
    for (run, root, want) in runs {
        let got = refusal(&run, root);
        match (want, got) {
            (None, None) => {}
            (Some(w), Some(g)) => assert!(g.contains(w) && g.starts_with(VAR), "{g}"),
            (w, g) => panic!("{root:?}: want {w:?}, got {g:?}"),
        }
    }
    // The drivers of each part of a run.
    let what = |r: &Request| groups(r).iter().map(|g| g.what).collect::<Vec<_>>();
    let mut r = request();
    assert_eq!(what(&r), [ROOT.what]);
    r.net_none = false;
    r.volumes = vec![(PathBuf::from("/h"), "/g".into(), false)];
    r.tty = true;
    assert_eq!(what(&r), [ROOT.what, NET.what, NINE.what, CONSOLE.what]);
}

/// The `modules.dep` of a kernel of Debian 13 (compressed modules, the
/// paths of depmod), cut to the drivers of a guest.
const DEP: &str = "kernel/fs/ext4/ext4.ko.xz: kernel/lib/crc16.ko.xz kernel/fs/mbcache.ko.xz kernel/fs/jbd2/jbd2.ko.xz\n\
kernel/fs/mbcache.ko.xz:\n\
kernel/fs/jbd2/jbd2.ko.xz:\n\
kernel/lib/crc16.ko.xz:\n\
kernel/crypto/crc32c_generic.ko.xz:\n\
kernel/drivers/block/virtio_blk.ko.xz:\n\
kernel/fs/overlayfs/overlay.ko.xz:\n\
kernel/fs/9p/9p.ko.xz: kernel/fs/netfs/netfs.ko.xz kernel/net/9p/9pnet.ko.xz\n\
kernel/net/9p/9pnet_virtio.ko.xz: kernel/net/9p/9pnet.ko.xz\n\
kernel/net/9p/9pnet.ko.xz:\n\
kernel/fs/netfs/netfs.ko.xz:\n\
kernel/drivers/net/virtio_net.ko.xz: kernel/drivers/net/net_failover.ko.xz kernel/net/core/failover.ko.xz\n\
kernel/drivers/net/net_failover.ko.xz: kernel/net/core/failover.ko.xz\n\
kernel/net/core/failover.ko.xz:\n";
const BUILTIN: &str = "kernel/drivers/virtio/virtio_pci.ko\n\
kernel/drivers/tty/serial/8250/8250.ko\nkernel/drivers/char/virtio_console.ko\n";

/// One table of kernels and the drivers of a run: each driver is a module
/// with its closure first, built in (by `modules.builtin` or by `=y` of
/// the config), or missing and named with what needs it and its config.
#[test]
fn each_driver_is_a_module_built_in_or_named_missing() {
    let cloud = "CONFIG_SERIAL_8250=y\nCONFIG_SERIAL_8250_CONSOLE=y\nCONFIG_DEVTMPFS=y\n\
                 CONFIG_BLK_DEV_INITRD=y\nCONFIG_EXT4_FS=y\n# CONFIG_NET_9P is not set\n";
    let all_in = "CONFIG_SERIAL_8250=y\nCONFIG_SERIAL_8250_CONSOLE=y\nCONFIG_DEVTMPFS=y\n\
                  CONFIG_BLK_DEV_INITRD=y\nCONFIG_VIRTIO_PCI=y\nCONFIG_VIRTIO_BLK=y\n\
                  CONFIG_EXT4_FS=y\nCONFIG_OVERLAY_FS=y\nCONFIG_9P_FS=y\nCONFIG_NET_9P_VIRTIO=y\n";
    type Case<'a> = (
        &'a str,
        &'a str,
        &'a str,
        Option<&'a str>,
        Vec<&'a Group>,
        Result<&'a [&'a str], &'a str>,
    );
    let cases: Vec<Case> = vec![
        (
            "debian, root and volume",
            DEP,
            BUILTIN,
            None,
            vec![&ROOT, &NINE],
            Ok(&["crc32c_generic", "virtio_blk", "crc16", "mbcache", "jbd2", "ext4", "overlay",
                 "9pnet", "9pnet_virtio", "netfs", "9p"]),
        ),
        (
            "debian, the network",
            DEP,
            BUILTIN,
            None,
            vec![&NET, &CONSOLE],
            Ok(&["failover", "net_failover", "virtio_net"]),
        ),
        ("each driver built in by its config", "", "", Some(all_in), vec![&ROOT, &NINE, &CONSOLE], Err("virtio_console (CONFIG_VIRTIO_CONSOLE is not set)")),
        ("each driver built in", "", "", Some(all_in), vec![&ROOT, &NINE], Ok(&[])),
        (
            "cloud kernel with no 9p",
            "kernel/drivers/block/virtio_blk.ko.xz:\nkernel/fs/overlayfs/overlay.ko.xz:\n",
            BUILTIN,
            Some(cloud),
            vec![&ROOT, &NINE],
            Err("has no 9pnet_virtio (CONFIG_NET_9P_VIRTIO is not set) and 9p (CONFIG_9P_FS is not set) for a -v volume"),
        ),
        ("no root disk driver", "", "", None, vec![&ROOT], Err("virtio_pci and virtio_blk and ext4 and overlay for the root disk")),
        ("a path out of the release", "kernel/../../virtio_blk.ko:\n", "", Some(all_in), vec![&ROOT], Err("not a plain module path")),
    ];
    for (name, dep, builtin, config, groups, want) in cases {
        let d = Drivers::parse("6.12.1", dep, builtin, config);
        let got = d.plan(&groups);
        match (want, got) {
            (Ok(w), Ok(g)) => {
                let names: Vec<&str> = g.iter().map(|(n, _)| n.as_str()).collect();
                assert_eq!(names, w, "{name}");
            }
            (Err(w), Err(g)) => {
                assert!(g.contains(w), "{name}: {w:?} is not in {g:?}");
                assert!(!g.contains('\n'), "{name}: one line: {g}");
            }
            (w, g) => panic!("{name}: want {w:?}, got {g:?}"),
        }
    }
    // The console of the markers and the first mounts.
    let base: [(&str, Option<&str>, Option<&str>); 5] = [
        ("", None, None),
        (BUILTIN, None, None),
        (
            "kernel/drivers/virtio/virtio_pci.ko\n",
            None,
            Some("needs 8250 built in"),
        ),
        (
            "",
            Some("CONFIG_SERIAL_8250=y\nCONFIG_DEVTMPFS=y\nCONFIG_BLK_DEV_INITRD=y\n"),
            Some("CONFIG_SERIAL_8250_CONSOLE=y"),
        ),
        ("", Some(all_in), None),
    ];
    for (builtin, config, want) in base {
        let got = Drivers::parse("r", "", builtin, config).base();
        match (want, got) {
            (None, None) => {}
            (Some(w), Some(g)) => assert!(g.contains(w), "{w:?} is not in {g:?}"),
            (w, g) => panic!("{builtin:?} {config:?}: want {w:?}, got {g:?}"),
        }
    }
    let module = Drivers::parse("r", "kernel/drivers/tty/serial/8250/8250.ko:\n", "", None);
    assert!(module.base().unwrap().contains("it is a module"));
}

/// A kernel file of x86 that QEMU takes: the magic of the setup header.
fn bzimage() -> Vec<u8> {
    let mut k = vec![0u8; 0x400];
    k[0x202..0x206].copy_from_slice(b"HdrS");
    k
}

/// The kernels of an image through each layout (Debian with `/lib` a link
/// to `usr/lib`, Arch with the kernel in the directory of its modules),
/// the modules of a run with their files in the image, and each refusal.
#[test]
fn the_kernel_of_an_image_is_found_through_its_links() {
    use std::os::unix::fs::symlink;
    let d = scratch("find");
    let deb = d.join("deb");
    let mods = deb.join("usr/lib/modules/6.12.1-amd64");
    std::fs::create_dir_all(deb.join("boot")).unwrap();
    std::fs::create_dir_all(&mods).unwrap();
    symlink("usr/lib", deb.join("lib")).unwrap();
    std::fs::write(deb.join("boot/vmlinuz-6.12.1-amd64"), bzimage()).unwrap();
    std::fs::write(
        deb.join("boot/config-6.12.1-amd64"),
        "CONFIG_SERIAL_8250=y\n\
        CONFIG_SERIAL_8250_CONSOLE=y\nCONFIG_DEVTMPFS=y\nCONFIG_BLK_DEV_INITRD=y\n",
    )
    .unwrap();
    std::fs::write(mods.join("modules.dep"), DEP).unwrap();
    std::fs::write(mods.join("modules.builtin"), BUILTIN).unwrap();
    for line in DEP.lines() {
        let p = mods.join(line.split(':').next().unwrap());
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"ko").unwrap();
    }
    assert_eq!(releases(&deb), ["6.12.1-amd64"]);
    let (k, got) = own(&deb, "6.12.1-amd64", &[&ROOT]).unwrap();
    assert_eq!(k, deb.join("boot/vmlinuz-6.12.1-amd64"));
    assert_eq!(got.load.len(), 7, "{:?}", got.load);
    assert_eq!(got.load[5].0, "ext4");
    assert_eq!(got.load[5].1, mods.join("kernel/fs/ext4/ext4.ko.xz"));
    // A second kernel: `1` refuses, and the release picks.
    let arch = deb.join("usr/lib/modules/6.19.2-arch1-1");
    std::fs::create_dir_all(&arch).unwrap();
    std::fs::write(
        arch.join("vmlinuz"),
        b"not a bzImage of x86, long enough to hold 0x206 bytes".repeat(20),
    )
    .unwrap();
    assert_eq!(releases(&deb), ["6.12.1-amd64", "6.19.2-arch1-1"]);
    let e = own(&deb, "6.19.2-arch1-1", &[&ROOT]).err().unwrap();
    assert!(e.contains("is not a bzImage"), "{e}");
    std::fs::write(arch.join("vmlinuz"), bzimage()).unwrap();
    let e = own(&deb, "6.19.2-arch1-1", &[&ROOT]).err().unwrap();
    assert!(
        e.contains("no /lib/modules/6.19.2-arch1-1/modules.dep") && e.contains("depmod"),
        "{e}"
    );
    // A module of modules.dep that the image does not hold.
    std::fs::remove_file(mods.join("kernel/fs/overlayfs/overlay.ko.xz")).unwrap();
    let e = own(&deb, "6.12.1-amd64", &[&ROOT]).err().unwrap();
    assert!(
        e.contains("names kernel/fs/overlayfs/overlay.ko.xz, and the image has no such file"),
        "{e}"
    );
    // A console that is not built in refuses before the modules.
    std::fs::write(
        deb.join("boot/config-6.12.1-amd64"),
        "CONFIG_SERIAL_8250=m\n",
    )
    .unwrap();
    let e = own(&deb, "6.12.1-amd64", &[&ROOT]).err().unwrap();
    assert!(e.contains("the console of a guest of podvm needs"), "{e}");
    assert!(releases(&d.join("none")).is_empty());
    std::fs::remove_dir_all(&d).unwrap();
}

/// A module is read as it is, or decompressed by its suffix, within its
/// bound; a damaged stream and a stream over the bound are errors.
#[test]
fn a_module_of_an_image_is_read_plain_or_decompressed_within_its_bound() {
    use std::io::Write;
    let d = scratch("read");
    let body = b"MODULE-XZ\n";
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(body).unwrap();
    let files: [(&str, Vec<u8>); 4] = [
        ("a.ko", body.to_vec()),
        ("b.ko.gz", gz.finish().unwrap()),
        ("c.ko.xz", include_bytes!("testdata/module.ko.xz").to_vec()),
        (
            "d.ko.zst",
            ruzstd::encoding::compress_to_vec(
                &body[..],
                ruzstd::encoding::CompressionLevel::Fastest,
            ),
        ),
    ];
    for (n, bytes) in &files {
        let p = d.join(n);
        std::fs::write(&p, bytes).unwrap();
        assert_eq!(read_module(&p).unwrap(), body, "{n}");
        let e = read_module_within(&p, 4).unwrap_err();
        assert!(e.contains("more than 4 bytes"), "{n}: {e}");
    }
    // A small stream that decompresses past the bound.
    let bomb = d.join("f.ko.zst");
    let zeros = vec![0u8; 100_000];
    std::fs::write(
        &bomb,
        ruzstd::encoding::compress_to_vec(&zeros[..], ruzstd::encoding::CompressionLevel::Fastest),
    )
    .unwrap();
    assert!(std::fs::metadata(&bomb).unwrap().len() < 1000);
    let e = read_module_within(&bomb, 1000).unwrap_err();
    assert!(e.contains("decompressed to more than 1000 bytes"), "{e}");
    let bad = d.join("e.ko.xz");
    std::fs::write(&bad, b"\xfd7zXZ\0 not xz").unwrap();
    assert!(read_module(&bad).unwrap_err().contains("e.ko.xz"));
    assert!(read_module(&d.join("none.ko"))
        .unwrap_err()
        .contains("none.ko"));
    std::fs::remove_dir_all(&d).unwrap();
}
