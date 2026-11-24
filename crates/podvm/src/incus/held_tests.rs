//! TODO/machine.md T-2078 (remaining 2): the record of the need of an image
//! that incus has, against one table.

use super::held::{parse, path, read, text, write};
use super::space::Need;

/// One rule for each record: only a record of version 1 with two counts
/// gives a need; each other text, a missing file, a directory and an alias
/// that is no file name give none, and a need that podvm wrote reads back.
#[test]
fn the_need_of_an_image_that_incus_has_comes_from_its_record() {
    let n = Need {
        bytes: 292_000_000,
        entries: 41_234,
    };
    // (case, the text, the need)
    let rows: [(&str, String, Option<Need>); 9] = [
        ("what podvm writes", text(n), Some(n)),
        ("zero counts", "v1 0 0\n".into(), Some(Need::default())),
        ("another version", "v2 1 2\n".into(), None),
        ("no version", "1 2\n".into(), None),
        ("one count", "v1 1\n".into(), None),
        ("a third field", "v1 1 2 3\n".into(), None),
        ("a negative count", "v1 -1 2\n".into(), None),
        ("not a count", "v1 lots 2\n".into(), None),
        ("empty", String::new(), None),
    ];
    for (case, t, want) in rows {
        assert_eq!(parse(&t), want, "{case}");
    }
    let root = super::tests::scratch("t2078-held");
    let store = podvm_image::Store::open(root.join("store")).unwrap();
    let alias = "podvm-0123456789abcdef-fedcba9876543210-v3";
    assert_eq!(read(&store, alias), None, "no record");
    write(&store, alias, n).unwrap();
    assert_eq!(read(&store, alias), Some(n), "the record reads back");
    let p = path(&store, alias).unwrap();
    std::fs::write(&p, "x".repeat(4096)).unwrap();
    assert_eq!(read(&store, alias), None, "a large file");
    std::fs::remove_file(&p).unwrap();
    std::fs::create_dir(&p).unwrap();
    assert_eq!(read(&store, alias), None, "a directory");
    for bad in ["", ".", "..", "a/b"] {
        assert_eq!(path(&store, bad), None, "{bad:?}");
        assert!(write(&store, bad, n).is_err(), "{bad:?}");
    }
    let _ = std::fs::remove_dir_all(&root);
}
