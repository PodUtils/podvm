//! TODO/deps.md T-1679: each host of another party that podvm's code names
//! is in `pod-services`, podcrates' one list of such hosts.
//!
//! The test reads the Rust sources of the workspace and takes each host of
//! a URL (`https://HOST`) and each registry of an image reference
//! (`"HOST/...`). The repository's own tools (`podvm-gate`,
//! `podvm-release`) and the tests are out of scope: they are the tools of
//! podvm's own repository (the inventory of `pod-services`, "stay").

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Hosts that podvm names and that `pod-services` does not list yet. Each
/// was reported to podcrates; the test fails when the crate lists one, so
/// this list only shrinks.
const WAITING: &[&str] = &[];

/// Hosts that only name an XML namespace: no code of podvm connects to
/// them (TODO/machine.md T-1866, the answer file of a Windows setup).
const NAMESPACES: &[&str] = &["schemas.microsoft.com"];

/// Hosts that are podvm's own or no host of another party.
const OWN: &[&str] = &["localhost", "example.com", "example.org"];

/// The names that RFC 2606 reserves: no party answers them.
fn reserved(h: &str) -> bool {
    [".test", ".example", ".invalid", ".localhost"]
        .iter()
        .any(|t| h.ends_with(t))
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            if name != "tests" {
                rust_files(&p, out);
            }
        } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") {
            out.push(p);
        }
    }
}

fn is_host(h: &str) -> bool {
    h.contains('.')
        && h.split('.').all(|l| {
            !l.is_empty()
                && l.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
        && h.rsplit('.')
            .next()
            .is_some_and(|tld| tld.bytes().all(|b| b.is_ascii_lowercase()))
}

/// The hosts that `text` names: of a URL, and of a quoted image reference.
fn hosts_of(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for scheme in ["https://", "http://"] {
        for part in text.split(scheme).skip(1) {
            let h: String = part
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
                .collect();
            if is_host(&h) {
                out.insert(h);
            }
        }
    }
    for part in text.split('"').skip(1) {
        if let Some((h, rest)) = part.split_once('/')
            && is_host(h)
            && !rest.starts_with('/')
            && !part.contains(' ')
        {
            out.insert(h.to_string());
        }
    }
    out
}

/// Each host that `pod-services` lists: of an endpoint, and of a fact whose
/// value is a host or a URL.
fn listed() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for s in pod_services::all() {
        for e in s.endpoints() {
            out.insert(e.host().to_string());
        }
        for f in s.facts() {
            let values: Vec<&str> = s
                .texts(f.name())
                .map(|t| t.iter().map(|x| x.as_ref()).collect())
                .or_else(|| s.text(f.name()).map(|v| vec![v]))
                .unwrap_or_default();
            for v in values {
                let v = v
                    .trim_start_matches("https://")
                    .trim_start_matches("http://");
                let h = v.split('/').next().unwrap_or("");
                if is_host(h) {
                    out.insert(h.to_string());
                }
            }
        }
    }
    out
}

#[test]
fn each_host_that_podvm_names_is_in_pod_services() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for e in std::fs::read_dir(&crates).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name == "podvm-gate" || name == "podvm-release" {
            continue;
        }
        rust_files(&e.path().join("src"), &mut files);
    }
    assert!(files.len() > 100, "the scan found {} files", files.len());
    let mut named: BTreeSet<String> = BTreeSet::new();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_default();
        named.extend(hosts_of(&text));
    }
    let listed = listed();
    let missing: Vec<&String> = named
        .iter()
        .filter(|h| !listed.contains(*h) && !OWN.contains(&h.as_str()) && !reserved(h))
        .filter(|h| !WAITING.contains(&h.as_str()) && !NAMESPACES.contains(&h.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "podvm names hosts that pod-services does not list: {missing:?}; report \
         each to podcrates (TODO/deps.md T-1679), podvm keeps no second list"
    );
    for w in WAITING {
        assert!(
            !listed.contains(*w),
            "pod-services lists {w} now: drop it from WAITING"
        );
        assert!(
            named.contains(*w),
            "podvm no longer names {w}: drop it from WAITING"
        );
    }
    // The values that podvm reads resolve.
    assert!(crate::machine::guest::kernel::url().starts_with("https://dl-cdn.alpinelinux.org/"));
    assert_eq!(crate::machine::guest::kernel::kernel_sha256().len(), 64);
    // The NIC module sums of the guest are facts of pod-services now; the
    // values are the ones podvm pinned before (T-1656).
    assert_eq!(
        crate::machine::guest::kernel::modules(),
        [
            (
                "failover",
                "6abca214c939b70af47d83380251e0ea16a1d8e47490ddb0521e64abcad3e1a1"
            ),
            (
                "net_failover",
                "0ede1d375546f9b177cc1d4250bc71d74e9a8444a83f5da7d1f92b7479b6c036"
            ),
            (
                "virtio_net",
                "fd93a908f5cc37eeb446c7255d614af90ab13dcadd21ddd7bada67a77cb1d177"
            ),
        ]
    );
    assert_eq!(
        crate::images::default_login_server(),
        "https://index.docker.io/v1/"
    );
}

/// T-1013 and T-1014 of podcrates (PodUtils/podvm#618): the store copy of
/// QEMU is the service `static-pkgs-qemu`, on `github.com`. Each archive of
/// `podvm_windows::tools` is an endpoint of it, with the size and the
/// sha256 of its facts, and each program in it has the member, the size and
/// the sha256 of its facts: the pins of podvm and of podcrates are one.
#[test]
fn the_store_copy_of_qemu_holds_the_facts_of_pod_services() {
    use podvm_windows::tools;
    let q = pod_services::get("static-pkgs-qemu").expect("the service of the static QEMU");
    assert_eq!(q.text("version"), Some(tools::QEMU_VERSION));
    assert_eq!(q.text("release"), Some(tools::RELEASE));
    for gone in ["pkgforge-qemu", "hermeticbuild-qemu"] {
        assert!(pod_services::get(gone).is_none(), "{gone}");
    }
    let mut endpoints = 0;
    for e in q.endpoints() {
        let name = e.name();
        let Some(a) = tools::ARCHIVES.iter().find(|a| {
            e.path()
                .is_some_and(|p| p.ends_with(&format!("/{}", a.name)))
        }) else {
            // qemu-system-aarch64: the emulator of an arm64 guest, which
            // the store of podvm does not take (TODO/machine.md T-1875).
            assert_eq!(
                name, "qemu-system-aarch64",
                "an archive that podvm does not pin"
            );
            continue;
        };
        endpoints += 1;
        assert_eq!(e.host(), "github.com", "{name}");
        assert_eq!(
            a.url,
            format!("https://github.com{}", e.path().unwrap()),
            "{name}"
        );
        assert_eq!(q.text(&format!("{name}-sha256")), Some(a.sha256), "{name}");
        assert_eq!(
            q.int(&format!("{name}-bytes")),
            Some(a.size as i64),
            "{name}"
        );
        if let Some(member) = q.text(&format!("{name}-member")) {
            let m = a
                .members
                .iter()
                .find(|m| m.path == member)
                .unwrap_or_else(|| panic!("{name}: no member {member}"));
            assert_eq!(
                q.text(&format!("{name}-bin-sha256")),
                Some(m.sha256),
                "{name}"
            );
            assert_eq!(
                q.int(&format!("{name}-bin-bytes")),
                Some(m.size as i64),
                "{name}"
            );
        }
    }
    assert_eq!(endpoints, tools::ARCHIVES.len());
}

#[test]
fn the_scan_finds_a_url_and_an_image_reference() {
    let h = hosts_of(
        r#"x = "https://dl-cdn.alpinelinux.org/a"; image = "quay.io/jupyter/b:1"; "a b/c" "//x""#,
    );
    assert_eq!(
        h,
        ["dl-cdn.alpinelinux.org", "quay.io"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    );
}
