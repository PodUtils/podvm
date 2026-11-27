//! TODO/cli.md T-2108 (OR-B42-06, OR-B42-07): the refusals of `cp` and
//! `wait` after the flags exit 1 (the published table); the tar stream of
//! `cp NAME:PATH -` and `cp - NAME:PATH`; `cp` keeps the modification time.

use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct Gone(PathBuf);

impl Drop for Gone {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn podvm_in(base: &Path, args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_podvm"))
        .args(args)
        .current_dir(base)
        // TODO/gate.md T-1859: no detached spare-tree fill writes into the
        // store after the test removed it.
        .env("PODVM_SPARE_TREE", "0")
        .env("HOME", base)
        .env("PODVM_STORE", base.join("store"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("podvm runs");
    // A write to a command that refuses at once can meet a closed pipe.
    let _ = c
        .stdin
        .take()
        .map(|mut s| s.write_all(stdin.unwrap_or(b"")));
    c.wait_with_output().expect("podvm ends")
}

fn podvm(base: &Path, args: &[&str]) -> Output {
    podvm_in(base, args, None)
}

fn ok(base: &Path, args: &[&str]) -> Output {
    let o = podvm(base, args);
    assert!(
        o.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    o
}

/// A store with the image `t2108:1` (`/etc/os-release`, `/etc/sub/deep`, a
/// static payload) and the created container `c1` of it.
fn fixture(tag: &str) -> (PathBuf, Gone, PathBuf) {
    let base = std::env::temp_dir().join(format!("podvm-t2108-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let r = base.join("r");
    std::fs::create_dir_all(r.join("etc/sub")).unwrap();
    std::fs::create_dir_all(r.join("bin")).unwrap();
    std::fs::create_dir_all(r.join("tmp")).unwrap();
    let gone = Gone(base.clone());
    std::fs::write(r.join("etc/os-release"), "x\n").unwrap();
    std::fs::write(r.join("etc/sub/deep"), "d\n").unwrap();
    std::os::unix::fs::symlink("os-release", r.join("etc/lnk")).unwrap();
    std::fs::copy(std::env::current_exe().unwrap(), r.join("bin/hello")).unwrap();
    let mut b = tar::Builder::new(Vec::new());
    b.follow_symlinks(false);
    for d in ["etc", "bin", "tmp"] {
        b.append_dir_all(d, r.join(d)).unwrap();
    }
    std::fs::write(base.join("a.tar"), b.into_inner().unwrap()).unwrap();
    ok(&base, &["import", "a.tar", "t2108:1"]);
    ok(
        &base,
        &["create", "--name", "c1", "t2108:1", "/bin/hello", "--list"],
    );
    let o = ok(&base, &["inspect", "--format", "{{.RootfsPath}}", "c1"]);
    let root = PathBuf::from(String::from_utf8_lossy(&o.stdout).trim());
    (base, gone, root)
}

fn entries(tar_bytes: &[u8]) -> Vec<(String, tar::EntryType)> {
    tar::Archive::new(tar_bytes)
        .entries()
        .expect("a tar")
        .map(|e| {
            let e = e.expect("an entry");
            (
                e.path().unwrap().to_string_lossy().into_owned(),
                e.header().entry_type(),
            )
        })
        .collect()
}

/// A tar with one entry the way the tester wants it, names with `..`
/// included (`set_path` would refuse them).
fn raw(name: &[u8], kind: tar::EntryType, link: &[u8], data: &[u8], mode: u32) -> Vec<u8> {
    let mut h = tar::Header::new_gnu();
    h.as_old_mut().name[..name.len()].copy_from_slice(name);
    h.set_entry_type(kind);
    h.set_mode(mode);
    h.set_mtime(1_000_000_000);
    h.set_size(data.len() as u64);
    if !link.is_empty() {
        h.as_old_mut().linkname[..link.len()].copy_from_slice(link);
    }
    h.set_cksum();
    let mut v = h.as_bytes().to_vec();
    v.extend_from_slice(data);
    v.resize(v.len().div_ceil(512) * 512, 0);
    v.extend_from_slice(&[0; 1024]);
    v
}

/// The exit table (`podvm system info --format '{{json .ExitCodes}}'`):
/// a refusal after the flags parsed is 1. One table for the family: a
/// directory with no `-r`, a FIFO, a special file inside a tree, two named
/// sides, and `wait` of a container that never started.
#[test]
fn a_refusal_of_cp_and_wait_after_the_flags_exits_1() {
    let (base, _gone, _root) = fixture("exit");
    let fifo = base.join("ff");
    let made = Command::new("mkfifo").arg(&fifo).status().expect("mkfifo");
    assert!(made.success());
    std::fs::create_dir_all(base.join("tree")).unwrap();
    let again = Command::new("mkfifo")
        .arg(base.join("tree/pipe"))
        .status()
        .unwrap();
    assert!(again.success());
    let rows: &[(&[&str], &str)] = &[
        (&["cp", "c1:/etc", "out"], "pass -r"),
        (&["cp", "ff", "c1:/tmp/ff"], "special files are refused"),
        (
            &["cp", "-r", "tree", "c1:/tmp/tree"],
            "special files are refused",
        ),
        (&["cp", "c1:/etc/os-release", "c1:/tmp/x"], "two named"),
        (&["cp", "a", "b"], "one of the two paths"),
        (&["wait", "c1"], "never started"),
    ];
    for (args, words) in rows {
        let o = podvm(&base, args);
        let err = String::from_utf8_lossy(&o.stderr);
        assert_eq!(o.status.code(), Some(1), "{args:?}: {err}");
        assert!(err.contains(words), "{args:?}: {err}");
    }
    // A flag that does not parse stays 125.
    let o = podvm(&base, &["cp", "--no-such-flag", "c1:/etc", "x"]);
    assert_eq!(o.status.code(), Some(125));
    // Nothing was made by a refusal.
    assert!(!base.join("out").exists());
}

/// docker's names of the stream going out: the last name of the path, or
/// `./` where the path ends with `/.`; a link goes as a link, a directory
/// with its tree, and a refusal comes before the first byte.
#[test]
fn cp_to_dash_writes_the_tar_that_docker_writes() {
    let (base, _gone, _root) = fixture("out");
    let one = ok(&base, &["cp", "c1:/etc/os-release", "-"]).stdout;
    assert_eq!(
        entries(&one),
        vec![("os-release".to_string(), tar::EntryType::Regular)]
    );
    let mut body = String::new();
    tar::Archive::new(&one[..])
        .entries()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .read_to_string(&mut body)
        .unwrap();
    assert_eq!(body, "x\n");

    let dir = ok(&base, &["cp", "c1:/etc", "-"]).stdout;
    // The completion layer adds files of its own to /etc: the ones of the
    // image are there, in the order of their names, parents first.
    let names: Vec<String> = entries(&dir).into_iter().map(|e| e.0).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
    for want in [
        "etc/",
        "etc/lnk",
        "etc/os-release",
        "etc/sub/",
        "etc/sub/deep",
    ] {
        assert!(names.iter().any(|n| n == want), "{want}: {names:?}");
    }
    let lnk = entries(&dir).into_iter().find(|e| e.0 == "etc/lnk");
    assert_eq!(lnk.map(|e| e.1), Some(tar::EntryType::Symlink));

    let contents = ok(&base, &["cp", "c1:/etc/.", "-"]).stdout;
    let names: Vec<String> = entries(&contents).into_iter().map(|e| e.0).collect();
    assert_eq!(names.first().map(String::as_str), Some("./"));
    assert!(names.iter().any(|n| n.ends_with("os-release")));
    assert!(!names.iter().any(|n| n.starts_with("etc")), "{names:?}");

    // An image is a source too; a missing path refuses with nothing on stdout.
    let img = ok(&base, &["cp", "t2108:1:/etc/os-release", "-"]).stdout;
    assert_eq!(entries(&img).len(), 1);
    let o = podvm(&base, &["cp", "c1:/etc/missing", "-"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty());
    // A path out of the root refuses by name, as the files copy does.
    let o = podvm(&base, &["cp", "c1:/../../etc/passwd", "-"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty());
}

/// The stream coming in: a clean tar lands with its modes and times, and
/// each class of hostile or damaged stream is refused with exit 1 and the
/// name of the entry, and makes nothing out of the root.
#[test]
fn cp_from_dash_takes_a_clean_tar_and_refuses_each_hostile_one() {
    let (base, _gone, root) = fixture("in");
    // The round trip of the whole of /etc, into /tmp/in.
    let out = ok(&base, &["cp", "c1:/etc", "-"]).stdout;
    std::fs::create_dir_all(root.join("tmp/in")).unwrap();
    let o = podvm_in(&base, &["cp", "-", "c1:/tmp/in"], Some(&out));
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert_eq!(
        std::fs::read_to_string(root.join("tmp/in/etc/sub/deep")).unwrap(),
        "d\n"
    );
    assert_eq!(
        std::fs::read_link(root.join("tmp/in/etc/lnk")).unwrap(),
        Path::new("os-release")
    );

    // One archive with a file of mode 640 and a time of 2001.
    let t = raw(b"f640", tar::EntryType::Regular, b"", b"data", 0o640);
    let o = podvm_in(&base, &["cp", "-", "c1:/tmp"], Some(&t));
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let m = std::fs::metadata(root.join("tmp/f640")).unwrap();
    assert_eq!(m.permissions().mode() & 0o7777, 0o640);
    assert_eq!(m.mtime(), 1_000_000_000);

    // Each class: a name above the destination, a link on the way, a
    // special file, a file onto a directory, a damaged stream, an empty one.
    // A link to a directory of the host, outside the root, and a file under it.
    std::fs::create_dir_all(base.join("outside")).unwrap();
    let outside = base.join("outside");
    let mut link_then_file = raw(
        b"l",
        tar::EntryType::Symlink,
        outside.to_str().unwrap().as_bytes(),
        b"",
        0o777,
    );
    link_then_file.truncate(512);
    link_then_file.extend(raw(
        b"l/passwd",
        tar::EntryType::Regular,
        b"",
        b"pwn",
        0o644,
    ));
    let whole = raw(b"whole", tar::EntryType::Regular, b"", &[7u8; 2000], 0o644);
    let rows: &[(&str, Vec<u8>, &str)] = &[
        (
            "dotdot",
            raw(b"../evil", tar::EntryType::Regular, b"", b"pwn", 0o644),
            "above the destination",
        ),
        (
            "link on the way",
            link_then_file,
            "writing through one is refused",
        ),
        (
            "fifo",
            raw(b"pipe", tar::EntryType::Fifo, b"", b"", 0o644),
            "special files",
        ),
        (
            "device",
            raw(b"dev", tar::EntryType::Char, b"", b"", 0o644),
            "special files",
        ),
        (
            "file onto a directory",
            raw(b"in", tar::EntryType::Regular, b"", b"x", 0o644),
            "a directory is there",
        ),
        (
            "hard link to nothing",
            raw(b"h", tar::EntryType::Link, b"nothing", b"", 0o644),
            "not a file",
        ),
        ("cut", whole[..700].to_vec(), "damaged or cut"),
        ("garbage", vec![b'x'; 1024], "damaged or cut"),
        ("empty", Vec::new(), "no tar stream"),
    ];
    for (what, bytes, words) in rows {
        let o = podvm_in(&base, &["cp", "-", "c1:/tmp"], Some(bytes));
        let err = String::from_utf8_lossy(&o.stderr);
        assert_eq!(o.status.code(), Some(1), "{what}: {err}");
        assert!(err.contains(words), "{what}: {err}");
    }
    // The planted file did not reach the host through the link `l`.
    assert!(!outside.join("planted").exists());
    assert!(!root.join("evil").exists() && !base.join("evil").exists());
    // The destination must be a directory that exists.
    for dest in ["c1:/etc/os-release", "c1:/nowhere"] {
        let o = podvm_in(&base, &["cp", "-", dest], Some(&t));
        assert_eq!(o.status.code(), Some(1), "{dest}");
    }
    // An image takes no copy in.
    let o = podvm_in(&base, &["cp", "-", "t2108:1:/tmp"], Some(&t));
    assert_eq!(o.status.code(), Some(1));
}

/// OR-B42-07: the modification time is kept, for a file, a directory and a
/// link, into the container and out of it, and never moves a target.
#[test]
fn cp_keeps_the_modification_time_both_ways() {
    let (base, _gone, root) = fixture("time");
    let old = |p: &Path, secs: i64| {
        let s = Command::new("touch")
            .args(["-h", "-d", &format!("@{secs}")])
            .arg(p)
            .status()
            .unwrap();
        assert!(s.success());
    };
    let t = base.join("tree");
    std::fs::create_dir_all(t.join("sub")).unwrap();
    std::fs::write(t.join("f"), "f\n").unwrap();
    std::fs::write(t.join("sub/g"), "g\n").unwrap();
    std::os::unix::fs::symlink("f", t.join("lnk")).unwrap();
    old(&t.join("f"), 1_577_934_245);
    old(&t.join("sub/g"), 1_577_934_246);
    old(&t.join("sub"), 1_577_934_247);
    old(&t.join("lnk"), 1_557_126_489);
    old(&t, 1_577_934_248);
    let secs = |p: &Path| std::fs::symlink_metadata(p).unwrap().mtime();

    ok(&base, &["cp", "-r", "tree", "c1:/tmp/tree"]);
    let inside = root.join("tmp/tree");
    for (p, want) in [
        ("f", 1_577_934_245),
        ("sub/g", 1_577_934_246),
        ("sub", 1_577_934_247),
        ("lnk", 1_557_126_489),
        ("", 1_577_934_248),
    ] {
        assert_eq!(secs(&inside.join(p)), want, "in: {p:?}");
    }
    ok(&base, &["cp", "-r", "c1:/tmp/tree", "back"]);
    for (p, want) in [
        ("f", 1_577_934_245),
        ("sub/g", 1_577_934_246),
        ("sub", 1_577_934_247),
        ("lnk", 1_557_126_489),
        ("", 1_577_934_248),
    ] {
        assert_eq!(secs(&base.join("back").join(p)), want, "out: {p:?}");
    }
    // One file, both ways.
    ok(&base, &["cp", "tree/f", "c1:/tmp/one"]);
    assert_eq!(secs(&root.join("tmp/one")), 1_577_934_245);
    ok(&base, &["cp", "c1:/tmp/one", "one.out"]);
    assert_eq!(secs(&base.join("one.out")), 1_577_934_245);
}
