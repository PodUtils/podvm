//! TODO/machine.md T-2078 (PodUtils/podrunner#346): the room of incusd for
//! an image, and the class of a refusal by size against a stand-in incusd.

use std::io::Write;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;

use super::space::{
    dir_only, judge, line, loads, need_of, transport, Dev, Load, Need, Plan, Room, Spot, Target,
    Transport, Verdict,
};

const MIB: u64 = 1024 * 1024;

fn spot(label: &str, dev: u64) -> Spot {
    Spot {
        label: label.to_string(),
        path: PathBuf::from(format!("/p/{dev}")),
        dev: Dev::Is(dev),
        clones: false,
        room: None,
    }
}

/// One rule for the family: refuse below the lower bound of bytes or of
/// inodes, warn below the estimate, else nothing; the line names the
/// place, the free and the need, and a refusal names the remedy.
#[test]
fn the_need_of_an_image_against_the_free_space_of_a_pool() {
    let n = |bytes: u64, entries: u64| Need { bytes, entries };
    let r = |bytes: u64, inodes: Option<u64>| Room { bytes, inodes };
    #[rustfmt::skip]
    let cases: &[(&str, Need, Room, Verdict, &[&str])] = &[
        ("room to spare", n(100 * MIB, 10), r(1000 * MIB, Some(1000)), Verdict::Fits, &[]),
        ("exactly the estimate", n(100 * MIB, 10), r(130 * MIB, Some(10)), Verdict::Fits, &[]),
        ("below the estimate", n(100 * MIB, 10), r(120 * MIB, Some(10)),
            Verdict::Tight { estimate: 130 * MIB }, &["warning:", "120.0 MiB free", "at least 100.0 MiB", "estimated 130.0 MiB"]),
        ("exactly the bound", n(100 * MIB, 10), r(100 * MIB, Some(10)),
            Verdict::Tight { estimate: 130 * MIB }, &["warning:"]),
        ("one byte short", n(100 * MIB, 10), r(100 * MIB - 1, Some(1000)), Verdict::NoBytes,
            &["has 100.0 MiB free", "needs at least 100.0 MiB", "free space there"]),
        ("the hosted case", n(16 * 1024 * MIB, 500_000), r(2600 * MIB, Some(9_000_000)), Verdict::NoBytes,
            &["2.54 GiB free", "at least 16.00 GiB"]),
        ("no inodes, bytes to spare", n(0, 400_002), r(331 * MIB, Some(102_380)), Verdict::NoInodes,
            &["102380 free inodes", "at least 400002", "free inodes there"]),
        ("inodes not counted", n(1, 400_002), r(331 * MIB, None), Verdict::Fits, &[]),
        ("bytes first", n(100 * MIB, 10), r(1, Some(0)), Verdict::NoBytes, &["free space there"]),
        ("a need past u64", n(u64::MAX, 1), r(u64::MAX, None), Verdict::Fits, &[]),
        ("an empty image", n(0, 0), r(0, Some(0)), Verdict::Fits, &[]),
        // T-2085: the upload of gzip adds to the estimate only.
        ("the upload, room for it", n(100 * MIB, 10), r(181 * MIB, None), Verdict::Fits, &[]),
        ("the upload, below its estimate", n(100 * MIB, 10), r(150 * MIB, None),
            Verdict::Tight { estimate: 180 * MIB }, &["warning:", "estimated 180.0 MiB"]),
        ("the upload alone, no bound", n(0, 1), r(1, None), Verdict::Tight { estimate: 50 * MIB },
            &["warning:", "at least 0 B"]),
    ];
    for (what, need, room, want, words) in cases {
        // The upload is 50 MiB in its cases, else none.
        let extra = if what.starts_with("the upload") {
            50 * MIB
        } else {
            0
        };
        let got = judge(*need, extra, *room);
        assert_eq!(got, *want, "{what}");
        let load = Load {
            spot: spot("the storage pool `default`", 1),
            what: "the root of the instance",
            need: *need,
            extra,
        };
        let text = line(&load, *room, got);
        assert_eq!(text.is_none(), words.is_empty(), "{what}: {text:?}");
        if let Some(t) = text {
            assert!(!t.contains('\n'), "{what}: one line: {t}");
            assert!(
                t.contains("the storage pool `default` at /p/1"),
                "{what}: {t}"
            );
            assert!(t.contains("T-2078"), "{what}: {t}");
            for w in *words {
                assert!(t.contains(w), "{what}: {w:?} not in {t}");
            }
        }
    }
}

/// The loads of a create: on one file system the import and the root add
/// up; on two, each has its own; with no import, only the root.
/// TODO/machine.md T-2085: the upload (gzip) has no lower bound and its
/// estimate is the bytes of the layers; where the import removes the
/// store's tree on the file system of the pool, the root takes its place.
#[test]
fn the_import_and_the_root_add_up_on_one_file_system() {
    let tree = Need {
        bytes: 10,
        entries: 5,
    };
    let pool = spot("the storage pool `default`", 1);
    let same = spot("incusd's images", 1);
    let other = spot("incusd's images", 2);
    // (what, images, importing, gzip, freed, [(dev, bytes, entries, extra)])
    type Case<'a> = (
        &'a str,
        Option<&'a Spot>,
        bool,
        bool,
        Option<u64>,
        &'a [(u64, u64, u64, u64)],
    );
    #[rustfmt::skip]
    let cases: &[Case] = &[
        ("import, one file system, plain, the tree stays", Some(&same), true, false, None, &[(1, 20, 6, 0)]),
        ("import, one file system, plain, the tree goes", Some(&same), true, false, Some(1), &[(1, 10, 1, 0)]),
        ("import, one file system, gzip, the tree stays", Some(&same), true, true, None, &[(1, 10, 6, 3)]),
        ("import, one file system, gzip, the tree goes", Some(&same), true, true, Some(1), &[(1, 0, 1, 3)]),
        ("import, a tree elsewhere goes", Some(&same), true, true, Some(7), &[(1, 10, 6, 3)]),
        ("import, two file systems, plain", Some(&other), true, false, None, &[(2, 10, 1, 0), (1, 10, 5, 0)]),
        ("import, two file systems, gzip", Some(&other), true, true, None, &[(2, 0, 1, 3), (1, 10, 5, 0)]),
        ("import, two, the tree goes from the pool's", Some(&other), true, true, Some(1),
            &[(2, 0, 1, 3), (1, 0, 0, 0)]),
        ("import, two, the tree goes from the images'", Some(&other), true, true, Some(2),
            &[(2, 0, 1, 3), (1, 10, 5, 0)]),
        ("import, images unknown", None, true, true, None, &[(1, 10, 5, 0)]),
        ("import, images unknown, the tree goes", None, true, true, Some(1), &[(1, 0, 0, 0)]),
        ("no import", Some(&same), false, false, None, &[(1, 10, 5, 0)]),
        ("no import, no tree goes", Some(&same), false, true, Some(1), &[(1, 10, 5, 0)]),
    ];
    for (what, images, importing, gzip, freed, want) in cases {
        let plan = Plan {
            tree,
            upload: 3,
            gzip: *gzip,
            freed: *freed,
            imported: false,
        };
        let got: Vec<(u64, u64, u64, u64)> = loads(&pool, *images, &plan, *importing)
            .iter()
            .map(|l| match l.spot.dev {
                Dev::Is(d) => (d, l.need.bytes, l.need.entries, l.extra),
                d => panic!("{what}: {d:?}"),
            })
            .collect();
        assert_eq!(got, *want, "{what}");
    }
}

/// T-2085: the upload is plain where incusd's images have room for three
/// trees, else gzip, also where podvm cannot read the free space.
#[test]
fn the_upload_is_gzip_where_the_images_lack_room() {
    use super::gzip::wanted;
    for (free, tree, want) in [
        (None, 1, true),
        (Some(30), 10, false),
        (Some(29), 10, true),
        (Some(0), 0, false),
        (Some(u64::MAX), u64::MAX / 3, false),
        (Some(u64::MAX), u64::MAX, true),
    ] {
        assert_eq!(wanted(free, tree), want, "{free:?} free, a tree of {tree}");
    }
}

#[test]
fn only_a_server_of_the_driver_dir_is_measured() {
    for (storage, want) in [
        ("dir", true),
        (" dir ", true),
        ("dir | dir", true),
        ("dir | zfs", false),
        ("btrfs", false),
        ("", false),
        ("|", false),
    ] {
        assert_eq!(dir_only(storage), want, "{storage:?}");
    }
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("podvm-t2078-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn the_need_counts_each_entry_and_the_bytes_of_each_file() {
    let d = scratch("need");
    std::fs::create_dir_all(d.join("root/a/b")).unwrap();
    std::fs::write(d.join("root/a/f"), vec![0u8; 1000]).unwrap();
    std::fs::write(d.join("root/a/b/g"), vec![0u8; 24]).unwrap();
    std::os::unix::fs::symlink("/nowhere", d.join("root/l")).unwrap();
    let need = need_of(&d.join("root"), 7);
    // a, a/b, a/f, a/b/g, l, and /.podvm with podvm.
    assert_eq!(
        need,
        Need {
            bytes: 1031,
            entries: 7
        }
    );
    // A root that is not there needs only podvm.
    assert_eq!(
        need_of(&d.join("none"), 7),
        Need {
            bytes: 7,
            entries: 2
        }
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_pool_is_one_segment_under_the_incusd_directory() {
    let d = scratch("target");
    std::fs::create_dir_all(d.join("storage-pools/default/containers")).unwrap();
    // A mount point that podvm does not see mounted (route A): no check.
    std::fs::create_dir_all(d.join("storage-pools/unseen")).unwrap();
    std::fs::create_dir_all(d.join("images")).unwrap();
    let t = Target::at(&d, "default").expect("the pool");
    assert_eq!(t.pool.path, d.join("storage-pools/default/containers"));
    assert_eq!(t.images.as_ref().map(|i| i.dev), Some(t.pool.dev));
    let Dev::Is(pool_dev) = t.pool.dev else {
        panic!("statfs gives the device: {:?}", t.pool.dev)
    };
    for bad in ["", ".", "..", "a/b", "../images", "missing", "unseen"] {
        assert!(Target::at(&d, bad).is_none(), "{bad:?}");
    }
    // With room: no refusal, and no line after a fault that is not a
    // refusal by size.
    let small = Plan {
        tree: Need {
            bytes: 1,
            entries: 1,
        },
        ..Plan::default()
    };
    assert_eq!(t.check(&small, true), Ok(Vec::new()));
    assert_eq!(t.after(Some(&small), false, Transport::Other, 64), None);
    assert_eq!(t.after(Some(&small), false, Transport::None, 64), None);
    // A refusal by size gives the free space each time.
    let h = t
        .after(Some(&small), true, Transport::TooLarge, 64 << 20)
        .expect("a line");
    assert!(h.contains("over podvm's bound of 64.0 MiB"), "{h}");
    assert!(
        h.contains("the storage pool `default` (also incusd's images) at"),
        "{h}"
    );
    assert!(!h.contains('\n'), "{h}");
    // A need that no disk holds: the refusal, and the line after any
    // fault of the transport.
    let huge = Plan {
        tree: Need {
            bytes: u64::MAX / 2,
            entries: 1,
        },
        ..Plan::default()
    };
    let e = t.check(&huge, false).expect_err("a refusal");
    assert!(e.contains("the root of the instance needs at least"), "{e}");
    assert!(t.after(Some(&huge), false, Transport::Other, 64).is_some());
    // T-2085: the same need where the import removes the store's tree on
    // the file system of the pool: no refusal, as the root takes its place.
    let freed = Plan {
        freed: Some(pool_dev),
        gzip: true,
        ..huge
    };
    assert!(t.check(&freed, true).is_ok());
    // With no tree (incus had the image), the need is a dash, and only a
    // refusal by size gives the line.
    let h = t
        .after(None, false, Transport::TooLarge, 64 << 20)
        .expect("a line");
    assert!(
        h.contains("needs - (podvm has no tree of the image)"),
        "{h}"
    );
    assert_eq!(t.after(None, false, Transport::Other, 64), None);
    let _ = std::fs::remove_dir_all(&d);
}

/// A stand-in incusd whose wait of a create sends one chunk of 80 MiB, as
/// incusd 6.0 sends the error of tar on a full pool: the class is the
/// lack of space (pod-incus `v0.83.0` gives the envelope with its text
/// cut), and another error envelope is no fault of the transport.
#[test]
fn an_error_of_tar_over_the_bound_is_a_lack_of_space() {
    let sd = crate::test_socket::SocketDir::new("t2078");
    let sock = sd.join("unix.socket");
    let l = UnixListener::bind(&sock).unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            let (m, p, _) = super::tests::read_request(&mut s);
            let (status, body, chunked) = match (m.as_str(), p.as_str()) {
                ("POST", "/1.0/instances") => (
                    202,
                    r#"{"type":"async","operation":"/1.0/operations/c","metadata":{"id":"c"}}"#
                        .to_string(),
                    false,
                ),
                (_, p) if p.starts_with("/1.0/operations/c/wait") => (
                    200,
                    format!(
                        "{:x}\r\n{{\"type\":\"error\",\"error_code\":500,\"error\":\"Unpack failed: tar: ./a: Cannot write: No space left on device ",
                        80 * MIB
                    ),
                    true,
                ),
                _ => (
                    500,
                    r#"{"type":"error","error_code":500,"error":"no room"}"#.to_string(),
                    false,
                ),
            };
            let te = if chunked {
                "Transfer-Encoding: chunked\r\n"
            } else {
                ""
            };
            let head = format!("HTTP/1.1 {status} X\r\n{te}Connection: close\r\n\r\n");
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(body.as_bytes());
            // incusd sends the whole chunk that it declares (108 MB of the
            // stderr of tar, T-2078); pod-incus `v0.83.0` reads up to its
            // bound and keeps the head. The client stops reading there.
            if chunked {
                // The rest of the declared chunk, then its CRLF and the
                // last chunk: a well-formed answer over the bound.
                let head_len = body.find("\r\n").map_or(0, |i| i + 2);
                // The end of the envelope: the last two bytes close the
                // string of the error and the object, as incusd sends it.
                let mut left = (80 * MIB) as usize - (body.len() - head_len) - 2;
                let fill = vec![b'a'; MIB as usize];
                while left > 0 {
                    let n = left.min(fill.len());
                    if s.write_all(&fill[..n]).is_err() {
                        break;
                    }
                    left -= n;
                }
                let _ = s.write_all(b"\"}\r\n0\r\n\r\n");
            }
        }
    });
    let c = pod_incus::Client::new(pod_incus::Settings::new(sock));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let new = pod_incus::NewInstance::container(
        "measure",
        pod_incus::Source::Alias("podvm-x".to_string()),
    );
    let e = rt
        .block_on(c.create_instance(&new))
        .expect_err("the answer is over the bound");
    assert_eq!(transport(&e), Transport::NoSpace, "{e}");
    let e = rt
        .block_on(c.profile("default"))
        .expect_err("an error envelope");
    assert_eq!(transport(&e), Transport::None, "{e}");
}
