//! TODO/machine.md T-1718: the HTTP parse, the image form and the command
//! form; one `run` against a stand-in incusd is in `run_tests`.

use std::io::Read;
use std::path::PathBuf;

use super::image::{alias, intended, unified_tarball, IMAGE_FORMAT};
use super::run::env_map;

pub(super) fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("podvm-t1718-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn the_alias_names_the_digest_and_the_format() {
    let me = "fedcba9876543210ffff";
    let a = alias("sha256:0123456789abcdef0123", me);
    assert_eq!(
        a,
        format!("podvm-0123456789abcdef-fedcba9876543210-v{IMAGE_FORMAT}")
    );
    assert_ne!(a, alias("sha256:0123456789abcdee0123", me));
    // TODO/machine.md T-1722: another podvm build is another image.
    assert_ne!(a, alias("sha256:0123456789abcdef0123", "0000000000000000"));
}

#[test]
fn the_command_and_the_environment_take_the_incus_form() {
    // TODO/machine.md T-2080: podvm in the image starts the argv, apart and
    // unquoted, in the working directory of the run.
    let argv = vec!["cat".to_string(), "a b".to_string()];
    let with = super::guest_exec::wrap(&argv, "/w");
    assert_eq!(with[0], "/.podvm/podvm");
    assert_eq!(
        &with[5..],
        argv.as_slice(),
        "the argv stays apart, unquoted"
    );
    let env = env_map(&["A=1".into(), "B=x=y".into(), "NOEQ".into()]);
    let want: std::collections::BTreeMap<String, String> = [("A", "1"), ("B", "x=y")]
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .into();
    assert_eq!(env, want);
}

#[test]
fn the_sidecar_gives_the_intended_ids_and_a_later_line_wins() {
    let side = concat!(
        r#"{"path":"etc/shadow","uid":0,"gid":0,"mode":"0600","applied":{"uid":5,"gid":5}}"#,
        "\n",
        r#"{"path":"etc/shadow","uid":0,"gid":42,"mode":"0640","applied":{"uid":5,"gid":5},"reason":"x"}"#,
        "\nnot json\n",
    );
    let m = intended(side);
    assert_eq!(m.len(), 1);
    assert_eq!(m["etc/shadow"], (0, 42, 0o640));
}

#[test]
fn the_unified_tarball_carries_the_metadata_and_the_intended_owners() {
    let d = scratch("tar");
    std::fs::create_dir_all(d.join("img/rootfs/bin")).unwrap();
    std::fs::create_dir_all(d.join("img/rootfs/etc")).unwrap();
    std::fs::write(d.join("img/rootfs/bin/sh"), b"#!sh").unwrap();
    std::fs::write(d.join("img/rootfs/etc/shadow"), b"root:*").unwrap();
    std::os::unix::fs::symlink("sh", d.join("img/rootfs/bin/ash")).unwrap();
    std::fs::write(
        d.join("img/.meta.jsonl"),
        r#"{"path":"etc/shadow","uid":0,"gid":42,"mode":"0640","applied":{"uid":5,"gid":5}}"#,
    )
    .unwrap();
    let out = d.join("img.tar");
    unified_tarball(&d.join("img/rootfs"), &out, b"podvm-bytes").unwrap();
    let mut a = tar::Archive::new(std::fs::File::open(&out).unwrap());
    let mut seen = Vec::new();
    for e in a.entries().unwrap() {
        let mut e = e.unwrap();
        let p = e.path().unwrap().to_string_lossy().into_owned();
        let h = e.header();
        let ids = (h.uid().unwrap(), h.gid().unwrap(), h.mode().unwrap());
        match p.as_str() {
            "rootfs/etc/shadow" => assert_eq!(ids, (0, 42, 0o640), "the intended owner"),
            "rootfs/bin/ash" => assert!(h.entry_type().is_symlink(), "a link stays a link"),
            "rootfs/.podvm/podvm" => {
                assert_eq!(ids, (0, 0, 0o755), "podvm is the init (T-1722)");
                let mut b = Vec::new();
                e.read_to_end(&mut b).unwrap();
                assert_eq!(b, b"podvm-bytes");
            }
            "metadata.yaml" => {
                let mut t = String::new();
                e.read_to_string(&mut t).unwrap();
                assert!(
                    !t.contains("creation_date: 0\n"),
                    "incus refuses a date of 0: {t}"
                );
            }
            _ => assert_eq!((ids.0, ids.1), (0, 0), "{p}: no record gives the owner 0"),
        }
        seen.push(p);
    }
    for want in [
        "metadata.yaml",
        "rootfs/bin",
        "rootfs/bin/sh",
        "rootfs/bin/ash",
        "rootfs/etc/shadow",
    ] {
        assert!(seen.iter().any(|s| s == want), "{want} in {seen:?}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

pub(super) fn read_request(s: &mut std::os::unix::net::UnixStream) -> (String, String, Vec<u8>) {
    let (_, m, p, b) = read_request_head(s);
    (m, p, b)
}

/// [`read_request`] with the head of the request (TODO/machine.md T-2080:
/// the key of a websocket).
pub(super) fn read_request_head(
    s: &mut std::os::unix::net::UnixStream,
) -> (String, String, String, Vec<u8>) {
    let mut raw = Vec::new();
    let mut b = [0u8; 65536];
    let end = loop {
        let n = s.read(&mut b).unwrap();
        raw.extend_from_slice(&b[..n]);
        if let Some(p) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            break p;
        }
        assert!(n > 0, "the client ended inside the headers");
    };
    let head = String::from_utf8_lossy(&raw[..end]).into_owned();
    let len = head
        .lines()
        .find_map(|l| l.strip_prefix("Content-Length: "))
        .map(|v| v.trim().parse::<usize>().unwrap())
        .unwrap_or(0);
    if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        while !raw[end + 4..].ends_with(b"0\r\n\r\n") {
            let n = s.read(&mut b).unwrap();
            assert!(n > 0, "the client ended inside a chunked body");
            raw.extend_from_slice(&b[..n]);
        }
        let mut body = Vec::new();
        let mut rest = &raw[end + 4..];
        loop {
            let eol = rest.windows(2).position(|w| w == b"\r\n").unwrap();
            let size = usize::from_str_radix(std::str::from_utf8(&rest[..eol]).unwrap().trim(), 16)
                .unwrap();
            rest = &rest[eol + 2..];
            if size == 0 {
                break;
            }
            body.extend_from_slice(&rest[..size]);
            rest = &rest[size + 2..];
        }
        let mut first = head.split(' ');
        let method = first.next().unwrap().to_string();
        let path = first
            .next()
            .unwrap()
            .replace("%2F", "/")
            .replace("%2f", "/");
        return (head, method, path, body);
    }
    while raw.len() < end + 4 + len {
        let n = s.read(&mut b).unwrap();
        assert!(n > 0, "the client ended inside the body");
        raw.extend_from_slice(&b[..n]);
    }
    let mut first = head.split(' ');
    let method = first.next().unwrap().to_string();
    // A query of the crate escapes the slashes of a path.
    let path = first
        .next()
        .unwrap()
        .replace("%2F", "/")
        .replace("%2f", "/");
    let body = raw[end + 4..end + 4 + len].to_vec();
    (head, method, path, body)
}

/// TODO/machine.md T-1722: the flag of the tier on a verb of an instance.
#[test]
fn the_tier_flag_selects_the_incus_verbs_and_leaves_the_rest() {
    use super::verbs::{valid_name, wants};
    let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(wants(&a(&["-a", "--podvm-tier=incus"])), Some(a(&["-a"])));
    assert_eq!(
        wants(&a(&["--podvm-tier", "incus", "web"])),
        Some(a(&["web"]))
    );
    assert_eq!(wants(&a(&["-a"])), None);
    assert_eq!(wants(&a(&["--podvm-tier=machine"])), None);
    assert!(valid_name("web-1"));
    assert!(!valid_name("1web"));
    assert!(!valid_name("web_1"));
    assert!(!valid_name(&"a".repeat(64)));
}

/// The command file of an instance that stays reads back as it was written.
#[test]
fn the_command_file_reads_back() {
    let c = super::Cmd {
        argv: vec!["sh".into(), "-c".into(), "echo 'a b'".into()],
        env: vec!["A=1".into()],
        cwd: "/w".into(),
    };
    assert_eq!(super::Cmd::parse(&c.to_json()).unwrap(), c);
}

/// TODO/cli.md T-1811: another tier than incus on a lifecycle verb is a
/// usage refusal by name, in both spellings.
#[test]
fn eq_form_of_another_tier_on_a_lifecycle_verb_is_refused_by_name() {
    let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        crate::incus::other_tier(&v(&["--podvm-tier=machine", "x"])),
        Some("machine".into())
    );
    assert_eq!(
        crate::incus::other_tier(&v(&["--podvm-tier", "chroot", "x"])),
        Some("chroot".into())
    );
    assert_eq!(
        crate::incus::other_tier(&v(&["--podvm-tier=incus", "x"])),
        None
    );
    assert_eq!(
        crate::incus::intercept(Some("start"), &v(&["--podvm-tier=machine", "nosuch"])),
        Some(podvm_image::error::EXIT_FLAG_ERROR)
    );
    assert_eq!(
        crate::incus::intercept(Some("run"), &v(&["--podvm-tier=machine"])),
        None
    );
}

/// TODO/machine.md T-1966: `--podvm-engine` gives the instance the keys of
/// a nested engine, and no key without it.
#[test]
fn engine_gives_the_instance_nesting_and_the_two_intercepts() {
    assert!(super::run::engine_keys(false).is_empty());
    let k: Vec<&str> = super::run::engine_keys(true)
        .iter()
        .map(|(k, _)| *k)
        .collect();
    assert_eq!(
        k,
        [
            "security.nesting",
            "security.syscalls.intercept.mknod",
            "security.syscalls.intercept.setxattr"
        ]
    );
    assert!(super::run::engine_keys(true)
        .iter()
        .all(|(_, v)| v == "true"));
}

/// T-1993, experiment 434 clause 19: ubi-init has files of mode 0000.
#[test]
fn the_tarball_reads_a_file_and_a_directory_of_mode_0000() {
    use std::os::unix::fs::PermissionsExt;
    let d = scratch("tar0");
    let r = d.join("img/rootfs");
    std::fs::create_dir_all(r.join("etc/locked")).unwrap();
    std::fs::write(r.join("etc/gshadow"), b"root:::").unwrap();
    std::fs::write(r.join("etc/locked/key"), b"k").unwrap();
    for p in ["etc/gshadow", "etc/locked"] {
        std::fs::set_permissions(r.join(p), std::fs::Permissions::from_mode(0o0)).unwrap();
    }
    let out = d.join("img.tar");
    unified_tarball(&r, &out, b"podvm").unwrap();
    let mut a = tar::Archive::new(std::fs::File::open(&out).unwrap());
    let mut seen = Vec::new();
    for e in a.entries().unwrap() {
        let mut e = e.unwrap();
        let p = e.path().unwrap().to_string_lossy().into_owned();
        let mode = e.header().mode().unwrap();
        let mut b = Vec::new();
        e.read_to_end(&mut b).unwrap();
        match p.as_str() {
            "rootfs/etc/gshadow" => assert_eq!((mode, &b[..]), (0, &b"root:::"[..])),
            "rootfs/etc/locked" => assert_eq!(mode, 0, "the image's mode"),
            "rootfs/etc/locked/key" => assert_eq!(b, b"k"),
            _ => {}
        }
        seen.push(p);
    }
    assert!(
        seen.iter().any(|s| s == "rootfs/etc/locked/key"),
        "{seen:?}"
    );
    for p in ["etc/gshadow", "etc/locked"] {
        let m = std::fs::metadata(r.join(p)).unwrap().permissions().mode();
        assert_eq!(m & 0o7777, 0, "{p}: the store keeps the image's mode");
        std::fs::set_permissions(r.join(p), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// TODO/machine.md T-2081 (OR-B39-02): the class of a failed import of an
/// image, after a second look at its alias.
#[test]
fn the_class_of_a_failed_import() {
    use super::import::{after_import, AfterImport, SAME_FINGERPRINT};
    let same = format!("Failed: {SAME_FINGERPRINT}");
    for (alias_there, words, fp, want) in [
        (true, "anything", None, AfterImport::Aliased),
        (true, same.as_str(), Some("ab"), AfterImport::Aliased),
        (
            false,
            same.as_str(),
            Some("ab"),
            AfterImport::Alias("ab".into()),
        ),
        // The writer did not end: no fingerprint to name.
        (false, same.as_str(), None, AfterImport::Failed),
        (
            false,
            "run out of disk space",
            Some("ab"),
            AfterImport::Failed,
        ),
    ] {
        assert_eq!(
            after_import(alias_there, words, fp),
            want,
            "{alias_there} {words}"
        );
    }
}
