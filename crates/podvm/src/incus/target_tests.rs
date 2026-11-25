//! TODO/machine.md T-2078 remaining (1): the room of a pool that incusd
//! measures (`GET /1.0/storage-pools/NAME/resources`), for a driver other
//! than `dir` and for a pool that podvm does not see.

use std::collections::BTreeMap;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use serde_json::json;

use super::space::{clones, loads, Dev, Need, Plan, Room, Spot, Target};

const MIB: u64 = 1024 * 1024;

/// (driver, source, config, the storage, clones)
type Storage<'a> = (&'a str, &'a str, &'a BTreeMap<String, String>, Dev, bool);

/// (what, pool, importing, freed, (bytes, entries) of the pool's load, words)
type LoadCase = (
    &'static str,
    Spot,
    bool,
    Option<u64>,
    (u64, u64),
    &'static str,
);

/// One rule for the family: the driver and the `source` tell whether a
/// pool has its own storage, and whether the root of an instance is a
/// clone of the volume of its image; an unknown value keeps the need a
/// lower bound.
#[test]
fn the_driver_and_the_source_tell_the_storage_of_a_pool() {
    let thick = BTreeMap::from([("lvm.use_thinpool".to_string(), "false".to_string())]);
    let none = BTreeMap::new();
    #[rustfmt::skip]
    let cases: &[Storage] = &[
        ("dir", "/var/lib/incus/storage-pools/default", &none, Dev::Unknown, false),
        ("btrfs", "/var/lib/incus/disks/default.img", &none, Dev::Own, true),
        ("btrfs", "/srv/btrfs/incus", &none, Dev::Unknown, true),
        ("btrfs", "/dev/sdb1", &none, Dev::Own, true),
        ("zfs", "tank/incus", &none, Dev::Unknown, true),
        ("zfs", "/var/lib/incus/disks/z.img", &none, Dev::Own, true),
        ("lvm", "vg0", &none, Dev::Own, true),
        ("lvm", "vg0", &thick, Dev::Own, false),
        ("lvmcluster", "vg0", &none, Dev::Own, false),
        ("ceph", "rbd", &none, Dev::Own, true),
        ("cephfs", "fs/incus", &none, Dev::Unknown, true),
        ("a new driver", "", &none, Dev::Unknown, true),
    ];
    for (driver, source, config, dev, clone) in cases {
        assert_eq!(Dev::of(driver, source), *dev, "{driver} on {source:?}");
        assert_eq!(clones(driver, config), *clone, "{driver} with {config:?}");
    }
    // Two loads on spots that are not known to be one keep their own bound.
    assert!(Dev::Is(3).same(Dev::Is(3)));
    for (a, b) in [
        (Dev::Is(3), Dev::Is(4)),
        (Dev::Own, Dev::Own),
        (Dev::Unknown, Dev::Unknown),
    ] {
        assert!(!a.same(b), "{a:?} {b:?}");
    }
    // The store's tree that an import removes gives room where it can.
    for (dev, freed, want) in [
        (Dev::Is(3), Some(3), true),
        (Dev::Is(3), Some(4), false),
        (Dev::Is(3), None, false),
        (Dev::Own, Some(3), false),
        (Dev::Unknown, Some(3), true),
        (Dev::Unknown, None, false),
    ] {
        assert_eq!(dev.can_gain(freed), want, "{dev:?} {freed:?}");
    }
}

fn pool(dev: Dev, clones: bool) -> Spot {
    Spot {
        label: "the storage pool `p`".to_string(),
        path: PathBuf::new(),
        dev,
        clones,
        room: Some(Room {
            bytes: 0,
            inodes: None,
        }),
    }
}

/// The loads on a pool that incusd measures: an import needs the tree for
/// the volume of the image on a pool of clones, and a create of an image
/// that incus has needs nothing that podvm can bound there; the tree that
/// the import removes gives room only where the pool can share its file
/// system; a pool and incusd's images that are not known to be one keep a
/// load each.
#[test]
fn a_pool_of_clones_needs_the_tree_once_for_the_volume_of_the_image() {
    let tree = Need {
        bytes: 10,
        entries: 5,
    };
    let images = Spot {
        label: "incusd's images".to_string(),
        path: PathBuf::from("/i"),
        dev: Dev::Is(1),
        clones: false,
        room: None,
    };
    #[rustfmt::skip]
    let cases: &[LoadCase] = &[
        ("btrfs on a loop file, import", pool(Dev::Own, true), true, None, (10, 5), "the volume of the image"),
        ("btrfs on a loop file, import, the tree goes", pool(Dev::Own, true), true, Some(1), (10, 5),
            "the volume of the image"),
        ("btrfs on a loop file, no import", pool(Dev::Own, true), false, None, (0, 0), "a clone"),
        ("zfs on a dataset, import, the tree goes", pool(Dev::Unknown, true), true, Some(1), (0, 0),
            "in the place of podvm's tree"),
        ("dir that podvm does not see, import", pool(Dev::Unknown, false), true, None, (10, 5),
            "the root of the instance"),
        ("dir that podvm does not see, no import", pool(Dev::Unknown, false), false, Some(1), (10, 5),
            "the root of the instance"),
        ("thick lvm, no import", pool(Dev::Own, false), false, None, (10, 5), "the root of the instance"),
    ];
    for (what, p, importing, freed, want, words) in cases {
        let plan = Plan {
            tree,
            upload: 3,
            gzip: true,
            freed: *freed,
            imported: false,
        };
        let got = loads(p, Some(&images), &plan, *importing);
        // With an import, incusd's images keep their own load.
        assert_eq!(got.len(), if *importing { 2 } else { 1 }, "{what}");
        let l = got.last().unwrap();
        assert_eq!((l.need.bytes, l.need.entries), *want, "{what}");
        assert!(l.what.contains(words), "{what}: {}", l.what);
        assert_eq!(l.spot.dev, p.dev, "{what}");
    }
    // The create after an import of this run makes the volume of the
    // image: the line of a failure names the tree, not 0 (measured on
    // btrfs, 2026-11-25).
    let plan = Plan {
        tree,
        imported: true,
        ..Plan::default()
    };
    let got = loads(&pool(Dev::Own, true), Some(&images), &plan, false);
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].need.bytes, got[0].need.entries), (10, 5));
    assert!(
        got[0].what.contains("the volume of the image"),
        "{}",
        got[0].what
    );
    // A pool of the driver `dir` copies the tree with or without it.
    let got = loads(&pool(Dev::Unknown, false), None, &plan, false);
    assert_eq!(
        (got[0].need.bytes, got[0].what),
        (10, "the root of the instance")
    );
}

/// A stand-in incusd of the driver `btrfs`: podvm takes the free space of
/// the pool from incusd, refuses an import that cannot fit by name, and
/// lets a create of an image that incus has through. A pool that incusd
/// does not measure (a total of 0) and a pool that incusd does not know
/// give no check.
#[test]
fn a_pool_of_btrfs_is_measured_by_incusd() {
    let sd = crate::test_socket::SocketDir::new("t2078p");
    let sock = sd.join("unix.socket");
    let l = UnixListener::bind(&sock).unwrap();
    let pool = std::sync::Arc::new(std::sync::Mutex::new("bt"));
    let served = pool.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            let (_, p, _) = super::tests::read_request(&mut s);
            let name = *served.lock().unwrap();
            let body = match p.as_str() {
                "/1.0/profiles/default" => json!({"type": "sync", "metadata": {"name": "default",
                    "devices": {"root": {"type": "disk", "path": "/", "pool": name}}}}),
                "/1.0/storage-pools/bt" => json!({"type": "sync", "metadata": {"name": "bt",
                    "driver": "btrfs", "config": {"source": "/var/lib/incus/disks/bt.img"}}}),
                "/1.0/storage-pools/bt/resources" => json!({"type": "sync", "metadata": {
                    "space": {"used": 90 * MIB, "total": 100 * MIB}, "inodes": {"used": 0, "total": 0}}}),
                "/1.0/storage-pools/nototal" => {
                    json!({"type": "sync", "metadata": {"name": "nototal",
                    "driver": "zfs", "config": {"source": "tank"}}})
                }
                "/1.0/storage-pools/nototal/resources" => json!({"type": "sync", "metadata": {}}),
                _ => json!({"type": "error", "error_code": 404, "error": "not found"}),
            };
            let status = if body["type"] == "sync" { 200 } else { 404 };
            let out = body.to_string();
            let _ = s.write_all(
                format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}",
                    out.len()
                )
                .as_bytes(),
            );
        }
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let c = pod_incus::Client::new(pod_incus::Settings::new(&sock));
    let t = rt
        .block_on(Target::find(&c, "btrfs"))
        .expect("incusd measures the pool");
    assert_eq!(t.pool.dev, Dev::Own);
    assert!(t.pool.clones);
    let plan = |bytes| Plan {
        tree: Need {
            bytes,
            entries: 400_000,
        },
        gzip: true,
        ..Plan::default()
    };
    let e = t.check(&plan(50 * MIB), true).expect_err("no room");
    assert!(
        e.starts_with(
            "the storage pool `bt` (driver `btrfs`, as incusd measures it) at \
             /var/lib/incus/disks/bt.img has 10.0 MiB free and the volume of the image"
        ),
        "{e}"
    );
    assert!(e.contains("needs at least 50.0 MiB"), "{e}");
    assert!(!e.contains('\n'), "{e}");
    // btrfs gives no count of inodes: 400 000 entries refuse nothing.
    assert_eq!(t.check(&plan(MIB), true), Ok(Vec::new()));
    // An image that incus has: its root is a clone, and nothing refuses.
    assert_eq!(t.check(&plan(50 * MIB), false), Ok(Vec::new()));
    // The line after a refusal by size names the free space of incusd.
    let h = t
        .after(
            Some(&plan(50 * MIB)),
            true,
            super::space::Transport::TooLarge,
            64,
        )
        .expect("a line");
    assert!(h.contains("(driver `btrfs`, as incusd measures it)"), "{h}");
    assert!(h.contains("has 10.0 MiB free now"), "{h}");
    for name in ["nototal", "unknown"] {
        *pool.lock().unwrap() = name;
        assert_eq!(rt.block_on(Target::find(&c, "btrfs")), None, "{name}");
    }
}
