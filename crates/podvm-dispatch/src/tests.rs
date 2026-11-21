//! TODO/enter.md T-1652: the parts of the handler that need no payload.

use crate::elf;
use crate::path::fd_link;

#[test]
fn the_proc_link_of_a_descriptor_and_of_the_cwd() {
    let mut b = [0u8; 32];
    assert_eq!(fd_link(1003, &mut b), b"/proc/self/fd/1003\0");
    assert_eq!(fd_link(0, &mut b), b"/proc/self/fd/0\0");
    assert_eq!(fd_link(crate::sys::AT_FDCWD, &mut b), b"/proc/self/cwd\0");
}

#[test]
fn a_shebang_gives_the_interpreter_and_one_argument() {
    assert_eq!(
        elf::shebang(b"#!/bin/sh\n"),
        Some((b"/bin/sh".to_vec(), None))
    );
    assert_eq!(
        elf::shebang(b"#! /usr/bin/env  python3 \r\nx"),
        Some((b"/usr/bin/env".to_vec(), Some(b"python3".to_vec())))
    );
    assert_eq!(elf::shebang(b"#!\n"), None);
    assert_eq!(elf::shebang(b"\x7fELF"), None);
}

#[test]
fn the_check_takes_a_static_program_and_refuses_a_dynamic_one() {
    let me = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    let read = |off: u64, out: &mut [u8]| {
        let s = me
            .get(off as usize..off as usize + out.len())
            .unwrap_or(&[]);
        out[..s.len()].copy_from_slice(s);
        s.len() as i64
    };
    assert_eq!(
        elf::static_x86_64(&me[..64], read),
        Ok(()),
        "this test binary is static"
    );
    let sh = std::fs::read("/bin/sh").unwrap_or_default();
    if sh.len() > 64 {
        let read = |off: u64, out: &mut [u8]| {
            let s = sh
                .get(off as usize..off as usize + out.len())
                .unwrap_or(&[]);
            out[..s.len()].copy_from_slice(s);
            s.len() as i64
        };
        assert_eq!(
            elf::static_x86_64(&sh[..64], read),
            Err("a dynamic program (it names an interpreter)")
        );
    }
    assert!(elf::static_x86_64(b"#!/bin/sh\n", |_, _| 0).is_err());
}

/// T-1657: the pure parts of the syscall tier.
#[test]
fn the_tier_counts_growth_and_names_its_release() {
    use crate::tier::*;
    assert!(under_tmp(b"/tmp") && under_tmp(b"/tmp/a/b"));
    assert!(!under_tmp(b"/tmpx") && !under_tmp(b"/var/tmp"));
    assert_eq!(growth(0, 10, 0), 10);
    assert_eq!(
        growth(5, 10, 20),
        0,
        "an overwrite inside the file grows nothing"
    );
    assert_eq!(growth(15, 10, 20), 5);
    assert!(fits(990, 10, 1000) && !fits(991, 10, 1000));
    assert!(fits(5000, 10, 0), "no cap");
    assert!(fits(5000, -10, 1000), "a shrink always fits");
    let mut u = [b'z'; 390];
    fill_uname(&mut u);
    assert_eq!(&u[130..130 + RELEASE.len()], RELEASE);
    assert_eq!(u[130 + RELEASE.len()], 0);
    assert_eq!(&u[0..5], b"zzzzz", "the other fields stay the host's");
    // TODO/enter.md T-1908: `--hostname` writes the nodename field only.
    let mut u = [b'z'; 390];
    assert!(crate::uts::set(b"unit1"));
    crate::uts::patch(&mut u);
    assert_eq!(&u[65..71], b"unit1\0");
    assert_eq!(u[129], 0);
    assert_eq!(&u[130..135], b"zzzzz", "the release stays the host's");
    assert!(!crate::uts::set(&[b'a'; 65]), "a name of 65 bytes");
}

/// TODO/enter.md T-1935, OS-01: `close_range` keeps the root's
/// descriptor, the count of `/tmp` and each volume's.
#[test]
fn close_range_goes_around_the_kept_descriptors() {
    use crate::handler::range_gaps;
    let m = u32::MAX as i64;
    for (lo, hi, keep, want) in [
        (3, 10, &[-1, -1][..], None),
        (3, 10, &[1000, 1001][..], None),
        (3, m, &[1000, 1001][..], Some(vec![(3, 999), (1002, m)])),
        (
            3,
            2000,
            &[1001, 1000][..],
            Some(vec![(3, 999), (1002, 2000)]),
        ),
        (1000, 1000, &[1000, -1][..], Some(vec![])),
        (3, 1500, &[1000, -1][..], Some(vec![(3, 999), (1001, 1500)])),
        (
            3,
            1500,
            &[5, 1000][..],
            Some(vec![(3, 4), (6, 999), (1001, 1500)]),
        ),
        // A root, a count and two volumes; a descriptor twice is one.
        (
            3,
            m,
            &[1000, 1001, 1002, 1003, 1002][..],
            Some(vec![(3, 999), (1004, m)]),
        ),
        (
            3,
            m,
            &[1000, -1, 1005, 1010][..],
            Some(vec![(3, 999), (1001, 1004), (1006, 1009), (1011, m)]),
        ),
    ] {
        let mut out = [(0, 0); 19];
        let got = range_gaps(lo, hi, keep, &mut out).map(|n| out[..n].to_vec());
        assert_eq!(got, want, "{lo} {hi} {keep:?}");
    }
}

/// TODO/enter.md T-2015: the handler's reader of a `#!` line (the
/// interposer's `shebang`, with the kernel's bound of 256 bytes) and the
/// arguments that the kernel gives the last interpreter of a chain.
#[test]
fn the_reader_of_a_shebang_line_and_the_lead_of_a_chain() {
    use crate::parse::{shebang, HEAD};
    use crate::script::{lead, Level, DEPTH};
    let long = [b"#!/bin/sh ".as_slice(), &[b'a'; 300]].concat();
    // (head, whole, interpreter, argument)
    for (head, whole, want) in [
        (&b"#!/bin/sh\n"[..], true, Some((&b"/bin/sh"[..], None))),
        (
            b"#! \t/usr/bin/env  node -x \t\nrest",
            true,
            Some((b"/usr/bin/env", Some(&b"node -x"[..]))),
        ),
        (b"#!/bin/sh", true, Some((b"/bin/sh", None))),
        (b"#!/bin/sh", false, None),
        (b"#!  \n", true, None),
        (b"\x7fELF", true, None),
        (
            &long[..HEAD],
            false,
            Some((b"/bin/sh", Some(&long[10..HEAD]))),
        ),
    ] {
        let got = shebang(head, whole).map(|s| {
            (
                &head[s.interp.0..s.interp.1],
                s.arg.map(|(a, b)| &head[a..b]),
            )
        });
        assert_eq!(got, want, "{:?}", String::from_utf8_lossy(head));
    }
    let heads = b"/ld A /i1 B /i2";
    let lv = |i: (usize, usize), a: Option<(usize, usize)>| Level { interp: i, arg: a };
    // (levels from the script down, the arguments before `file0`)
    for (levels, want) in [
        (vec![lv((0, 3), None)], vec![]),
        (vec![lv((0, 3), Some((4, 5)))], vec![&b"A"[..]]),
        (
            vec![lv((6, 9), Some((10, 11))), lv((0, 3), Some((4, 5)))],
            vec![&b"A"[..], b"/i1", b"B"],
        ),
        (
            vec![
                lv((12, 15), None),
                lv((6, 9), Some((10, 11))),
                lv((0, 3), None),
            ],
            vec![&b"/i1"[..], b"B", b"/i2"],
        ),
    ] {
        let mut out: [&[u8]; 2 * DEPTH] = [&[]; 2 * DEPTH];
        let k = lead(heads, &levels, &mut out);
        assert_eq!(out[..k].to_vec(), want);
    }
}

/// The interposer's copy of `pod_confine::loader`, which it keeps as it is
/// (the interposer takes no crate with std).
#[rustfmt::skip]
#[path = "../../podvm-interpose/src/exec/ldso.rs"]
mod interpose_loader;

/// TODO/deps.md T-2050 (podcrates `v0.80.0`, T-998 of podcrates): the copy
/// of the interposer gives the answers of `pod_confine::loader`, which the
/// handler of the syscall tier and `pod-enter` take: the names of a loader,
/// and the programs that a check sees for each reading of the options.
#[test]
fn the_loader_rule_of_the_interposer_is_the_crates() {
    use pod_confine::loader;
    for name in [
        &b"/lib64/ld-linux-x86-64.so.2"[..],
        b"/lib/ld-musl-x86_64.so.1",
        b"/lib/ld-2.28.so",
        b"/lib64/ld64.so.2",
        b"/usr/lib/ld.so.1",
        b"/lib/libc.musl-x86_64.so.1",
        b"/usr/lib/musl/lib/libc.so",
        b"/lib/x86_64-linux-gnu/libc.so.6",
        b"/usr/bin/ld",
        b"/usr/bin/ldd",
        b"ld.so",
        b"",
    ] {
        let n = String::from_utf8_lossy(name);
        assert_eq!(
            interpose_loader::is_loader(name),
            loader::is_loader(name),
            "{n}"
        );
    }
    assert_eq!(interpose_loader::VALUED, loader::VALUED);
    assert_eq!(interpose_loader::FLAGS, loader::FLAGS);
    let rows: [&[&str]; 12] = [
        &["/tmp/p", "a"],
        &["--library-path", "/tmp", "/tmp/p"],
        &["--inhibit-cache", "--argv0", "x", "./p"],
        &["--list", "/tmp/p"],
        &["--library-path=/l", "/tmp/p"],
        &["--help"],
        &[],
        &["--library-path", "/tmp", "p"],
        &["--", "/tmp/p"],
        &["--unknown", "/tmp/p"],
        &["--argv0=x", "/tmp/p"],
        &["-x"],
    ];
    for args in rows {
        let bytes = || args.iter().map(|x| x.as_bytes());
        let (mut seen_copy, mut seen_krate) = (Vec::new(), Vec::new());
        let copy = interpose_loader::each_program(bytes(), |p| {
            seen_copy.push(p.to_vec());
            p != b"/tmp/p"
        });
        let krate = loader::each_program(bytes(), |p| {
            seen_krate.push(p.to_vec());
            p != b"/tmp/p"
        });
        assert_eq!((copy, seen_copy), (krate, seen_krate), "{args:?}");
    }
}
