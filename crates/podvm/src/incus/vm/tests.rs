//! TODO/machine.md T-1998: the decisions of `--podvm-vm` and the build of
//! its disk, with no incusd.

use super::*;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

/// Each decision of the VM mode: a refusal by name, or the value it takes.
#[test]
fn the_vm_mode_takes_or_refuses_each_ask() {
    let ok = Ask {
        vm: true,
        incus: true,
        init_image: true,
        stays: true,
        limits: vec![("--memory", "limits.memory"), ("--cpus", "limits.cpu")],
        ..Ask::default()
    };
    let with = |f: &dyn Fn(&mut Ask)| {
        let mut a = ok.clone();
        f(&mut a);
        a
    };
    // (the ask, None where it runs, else words of its refusal)
    let rows: Vec<(Ask, Option<&str>)> = vec![
        (Ask::default(), None),
        (ok.clone(), None),
        (with(&|a| a.incus = false), Some("takes --podvm-tier=incus")),
        (
            with(&|a| a.init_image = false),
            Some("give --podvm-init=image"),
        ),
        (
            with(&|a| a.stays = false),
            Some("holds on run -d and create"),
        ),
        (with(&|a| a.engine = true), Some("--podvm-engine")),
        (with(&|a| a.channel = true), Some("--podvm-channel")),
        (
            with(&|a| a.limits.push(("--pids-limit", "limits.processes"))),
            Some("--pids-limit: incusd has no limits.processes for a VM"),
        ),
        (
            with(&|a| a.limits.push(("--storage-opt", "podvm.root.size"))),
            Some("--storage-opt"),
        ),
        (
            with(&|a| a.limits = vec![("--cpus", "limits.cpu.allowance")]),
            Some("limits.cpu.allowance"),
        ),
    ];
    for (ask, want) in rows {
        let got = refusal(&ask);
        match want {
            None => assert_eq!(got, None, "{ask:?}"),
            Some(w) => {
                let got = got.unwrap_or_else(|| panic!("{ask:?} runs, and {w:?} was due"));
                assert!(got.contains(w) && got.contains("T-1998"), "{got}");
            }
        }
    }
    // The drivers of incusd: `qemu` among them, or a refusal.
    for (driver, runs) in [
        ("lxc | qemu", true),
        ("qemu", true),
        ("lxc", false),
        ("", false),
        ("lxc | qemux", false),
    ] {
        assert_eq!(no_vm_driver(driver).is_none(), runs, "{driver:?}");
    }
    // The label of the root: the one of the fstab, else podvm's.
    let azure = "# a comment\nLABEL=cloudimg-rootfs\t/\text4\tdiscard 0 1\n\
                 LABEL=UEFI\t/boot/efi\tvfat\tumask=0077 0 1\n";
    for (fstab, label) in [
        (azure, "cloudimg-rootfs"),
        ("UUID=0b1c / ext4 defaults 0 1\n", DEFAULT_LABEL),
        ("LABEL=a-label-of-17-bytes / ext4\n", DEFAULT_LABEL),
        ("LABEL=bad'q / ext4\n", DEFAULT_LABEL),
        ("LABEL=data /data ext4\n", DEFAULT_LABEL),
        ("", DEFAULT_LABEL),
    ] {
        assert_eq!(root_label(fstab), label, "{fstab:?}");
    }
    // The paths of the store that raw.qemu and raw.apparmor quote.
    for (p, plain) in [
        ("/home/runner/.local/share/podvm/incus-vm/a/vmlinuz", true),
        ("/store with space/vmlinuz", true),
        ("/st'ore/vmlinuz", false),
        ("/st\"ore/vmlinuz", false),
        ("/st*re/vmlinuz", false),
        ("/st,re/vmlinuz", false),
        ("/st\nre/vmlinuz", false),
        ("relative/vmlinuz", false),
    ] {
        assert_eq!(host_path(Path::new(p)).is_ok(), plain, "{p:?}");
    }
    let q = raw_qemu("/s/vmlinuz", "/s/initrd", "cloudimg-rootfs");
    assert_eq!(
        q,
        "-kernel '/s/vmlinuz' -initrd '/s/initrd' -append 'root=LABEL=cloudimg-rootfs rw \
         console=ttyS0 fsck.mode=skip fstab=no'"
    );
    assert_eq!(
        raw_apparmor("/s/vmlinuz", "/s/initrd"),
        "\"/s/vmlinuz\" r,\n\"/s/initrd\" r,"
    );
    assert_eq!(root_size(1), 1 << 30);
    assert_eq!(root_size(1 << 30), 1 << 30);
    assert_eq!(root_size((9 << 30) + 1), 10 << 30);
    assert_eq!(
        alias("sha256:0123456789abcdef0123", "fedcba9876543210ff"),
        format!(
            "podvm-vm-0123456789abcdef-fedcba9876543210-v{}",
            build::FORMAT
        )
    );
    // The bound of the linger of guest-hold.
    let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        super::super::linger_of(&args(&["--run", "--linger", "120"])),
        Some(120)
    );
    assert_eq!(
        super::super::linger_of(&args(&["--linger", "99999"])),
        Some(3600)
    );
    assert_eq!(super::super::linger_of(&args(&["--linger", "x"])), None);
    assert_eq!(super::super::linger_of(&args(&["--run"])), None);
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "podvm-t1998-{name}-{}",
        crate::machine::guest::init::nonce().unwrap()
    ));
    std::fs::create_dir(&d).unwrap();
    d
}

/// The bytes of the disk in the qcow2 file `q`, read through its L1 and
/// L2 tables; and a check of each reference count.
fn read_qcow2(q: &[u8]) -> Vec<u8> {
    let be64 = |at: usize| u64::from_be_bytes(q[at..at + 8].try_into().unwrap());
    let be32 = |at: usize| u32::from_be_bytes(q[at..at + 4].try_into().unwrap());
    assert_eq!(&q[..4], b"QFI\xfb");
    assert_eq!((be32(4), be32(20), be32(96), be32(100)), (3, 16, 4, 104));
    let cluster = qcow2::CLUSTER;
    let size = be64(24);
    let (l1_size, l1_at) = (be32(36) as usize, be64(40) as usize);
    let (rt_at, rt_clusters) = (be64(48) as usize, be32(56) as u64);
    assert_eq!(q.len() as u64 % cluster, 0);
    let clusters = q.len() as u64 / cluster;
    // Each cluster of the file has the count 1, and no other has one.
    let mut refs = 0u64;
    for k in 0..(rt_clusters * cluster / 8) as usize {
        let rb = be64(rt_at + k * 8) as usize;
        if rb == 0 {
            continue;
        }
        for c in 0..qcow2::REFS_PER_BLOCK as usize {
            let n = u16::from_be_bytes([q[rb + 2 * c], q[rb + 2 * c + 1]]) as u64;
            let host = k as u64 * qcow2::REFS_PER_BLOCK + c as u64;
            assert_eq!(n, u64::from(host < clusters), "the count of cluster {host}");
            refs += n;
        }
    }
    assert_eq!(refs, clusters);
    let mut out = vec![0u8; size as usize];
    for j in 0..l1_size {
        let l2 = be64(l1_at + j * 8);
        if l2 == 0 {
            continue;
        }
        assert_ne!(l2 & qcow2::COPIED, 0);
        let l2 = (l2 & !qcow2::COPIED) as usize;
        for e in 0..qcow2::L2_ENTRIES as usize {
            let d = be64(l2 + e * 8);
            if d == 0 {
                continue;
            }
            let d = (d & !qcow2::COPIED) as usize;
            let at = (j as u64 * qcow2::L2_ENTRIES + e as u64) * cluster;
            let n = (size - at).min(cluster) as usize;
            out[at as usize..at as usize + n].copy_from_slice(&q[d..d + n]);
        }
    }
    out
}

/// The build of the disk: the tree with the files of the agent and podvm,
/// a free spare, the label of the image, as qcow2 that gives the raw disk
/// back; e2fsck finds no fault where the host has it.
#[test]
fn the_disk_of_a_vm_holds_the_image_the_agent_and_room() {
    // qcow2 alone: clusters past an L2 table, a short last cluster, holes.
    let d = scratch("qcow2");
    let raw_path = d.join("raw");
    let size = (qcow2::L2_ENTRIES + 3) * qcow2::CLUSTER + 4096;
    let raw = std::fs::File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&raw_path)
        .unwrap();
    raw.set_len(size).unwrap();
    for (at, b) in [(0u64, 1u8), (3 * qcow2::CLUSTER + 7, 2), (size - 1, 3)] {
        raw.write_all_at(&[b], at).unwrap();
    }
    let out = std::fs::File::create_new(d.join("q")).unwrap();
    let len = qcow2::write(&raw, &out).unwrap();
    let q = std::fs::read(d.join("q")).unwrap();
    assert_eq!(q.len() as u64, len);
    assert_eq!(read_qcow2(&q), std::fs::read(&raw_path).unwrap());
    // incusd reads the image with qemu-img: where the host has it, it
    // finds no fault and the bytes of the raw disk.
    if let Some(qi) = ["/usr/bin/qemu-img", "/usr/local/bin/qemu-img"]
        .iter()
        .find(|p| Path::new(p).exists())
    {
        let ok = |args: &[&str]| {
            let o = std::process::Command::new(qi).args(args).output().unwrap();
            assert!(o.status.success(), "{args:?}: {o:?}");
        };
        let (qp, rp) = (d.join("q"), raw_path.clone());
        ok(&["check", "-f", "qcow2", qp.to_str().unwrap()]);
        ok(&[
            "compare",
            "-f",
            "qcow2",
            "-F",
            "raw",
            qp.to_str().unwrap(),
            rp.to_str().unwrap(),
        ]);
    }
    let l = qcow2::Layout::of(size, vec![0, 3, qcow2::L2_ENTRIES + 3]);
    assert_eq!((l.l1_entries, l.l2.len(), l.data.len()), (2, 2, 3));
    std::fs::remove_dir_all(&d).unwrap();

    // The disk of a small image.
    let d = scratch("disk");
    let rootfs = d.join("rootfs");
    std::fs::create_dir_all(rootfs.join("etc")).unwrap();
    std::fs::write(rootfs.join("etc/machine-id"), "0123456789abcdef\n").unwrap();
    std::fs::write(rootfs.join("etc/hostname"), "image\n").unwrap();
    let podvm = d.join("podvm-bin");
    std::fs::write(&podvm, b"\x7fELF a podvm of the test").unwrap();
    let parts = d.join("parts");
    std::fs::create_dir(&parts).unwrap();
    let bytes =
        build::write_disk(&rootfs, "sha256:00ff", "cloudimg-rootfs", &podvm, &parts).unwrap();
    assert!(bytes >= SPARE, "{bytes}");
    assert!(!parts.join("root.raw").exists());
    if let Some(qi) = ["/usr/bin/qemu-img", "/usr/local/bin/qemu-img"]
        .iter()
        .find(|p| Path::new(p).exists())
    {
        let o = std::process::Command::new(qi)
            .args(["check", "-f", "qcow2"])
            .arg(parts.join("rootfs.img"))
            .output()
            .unwrap();
        assert!(o.status.success(), "{o:?}");
    }
    let q = std::fs::read(parts.join("rootfs.img")).unwrap();
    // The qcow2 file holds the tree, not the spare.
    assert!((q.len() as u64) < 64 << 20, "{}", q.len());
    let disk = read_qcow2(&q);
    assert_eq!(disk.len() as u64, bytes);
    assert_eq!(&disk[1024 + 120..1024 + 135], b"cloudimg-rootfs");
    let img = d.join("disk.raw");
    std::fs::write(&img, &disk).unwrap();
    drop(disk);
    if let Some(dbg) = ["/usr/sbin/debugfs", "/sbin/debugfs"]
        .iter()
        .find(|p| Path::new(p).exists())
    {
        let cat = |p: &str| {
            let o = std::process::Command::new(dbg)
                .args(["-R", &format!("cat {p}")])
                .arg(&img)
                .output()
                .unwrap();
            String::from_utf8_lossy(&o.stdout).into_owned()
        };
        assert_eq!(cat("/etc/machine-id"), "");
        assert_eq!(cat("/etc/hostname"), "image\n");
        assert_eq!(cat("/etc/udev/rules.d/99-incus-agent.rules"), agent::RULE);
        assert_eq!(cat("/etc/systemd/system/incus-agent.service"), agent::UNIT);
        assert_eq!(cat(agent::SETUP_PATH), agent::SETUP);
        assert_eq!(cat("/.podvm/podvm"), "\x7fELF a podvm of the test");
    }
    if let Some(fsck) = ["/usr/sbin/e2fsck", "/sbin/e2fsck"]
        .iter()
        .find(|p| Path::new(p).exists())
    {
        let o = std::process::Command::new(fsck)
            .args(["-fn"])
            .arg(&img)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&o.stdout);
        // The writer of T-1994 makes no lost+found: that note, the passes
        // and the summary are each line of a disk with no fault.
        let faults: Vec<&str> = text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with("Pass "))
            .filter(|l| !l.contains("lost+found not found") && !l.contains(" files ("))
            .filter(|l| !l.contains("WARNING: Filesystem still has errors"))
            .collect();
        assert!(faults.is_empty(), "{text}");
        assert!(text.contains(" files ("), "{text}");
    }
    let m = build::metadata_tar(1_700_000_000).unwrap();
    let mut a = tar::Archive::new(&m[..]);
    let mut e = a.entries().unwrap().next().unwrap().unwrap();
    assert_eq!(e.path().unwrap().to_str(), Some("metadata.yaml"));
    let mut text = String::new();
    std::io::Read::read_to_string(&mut e, &mut text).unwrap();
    assert!(text.contains("architecture: x86_64\ncreation_date: 1700000000\n"));
    std::fs::remove_dir_all(&d).unwrap();
}
