//! TODO/machine.md T-1994, T-1999: the root disk of an image and its
//! cache.

use super::{image, inode, root, tree};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "podvm-t1994-{name}-{}",
        super::super::init::nonce().unwrap()
    ));
    std::fs::create_dir(&d).unwrap();
    d
}

unsafe extern "C" {
    fn mkfifo(path: *const std::ffi::c_char, mode: u32) -> i32;
}

/// A v2 file capability with `cap_net_raw` (13) effective and permitted.
const CAP: &str = "0x0100000200200000000000000000000000000000";
/// A POSIX ACL of the VFS: the owner rw, the user 1000 r, the group r, the
/// mask r and the others r.
const ACL: &str =
    "0x02000000010006000000000002000400e8030000040004000000000010000400000000002000040000000000";

fn unhex(s: &str) -> Vec<u8> {
    let d = s.trim_start_matches("0x");
    (0..d.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&d[i..i + 2], 16).unwrap())
        .collect()
}

/// The tree of the tests: each kind of file, a hard link, a directory of
/// mode 0000, and the owners, the modes and the attributes of a sidecar.
fn fixture(d: &Path) -> PathBuf {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let r = d.join("rootfs");
    for dir in ["etc", "bin", "closed", "usr/share"] {
        std::fs::create_dir_all(r.join(dir)).unwrap();
    }
    std::fs::write(r.join("etc/hostname"), b"podvm\n").unwrap();
    std::fs::write(r.join("etc/shadow"), b"root:*::0:::::\n").unwrap();
    std::fs::write(r.join("etc/acl"), b"acl\n").unwrap();
    let big: Vec<u8> = (0..12_345u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(r.join("bin/big"), &big).unwrap();
    std::fs::hard_link(r.join("bin/big"), r.join("bin/big2")).unwrap();
    let mut zeros = vec![0u8; 3 * 4096];
    zeros.push(b'x');
    std::fs::write(r.join("bin/zeros"), &zeros).unwrap();
    std::fs::write(r.join("bin/ping"), b"ping").unwrap();
    std::fs::write(r.join("empty"), b"").unwrap();
    std::fs::write(r.join("closed/inner"), b"inner").unwrap();
    symlink("busybox", r.join("bin/sh")).unwrap();
    symlink("x".repeat(100), r.join("long")).unwrap();
    let fifo = std::ffi::CString::new(r.join("fifo").to_str().unwrap()).unwrap();
    // SAFETY: a NUL-ended path of this frame.
    assert_eq!(unsafe { mkfifo(fifo.as_ptr(), 0o600) }, 0);
    std::fs::set_permissions(r.join("closed"), std::fs::Permissions::from_mode(0o000)).unwrap();
    let side = [
        r#"{"path":"etc/shadow","uid":0,"gid":42,"mode":"0640"}"#.to_string(),
        format!(
            r#"{{"path":"bin/ping","uid":0,"gid":0,"mode":"4755","xattrs":[{{"name":"security.capability","value":"{CAP}"}}]}}"#
        ),
        format!(
            r#"{{"path":"etc/acl","uid":7,"gid":8,"mode":"0644","xattrs":[{{"name":"system.posix_acl_access","value":"{ACL}"}},{{"name":"user.big","value":"0x{}"}}]}}"#,
            "ab".repeat(2000)
        ),
        r#"{"path":"etc/hostname","uid":0,"gid":0,"mode":"0644","xattrs":[{"name":"user.k","value":"0x76"}]}"#
            .to_string(),
    ];
    std::fs::write(d.join(".meta.jsonl"), side.join("\n") + "\n").unwrap();
    r
}

fn debugfs(img: &Path, cmd: &str) -> String {
    let out = std::process::Command::new("/usr/sbin/debugfs")
        .args(["-R", cmd])
        .arg(img)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn build(rootfs: &Path, img: &Path) {
    let t = tree::Tree::walk(rootfs).unwrap();
    let p = image::plan(&t).unwrap();
    let f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(img)
        .unwrap();
    f.set_len(p.layout.blocks * 4096).unwrap();
    let names = super::ext::Names {
        uuid: [7; 16],
        seed: [9; 16],
        label: b"podvm-root",
    };
    image::write(&p, &t, &f, &names).unwrap();
}

/// One table for each kind of file of the disk, with e2fsprogs as the
/// reader: `debugfs` gives each file back as the kernel would, and
/// `e2fsck -f -n` finds no fault but the missing `lost+found`, which the
/// image does not have. The disk keeps the owner and the mode of the
/// sidecar, `security.capability`, a POSIX ACL, an attribute in the inode
/// and one in a block of its own, a hard link, both kinds of link, a FIFO
/// and a file of mode 0000 in a directory of mode 0000.
#[test]
fn the_root_disk_keeps_each_kind_of_file_and_each_attribute() {
    if !Path::new("/usr/sbin/debugfs").exists() || !Path::new("/usr/sbin/e2fsck").exists() {
        eprintln!("SKIP: no e2fsprogs on this host; the encoder tests below still run");
        return;
    }
    let d = scratch("kinds");
    let img = d.join("root.img");
    build(&fixture(&d), &img);
    let big: Vec<u8> = (0..12_345u32).map(|i| (i % 251) as u8).collect();
    let mut zeros = vec![0u8; 3 * 4096];
    zeros.push(b'x');
    let x100 = "x".repeat(100);
    // (the command of debugfs, what its output holds)
    let cases: Vec<(String, Vec<u8>)> = vec![
        ("cat /etc/hostname".into(), b"podvm\n".to_vec()),
        ("cat /bin/big".into(), big.clone()),
        ("cat /bin/big2".into(), big),
        ("cat /bin/zeros".into(), zeros),
        ("cat /closed/inner".into(), b"inner".to_vec()),
        ("cat /long".into(), x100.into_bytes()),
        ("stat /etc/shadow".into(), b"Group:    42".to_vec()),
        ("stat /etc/shadow".into(), b"Mode:  0640".to_vec()),
        (
            "stat /etc/acl".into(),
            b"User:     7   Group:     8".to_vec(),
        ),
        ("stat /bin/ping".into(), b"Mode:  04755".to_vec()),
        ("stat /bin/big".into(), b"Links: 2".to_vec()),
        (
            "stat /bin/sh".into(),
            b"Fast link dest: \"busybox\"".to_vec(),
        ),
        ("stat /fifo".into(), b"Type: FIFO".to_vec()),
        ("stat /closed".into(), b"Mode:  0000".to_vec()),
        ("stat /empty".into(), b"Size: 0".to_vec()),
        ("stat /".into(), b"Links: 6".to_vec()),
        (
            "ea_list /etc/hostname".into(),
            b"user.k (1) = \"v\"".to_vec(),
        ),
    ];
    for (cmd, want) in &cases {
        let got = std::process::Command::new("/usr/sbin/debugfs")
            .args(["-R", cmd])
            .arg(&img)
            .output()
            .unwrap()
            .stdout;
        let hit = if cmd.starts_with("cat ") {
            got == *want
        } else {
            got.windows(want.len()).any(|w| w == &want[..])
        };
        assert!(
            hit,
            "{cmd}: {:?} is not in {:?}",
            String::from_utf8_lossy(want),
            String::from_utf8_lossy(&got)
        );
    }
    for (path, name, want) in [
        ("/bin/ping", "security.capability", unhex(CAP)),
        ("/etc/acl", "system.posix_acl_access", unhex(ACL)),
        ("/etc/acl", "user.big", vec![0xab; 2000]),
    ] {
        let out = d.join("ea.out");
        let _ = std::fs::remove_file(&out);
        debugfs(&img, &format!("ea_get -f {} {path} {name}", out.display()));
        assert_eq!(std::fs::read(&out).ok(), Some(want), "{path} {name}");
    }
    assert!(
        debugfs(&img, "stat /etc/acl").contains("File ACL: "),
        "user.big is not in a block"
    );
    let fsck = std::process::Command::new("/usr/sbin/e2fsck")
        .args(["-f", "-n"])
        .arg(&img)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&fsck.stdout).into_owned();
    let faults: Vec<&str> = text
        .lines()
        .filter(|l| l.contains('?') && !l.contains("/lost+found not found"))
        .collect();
    assert!(faults.is_empty(), "e2fsck: {faults:?}\n{text}");
    assert!(
        fsck.status.code().is_some_and(|c| c <= 4),
        "e2fsck {}\n{text}",
        fsck.status
    );
    restore(&d);
}

fn restore(d: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(
        d.join("rootfs/closed"),
        std::fs::Permissions::from_mode(0o755),
    );
    let _ = std::fs::remove_dir_all(d);
}

/// Each attribute that the disk cannot hold refuses the plan by its path
/// and its name, and the encoders refuse a bad value.
#[test]
fn the_plan_refuses_what_the_disk_cannot_hold() {
    let d = scratch("refuse");
    let r = d.join("rootfs");
    std::fs::create_dir_all(&r).unwrap();
    std::fs::write(r.join("f"), b"f").unwrap();
    for (row, want) in [
        (
            r#""xattrs":[{"name":"foo.bar","value":"0x01"}]"#.to_string(),
            "/f: the attribute foo.bar",
        ),
        (
            format!(
                r#""xattrs":[{{"name":"user.huge","value":"0x{}"}}]"#,
                "00".repeat(5000)
            ),
            "/f: 1 attributes with 5000 bytes",
        ),
        (
            r#""xattrs":[{"name":"system.posix_acl_access","value":"0x0100000001000600ffffffff"}]"#
                .to_string(),
            "not version 2",
        ),
    ] {
        let line = format!(r#"{{"path":"f","uid":0,"gid":0,"mode":"0644",{row}}}"#);
        std::fs::write(d.join(".meta.jsonl"), line + "\n").unwrap();
        let t = tree::Tree::walk(&r).unwrap();
        let err = image::plan(&t).err().unwrap_or_default();
        assert!(err.contains(want), "{want:?} is not in {err:?}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// The extent tree of each count of runs: in the inode up to 4, one level
/// of leaves above that, and a refusal past 4 leaves of 340.
#[test]
fn the_extents_of_each_count_of_runs() {
    for (runs, leaves) in [
        (0usize, 0usize),
        (1, 0),
        (4, 0),
        (5, 1),
        (340, 1),
        (341, 2),
        (1360, 4),
    ] {
        let r: Vec<(u64, u64)> = (0..runs as u64).map(|i| (100 + 3 * i, 2)).collect();
        let at: Vec<u64> = (0..leaves as u64).map(|i| 9000 + i).collect();
        let e = inode::extents(&r, &at).unwrap();
        assert_eq!(e.leaves.len(), leaves, "{runs} runs");
        assert_eq!(&e.iblock[..2], &[0x0a, 0xf3], "{runs} runs: the magic");
        assert_eq!(
            u16::from_le_bytes([e.iblock[6], e.iblock[7]]),
            u16::from(leaves > 0)
        );
    }
    let r: Vec<(u64, u64)> = (0..1361).map(|i| (100 + 3 * i, 2)).collect();
    assert!(inode::extents(&r, &[1, 2, 3, 4, 5])
        .unwrap_err()
        .contains("1360 at most"));
    assert!(inode::extents(&[(5, 0)], &[]).is_err(), "an empty run");
}

/// A planted fault for each rule of the cache: a second run takes the
/// disk of the first; a lost record is a new hash, not a build; a changed
/// byte, a cut file and a trailer of another format are built again; a
/// partial file of a dead build goes; the disk is read-only for its owner.
#[test]
fn the_cache_of_the_root_disk_takes_only_a_whole_disk() {
    use std::os::unix::fs::{FileExt, PermissionsExt};
    let d = scratch("cache");
    let r = d.join("rootfs");
    std::fs::create_dir_all(r.join("bin")).unwrap();
    std::fs::write(r.join("bin/sh"), b"#!sh").unwrap();
    let store = d.join("store");
    let digest = "sha256:00ff";
    let (p, got) = root::ensure(&store, digest, &r).unwrap();
    assert_eq!(got, root::Got::Built(None));
    assert_eq!(p, root::path(&store, digest));
    let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o444, "the disk is not read-only");
    let writable =
        |p: &Path| std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o644)).unwrap();
    type Plant = fn(&Path);
    let cases: [(&str, Plant, bool); 6] = [
        ("nothing", |_| {}, false),
        (
            "the record",
            |p| std::fs::remove_file(p.with_extension("ok")).unwrap(),
            false,
        ),
        (
            "a byte",
            |p| {
                let f = std::fs::OpenOptions::new().write(true).open(p).unwrap();
                f.write_all_at(b"Z", 4096 * 3).unwrap();
            },
            true,
        ),
        (
            "the end",
            |p| {
                let f = std::fs::OpenOptions::new().write(true).open(p).unwrap();
                let len = f.metadata().unwrap().len();
                f.set_len(len - 4096).unwrap();
            },
            true,
        ),
        (
            "the trailer",
            |p| {
                let f = std::fs::OpenOptions::new().write(true).open(p).unwrap();
                let len = f.metadata().unwrap().len();
                f.write_all_at(b"podvm-root-disk 0", len - 4096).unwrap();
            },
            true,
        ),
        (
            "a partial file",
            |p| {
                std::fs::write(PathBuf::from(format!("{}.partial-dead", p.display())), b"x")
                    .unwrap();
                std::fs::remove_file(p).unwrap();
            },
            true,
        ),
    ];
    for (what, plant, rebuilt) in cases {
        if p.exists() {
            writable(&p);
        }
        plant(&p);
        if p.exists() {
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o444)).unwrap();
        }
        let (_, got) = root::ensure(&store, digest, &r).unwrap();
        assert_eq!(
            matches!(got, root::Got::Built(_)),
            rebuilt,
            "{what}: {got:?}"
        );
        assert!(
            root::check(&p, "n").is_ok(),
            "{what}: the disk after the run"
        );
    }
    let left: Vec<_> = std::fs::read_dir(p.parent().unwrap())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("partial"))
        .collect();
    assert!(left.is_empty(), "{left:?}");
    let _ = std::fs::remove_dir_all(&d);
}

/// The choice of the root: the default takes the disk where the guest can
/// have it, a variable selects the initramfs, and each other value or an
/// asked disk that the guest cannot have refuses by name.
#[test]
fn the_root_of_the_guest_is_the_disk_where_it_can_be() {
    type Want = Result<bool, &'static str>;
    // (the variable, an x86_64 guest, the pinned kernel, --podvm-engine)
    let cases: [(Option<&str>, bool, bool, bool, Want); 10] = [
        (None, true, true, false, Ok(true)),
        (None, false, true, false, Ok(false)),
        (None, true, false, false, Ok(false)),
        (None, true, true, true, Ok(false)),
        (Some("initramfs"), true, true, false, Ok(false)),
        (Some("disk"), true, true, false, Ok(true)),
        (Some("disk"), true, true, true, Ok(true)),
        (Some("disk"), false, true, false, Err("linux/arm64")),
        (
            Some("disk"),
            true,
            false,
            false,
            Err("PODVM_MACHINE_KERNEL"),
        ),
        (
            Some("ram"),
            true,
            true,
            false,
            Err("the values are disk and initramfs"),
        ),
    ];
    for (var, amd64, pinned, engine, want) in cases {
        match (super::root_choice(var, amd64, pinned, engine), want) {
            (Ok(g), Ok(w)) => assert_eq!(g, w, "{var:?} {amd64} {pinned}"),
            (Err(g), Err(w)) => assert!(g.contains(w) && g.contains("T-1994"), "{g}"),
            (g, w) => panic!("{var:?} {amd64} {pinned}: want {w:?}, got {g:?}"),
        }
    }
    let mut argv: Vec<String> = ["-append", "console=ttyS0 rdinit=/podvm-init", "-m"]
        .map(String::from)
        .to_vec();
    root::with_init(
        &mut argv,
        &["virtio_blk".into(), "ext4".into()],
        true,
        None,
        "abc",
    );
    assert_eq!(
        argv[1],
        "console=ttyS0 rdinit=/podvm/podvm -- guest-root --load virtio_blk,ext4 --disk abc"
    );
}
