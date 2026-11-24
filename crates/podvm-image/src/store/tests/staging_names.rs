//! TODO/image.md T-2069: the names of the staging files and the sweep of
//! the ones that a writer locks from their creation.

use super::*;
use crate::store::blobs::{HELD_AGE, STAGING_AGE};
use crate::store::{staging_name_with, HELD_SUFFIX};

/// A planted random source: `Some` gives its bytes, `None` refuses.
struct Planted(Option<[u8; 8]>);

impl pod_core::entropy::Entropy for Planted {
    fn fill(&mut self, buf: &mut [u8]) -> std::result::Result<(), pod_core::entropy::NoRandom> {
        match self.0 {
            Some(b) => {
                buf.copy_from_slice(&b[..buf.len()]);
                Ok(())
            }
            None => Err(pod_core::entropy::NoRandom),
        }
    }
}

/// One rule for each name: the hint, the pid, a sequence and 16 hex digits
/// of the random source (or of the clock where the source refuses), then
/// the suffix of a held file; two random sources give two names also where
/// the pid and the sequence are the same.
#[test]
fn a_staging_name_is_unique_across_pid_namespaces() {
    let pid = std::process::id().to_string();
    // (case, the source, the token that the name holds, or None for any)
    type Row = (&'static str, Option<[u8; 8]>, Option<&'static str>);
    let rows: [Row; 3] = [
        (
            "random bytes",
            Some([1, 2, 3, 4, 5, 6, 7, 8]),
            Some("0807060504030201"),
        ),
        (
            "other bytes",
            Some([9, 9, 9, 9, 9, 9, 9, 9]),
            Some("0909090909090909"),
        ),
        ("no random bytes: the clock", None, None),
    ];
    let mut tokens = Vec::new();
    for (case, src, want) in rows {
        let n = staging_name_with("layer", &mut Planted(src));
        let parts: Vec<&str> = n
            .strip_suffix(HELD_SUFFIX)
            .unwrap_or("")
            .split('.')
            .collect();
        assert_eq!(parts.len(), 4, "{case}: {n}");
        assert_eq!((parts[0], parts[1]), ("layer", pid.as_str()), "{case}: {n}");
        assert!(parts[2].parse::<u64>().is_ok(), "{case}: {n}");
        assert!(
            parts[3].len() == 16 && parts[3].chars().all(|c| c.is_ascii_hexdigit()),
            "{case}: {n}"
        );
        if let Some(w) = want {
            assert_eq!(parts[3], w, "{case}");
        }
        tokens.push(parts[3].to_string());
    }
    assert_ne!(tokens[0], tokens[1], "two sources gave one token");
}

/// One rule for each staging file: a sweep removes a file that nobody holds
/// once it is older than its age: [`HELD_AGE`] for a name of a writer that
/// locks from the create, [`STAGING_AGE`] for any other `.partial`. A held
/// file stays at any age.
#[test]
fn the_sweep_takes_a_held_name_that_nobody_holds_after_its_short_age() {
    let _serialised = STORE_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let s = scratch("t2069-sweep");
    let dir = s.root().join("staging");
    let ago = |d: std::time::Duration| std::time::SystemTime::now() - d;
    let set_age = |p: &std::path::Path, when: std::time::SystemTime| {
        std::fs::File::options()
            .write(true)
            .open(p)
            .and_then(|f| f.set_modified(when))
            .unwrap();
    };
    let (dead, handle) = s.stage("killed-pull").unwrap();
    drop(handle);
    let (live, _held) = s.stage("live-pull").unwrap();
    let young = dir.join(format!("young.1.2.0000000000000001{HELD_SUFFIX}"));
    let other = dir.join("containers.json.1.2.partial");
    std::fs::write(&young, b"x").unwrap();
    std::fs::write(&other, b"[]").unwrap();
    let past_held = ago(HELD_AGE + std::time::Duration::from_secs(5));
    set_age(&dead, past_held);
    set_age(&live, ago(STAGING_AGE * 2));
    set_age(&other, past_held);
    // (case, the file, swept)
    let rows = [
        ("a held name that nobody holds, past its age", &dead, true),
        (
            "a held name that its writer holds, at any age",
            &live,
            false,
        ),
        (
            "a held name that nobody holds, younger than its age",
            &young,
            false,
        ),
        (
            "another .partial past the short age, younger than its own",
            &other,
            false,
        ),
    ];
    let swept = s.sweep_staging();
    for (case, p, want) in rows {
        assert_eq!(swept.contains(p), want, "{case}: {}", p.display());
        assert_eq!(p.exists(), !want, "{case}");
    }
    let _ = std::fs::remove_dir_all(s.root());
}

/// A blob that `put_bytes` writes is named by its digest, and no staging
/// file stays: the lock holds through the rename.
#[test]
fn put_bytes_leaves_no_staging_file() {
    let _serialised = STORE_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let s = scratch("t2069-put");
    let body = b"a small blob";
    let d = crate::digest::Digest::of(body);
    s.put_bytes(body, &d, "config").unwrap();
    assert_eq!(s.read_blob(&d).unwrap(), body);
    let left: Vec<_> = std::fs::read_dir(s.root().join("staging"))
        .unwrap()
        .flatten()
        .collect();
    assert!(left.is_empty(), "{left:?}");
    let _ = std::fs::remove_dir_all(s.root());
}

/// Two writers on one name: the second is refused with one true line that
/// names the file, and the first keeps it.
#[test]
fn a_clash_of_names_is_refused_with_a_true_line() {
    let _serialised = STORE_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let s = scratch("t2069-clash");
    let p = s
        .root()
        .join("staging")
        .join(format!("x.1.2.0000000000000003{HELD_SUFFIX}"));
    let _first = crate::store::StagedFile::create_for_test(&p).unwrap();
    let e = crate::store::StagedFile::create_for_test(&p)
        .err()
        .expect("the second writer was let in");
    let line = e.to_string();
    assert!(
        line.contains("another process holds the staging file")
            && line.contains(&p.display().to_string()),
        "{line}"
    );
    assert!(!line.contains("defect"), "{line}");
    assert!(p.exists());
    let _ = std::fs::remove_dir_all(s.root());
}
