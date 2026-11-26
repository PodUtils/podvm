//! TODO/machine.md T-2103: the checks of `volume rm` and the record of the
//! maps.

use super::*;
use crate::incus::volmap::{self, Map, Record};

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "podvm-volrm-{tag}-{}",
        crate::machine::guest::init::nonce().unwrap()
    ));
    std::fs::create_dir(&d).unwrap();
    std::fs::canonicalize(&d).unwrap()
}

/// One table: each path that `volume rm` refuses, with its words, and the
/// path that it takes.
#[test]
fn the_removal_stays_in_the_callers_own_directory() {
    use std::os::unix::fs::MetadataExt;
    let d = tmp("check");
    let host = d.join("cache");
    std::fs::create_dir(&host).unwrap();
    std::fs::write(d.join("file"), b"x").unwrap();
    std::os::unix::fs::symlink(&host, d.join("link")).unwrap();
    let me = std::fs::metadata(&host).unwrap().uid();
    let s = |p: &Path| p.to_str().unwrap().to_string();
    type Case = (String, u32, Option<PathBuf>, Result<PathBuf, &'static str>);
    let cases: Vec<Case> = vec![
        (s(&host), me, None, Ok(host.clone())),
        (s(&d.join("link")), me, None, Err("is a link")),
        (s(&d.join("file")), me, None, Err("not a directory")),
        (s(&d.join("gone")), me, None, Err("No such file")),
        ("/".into(), 0, None, Err("root of the host")),
        (s(&host), me, Some(host.clone()), Err("home directory")),
        (s(&d), me, Some(host.join("x")), Err("home directory")),
        (s(&host), me + 1, None, Err("belongs to uid")),
    ];
    for (arg, euid, home, want) in cases {
        match (check_host(&arg, euid, home.as_deref()), want) {
            (Ok(p), Ok(w)) => assert_eq!(p, w, "{arg}"),
            (Err(e), Err(w)) => assert!(e.contains(w), "{arg}: {e}"),
            (got, want) => panic!("{arg}: {got:?}, want {want:?}"),
        }
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// A mount point at or under HOST refuses the removal; one beside it or
/// above it does not, and the octal escapes of the table are read.
#[test]
fn a_mount_under_the_directory_refuses_the_removal() {
    let info = "\
22 1 0:21 / / rw - ext4 /dev/vda rw
23 22 0:22 / /data/cache\\040dir/sub rw - tmpfs t rw
24 22 0:23 / /data/cache\\040dirx rw - tmpfs t rw
";
    let host = Path::new("/data/cache dir");
    assert_eq!(
        mounts_under(info, host),
        vec![PathBuf::from("/data/cache dir/sub")]
    );
    assert!(mounts_under(info, Path::new("/data/other")).is_empty());
    assert_eq!(mounts_under(info, Path::new("/data")).len(), 2);
}

/// The arguments of the verb: the tier is required and is incus.
#[test]
fn volume_rm_takes_the_incus_tier_and_a_path() {
    let a = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(
        parse(&a(&["--podvm-tier=incus", "/a", "--", "-b"])).unwrap(),
        Some(vec!["/a".to_string(), "-b".to_string()])
    );
    assert_eq!(
        parse(&a(&["--podvm-tier", "incus", "/a"])).unwrap(),
        Some(vec!["/a".to_string()])
    );
    assert_eq!(parse(&a(&["--help"])).unwrap(), None);
    for bad in [
        &["/a"][..],
        &["--podvm-tier=machine", "/a"],
        &["--podvm-tier=incus"],
    ] {
        assert_eq!(parse(&a(bad)), Err(EXIT_FLAG_ERROR), "{bad:?}");
    }
}

/// The map of incusd's `volatile.idmap.next`: one range for the uids and
/// the same for the gids, from id 0; each other form is refused.
#[test]
fn the_map_of_an_instance_is_one_range_from_id_0() {
    let e = |u: bool, g: bool, h: u64, n: u64, r: u64| {
        format!(r#"{{"Isuid":{u},"Isgid":{g},"Hostid":{h},"Nsid":{n},"Maprange":{r}}}"#)
    };
    let two = format!(
        "[{},{}]",
        e(true, false, 1065536, 0, 65536),
        e(false, true, 1065536, 0, 65536)
    );
    let cases = [
        (
            two,
            Ok(Map {
                base: 1065536,
                size: 65536,
            }),
        ),
        (
            format!("[{}]", e(true, true, 1000000, 0, 1000000000)),
            Ok(Map {
                base: 1000000,
                size: 1000000000,
            }),
        ),
        (
            format!("[{},{}]", e(true, false, 1, 0, 9), e(false, true, 2, 0, 9)),
            Err("differ"),
        ),
        (
            format!("[{}]", e(true, false, 1, 0, 9)),
            Err("no uid range or no gid"),
        ),
        (format!("[{}]", e(true, true, 1, 5, 9)), Err("from id 0")),
        (
            format!("[{},{}]", e(true, true, 1, 0, 9), e(true, false, 1, 0, 9)),
            Err("two entries"),
        ),
        ("{}".to_string(), Err("not a list")),
        ("[".to_string(), Err("not JSON")),
    ];
    for (text, want) in cases {
        match (volmap::of_json(&text), want) {
            (Ok(m), Ok(w)) => assert_eq!(m, w, "{text}"),
            (Err(e), Err(w)) => assert!(e.contains(w), "{text}: {e}"),
            (got, want) => panic!("{text}: {got:?}, want {want:?}"),
        }
    }
}

/// The record: its text and its parse agree, a damaged one is refused by
/// its line, and `related` finds the records of a directory, of one under
/// it and of one above it, not of one beside it.
#[test]
fn the_record_of_the_maps_reads_back() {
    let r = Record {
        host: PathBuf::from("/c/d e"),
        maps: vec![Map { base: 1, size: 2 }, Map { base: 3, size: 4 }],
    };
    let t = volmap::text(&r).unwrap();
    assert_eq!(volmap::parse(&t).unwrap(), r);
    for (bad, why) in [
        ("v2\nhost /a\n", "v1"),
        ("v1\nhost a\n", "host /PATH"),
        ("v1\nhost /a\nmap 1\n", "map BASE SIZE"),
        ("v1\nhost /a\nmap 1 0\n", "map BASE SIZE"),
    ] {
        assert!(volmap::parse(bad).unwrap_err().contains(why), "{bad}");
    }
    let nl = Record {
        host: PathBuf::from("/a\nb"),
        maps: vec![],
    };
    assert!(volmap::text(&nl).is_err());
    let d = tmp("record");
    let store = podvm_image::Store::open(d.join("store")).unwrap();
    let m = Map {
        base: 1065536,
        size: 65536,
    };
    for h in ["/w/cache", "/w/cache/sub", "/w", "/w/cachex"] {
        volmap::add(&store, Path::new(h), m).unwrap();
    }
    volmap::add(&store, Path::new("/w/cache"), m).unwrap();
    volmap::add(&store, Path::new("/w/cache"), Map { base: 7, size: 8 }).unwrap();
    let rel = volmap::related(store.root(), Path::new("/w/cache")).unwrap();
    let mut hosts: Vec<_> = rel.iter().map(|(_, r)| r.host.clone()).collect();
    hosts.sort();
    assert_eq!(hosts, ["/w", "/w/cache", "/w/cache/sub"].map(PathBuf::from));
    assert_eq!(volmap::maps(&rel), vec![Map { base: 7, size: 8 }, m]);
    volmap::forget(&store, Path::new("/w/cache"), &rel);
    let left = volmap::related(store.root(), Path::new("/w/cache")).unwrap();
    assert_eq!(left.len(), 1, "the record above stays");
    let _ = std::fs::remove_dir_all(&d);
}

/// The guest pass removes each entry it can, keeps a link as a link (the
/// target outside stays), and says what stays.
#[test]
fn the_guest_pass_removes_each_entry_and_follows_no_link() {
    let d = tmp("guest");
    let vol = d.join("vol");
    let outside = d.join("outside");
    std::fs::create_dir_all(vol.join("a/b")).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("keep"), b"k").unwrap();
    std::fs::write(vol.join("a/b/f"), b"x").unwrap();
    std::os::unix::fs::symlink(&outside, vol.join("ln")).unwrap();
    std::os::unix::fs::symlink(&outside, vol.join("a/ln")).unwrap();
    assert_eq!(guest_main(&[vol.to_str().unwrap().to_string()]), 0);
    assert_eq!(std::fs::read_dir(&vol).unwrap().count(), 0);
    assert!(outside.join("keep").exists());
    assert_eq!(
        guest_main(&[d.join("gone").to_str().unwrap().to_string()]),
        2
    );
    let _ = std::fs::remove_dir_all(&d);
}
