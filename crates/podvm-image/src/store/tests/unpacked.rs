//! TODO/image.md T-2104: the field `unpacked_bytes` of the record.

use super::*;

/// The field is optional on disk: a record of an older podvm has none and
/// reads as `None`; `None` writes no key; a value reads back; and
/// `set_unpacked` keeps a measure in each record of one manifest only.
#[test]
fn the_unpacked_bytes_of_a_record_read_back_and_an_old_record_has_none() {
    let s = scratch("unpacked-field");
    let mut a = record("a", Some("1"), 1);
    let b = record("b", Some("1"), 2);
    // A second name of the manifest of `a`.
    let mut a2 = record("c", Some("1"), 3);
    a2.manifest_digest = a.manifest_digest.clone();
    s.put_record(a.clone()).unwrap();
    s.put_record(b.clone()).unwrap();
    s.put_record(a2.clone()).unwrap();

    // No key where there is no value: the document is the one of an older
    // podvm.
    let text = std::fs::read_to_string(s.root().join("store.json")).unwrap();
    assert!(!text.contains("unpacked_bytes"), "{text}");
    let index = s.read_index().unwrap();
    assert!(index.images.iter().all(|r| r.unpacked_bytes.is_none()));

    s.set_unpacked(&a.manifest_digest, 4242).unwrap();
    let by_repo = |s: &Store, repo: &str| {
        let i = s.read_index().unwrap();
        i.images
            .into_iter()
            .find(|r| r.repository == repo)
            .unwrap()
            .unpacked_bytes
    };
    assert_eq!(by_repo(&s, "a"), Some(4242));
    assert_eq!(by_repo(&s, "c"), Some(4242), "the same manifest");
    assert_eq!(by_repo(&s, "b"), None, "another manifest");
    let text = std::fs::read_to_string(s.root().join("store.json")).unwrap();
    assert!(text.contains("\"unpacked_bytes\": 4242"), "{text}");

    // A record that carries the value, and a document that lacks it.
    a.unpacked_bytes = Some(7);
    s.put_record(a.clone()).unwrap();
    assert_eq!(by_repo(&s, "a"), Some(7));
    let mut doc: serde_json::Value = serde_json::to_value(&b).unwrap();
    assert!(doc.get("unpacked_bytes").is_none());
    doc.as_object_mut().unwrap().remove("unpacked_bytes");
    let old: Record = serde_json::from_value(doc).unwrap();
    assert_eq!(old.unpacked_bytes, None);
    let _ = std::fs::remove_dir_all(s.root());
}
