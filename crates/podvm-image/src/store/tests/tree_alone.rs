use super::*;

/// TODO/machine.md T-2085 (PodUtils/podrunner#347 ask 1): the incus tier
/// removes the tree of an image after its import only where no other
/// process uses it. One rule: each hold of a record that names the tree,
/// and its extraction lock, refuse; a hold of another tree, and a fill
/// hold, do not.
#[test]
fn a_tree_is_alone_only_where_nothing_holds_a_record_of_it() {
    let _serialised = STORE_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let s = scratch("treealone");
    let a = record("docker.io/library/alpine", Some("latest"), 51);
    // A second tag of the same manifest, under another record digest.
    let mut b = record("docker.io/library/alpine", Some("3"), 52);
    b.manifest_digest = a.manifest_digest.clone();
    let other = record("docker.io/library/busybox", Some("latest"), 53);
    for r in [&a, &b, &other] {
        s.put_record(r.clone()).unwrap();
    }
    type Hold = fn(&Store, &Record) -> Result<Lock>;
    type Case<'a> = (&'a str, Option<(Hold, &'a Record)>, bool);
    let cases: &[Case] = &[
        ("nothing holds it", None, true),
        ("a run of the same tag", Some((Store::hold, &a)), false),
        (
            "a run of another tag of the tree",
            Some((Store::hold, &b)),
            false,
        ),
        (
            "an extraction of the tree",
            Some((Store::hold_extract, &a)),
            false,
        ),
        ("a run of another image", Some((Store::hold, &other)), true),
        ("a fill of the spare", Some((Store::hold_fill, &a)), true),
    ];
    for (what, hold, alone) in cases {
        let held = hold.map(|(f, r)| f(&s, r).unwrap());
        let got = s.try_hold_tree_alone(&a.manifest_digest).unwrap();
        assert_eq!(got.is_some(), *alone, "{what}");
        // The locks of a tree alone keep each run of it out.
        if let Some(locks) = got {
            assert_eq!(locks.len(), 3, "{what}: two holds and the extraction lock");
            assert!(s.in_use(&a).unwrap() && s.in_use(&b).unwrap(), "{what}");
        }
        drop(held);
    }
    // A tree that no record names (removed meanwhile) is alone.
    assert_eq!(
        s.try_hold_tree_alone("sha256:00").unwrap().map(|l| l.len()),
        Some(0)
    );
    let _ = std::fs::remove_dir_all(s.root());
}
