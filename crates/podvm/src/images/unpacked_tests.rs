//! TODO/image.md T-2104 (PodUtils/podrunner#372): `.UnpackedBytes` of
//! `inspect`, from the layers of an image in a store.

use super::tests::a_record;
use super::*;

/// An entry of a test layer.
enum E {
    /// A regular file of this many bytes.
    File(&'static str, usize),
    /// A hard link to an earlier name.
    Link(&'static str, &'static str),
}

use E::{File, Link};

fn layer(entries: &[E]) -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());
    for e in entries {
        let mut h = tar::Header::new_ustar();
        h.set_uid(0);
        h.set_gid(0);
        h.set_mode(0o644);
        match e {
            File(p, n) => {
                h.set_size(*n as u64);
                tar.append_data(&mut h, p, vec![b'x'; *n].as_slice())
                    .unwrap();
            }
            Link(p, target) => {
                h.set_entry_type(tar::EntryType::Link);
                h.set_size(0);
                tar.append_link(&mut h, p, target).unwrap();
            }
        }
    }
    tar.into_inner().unwrap()
}

/// A store with the image of `layers` and its record (with no value of the
/// unpacked bytes, as a record of an older podvm).
fn image(name: &str, layers: &[Vec<E>]) -> (Store, Record) {
    let dir = std::env::temp_dir().join(format!("podvm-unpacked-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(dir).unwrap();
    let mut descriptors = Vec::new();
    let mut digests = Vec::new();
    for l in layers {
        let bytes = layer(l);
        let d = podvm_image::digest::Digest::of(&bytes);
        store.put_bytes(&bytes, &d, "layer").unwrap();
        descriptors.push(serde_json::json!({
            "mediaType": "application/vnd.oci.image.layer.v1.tar",
            "digest": d.to_string(),
            "size": bytes.len(),
        }));
        digests.push(d.to_string());
    }
    let config = b"{}".to_vec();
    let config_digest = podvm_image::digest::Digest::of(&config);
    store.put_bytes(&config, &config_digest, "config").unwrap();
    let manifest = serde_json::to_vec(&serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "digest": config_digest.to_string(),
            "size": config.len(),
        },
        "layers": descriptors,
    }))
    .unwrap();
    let manifest_digest = podvm_image::digest::Digest::of(&manifest);
    store
        .put_bytes(&manifest, &manifest_digest, "manifest")
        .unwrap();
    let record = Record {
        digest: manifest_digest.to_string(),
        manifest_digest: manifest_digest.to_string(),
        config_digest: config_digest.to_string(),
        layers: digests,
        ..a_record()
    };
    store.put_record(record.clone()).unwrap();
    (store, record)
}

/// The cases of the measure: the layers, oldest first, and the bytes of
/// the regular files of the final tree with each inode once.
fn cases() -> Vec<(&'static str, Vec<Vec<E>>, u64)> {
    vec![
        ("no layer", vec![], 0),
        ("one file", vec![vec![File("a", 10)]], 10),
        (
            "files of two layers",
            vec![vec![File("a", 10)], vec![File("d/b", 5)]],
            15,
        ),
        ("empty files", vec![vec![File("a", 0), File("d/b", 0)]], 0),
        (
            "a whiteout removes a file",
            vec![vec![File("a", 10), File("b", 5)], vec![File(".wh.a", 0)]],
            5,
        ),
        (
            "a whiteout removes a directory",
            vec![
                vec![File("d/x", 7), File("d/y", 3), File("e", 1)],
                vec![File(".wh.d", 0)],
            ],
            1,
        ),
        (
            "an opaque marker hides the files below it",
            vec![
                vec![File("d/x", 7), File("d/y", 3)],
                vec![File("d/.wh..wh..opq", 0), File("d/z", 2)],
            ],
            2,
        ),
        (
            "a later layer replaces a file",
            vec![vec![File("a", 10)], vec![File("a", 4)]],
            4,
        ),
        (
            "a later layer replaces a file with a longer one",
            vec![vec![File("a", 4)], vec![File("a", 10)]],
            10,
        ),
        (
            "hard links of one layer count once",
            vec![vec![
                File("bin/busybox", 100),
                Link("bin/sh", "bin/busybox"),
                Link("usr/bin/env", "bin/busybox"),
            ]],
            100,
        ),
        (
            "a hard link across layers counts once",
            vec![
                vec![File("bin/busybox", 100)],
                vec![Link("bin/sh", "bin/busybox")],
            ],
            100,
        ),
        (
            "a whiteout of the first name keeps the data of the link",
            vec![
                vec![File("bin/busybox", 100), Link("bin/sh", "bin/busybox")],
                vec![File("bin/.wh.busybox", 0)],
            ],
            100,
        ),
        (
            "a later layer replaces the target of a link",
            vec![
                vec![File("bin/t", 50), Link("bin/l", "bin/t")],
                vec![File("bin/t", 8)],
            ],
            58,
        ),
    ]
}

/// One rule: `inspect` gives the sum of the sizes of the regular files of
/// the final tree, each inode once, in the JSON and in a template; it keeps
/// the value in the record; and a record with the value gives it with no
/// layer in the store.
#[test]
fn the_unpacked_bytes_follow_the_final_tree_of_the_layers() {
    for (i, (what, layers, want)) in cases().into_iter().enumerate() {
        let (store, record) = image(&format!("t{i}"), &layers);
        assert_eq!(record.unpacked_bytes, None, "{what}");
        let (got, why) = podvm_image::unpacked::complete(&store, record.clone());
        assert_eq!(why, None, "{what}");
        assert_eq!(got.unpacked_bytes, Some(want), "{what}");

        let doc: serde_json::Value = serde_json::from_str(&inspect_json(&got, &store)).unwrap();
        assert_eq!(doc["UnpackedBytes"], serde_json::json!(want), "{what}");
        let fields = inspect_fields(&got, &store);
        let text = format::render_typed("{{.UnpackedBytes}}", &fields, &[]).unwrap();
        assert_eq!(text, want.to_string(), "{what}");
        let json = format::render_typed("{{json .UnpackedBytes}}", &fields, &[]).unwrap();
        assert_eq!(json, want.to_string(), "{what}");

        // The first `inspect` kept it: the next one reads the record.
        let kept = store.read_index().unwrap();
        assert_eq!(kept.images[0].unpacked_bytes, Some(want), "{what}: kept");

        // A record with the value needs no layer.
        for l in &record.layers {
            let d = podvm_image::digest::Digest::parse(l).unwrap();
            let _ = std::fs::remove_file(store.blob_path(&d));
        }
        let (again, why) = podvm_image::unpacked::complete(&store, kept.images[0].clone());
        assert_eq!((again.unpacked_bytes, why), (Some(want), None), "{what}");
        let _ = std::fs::remove_dir_all(store.root());
    }
}

/// Where podvm cannot tell, the field is null (a dash in a template) and
/// the reason is one line; nothing fails.
#[test]
fn an_unknown_unpacked_size_is_a_dash_with_its_reason() {
    // A layer that is not in the store.
    let (store, record) = image("gone", &[vec![File("a", 10)]]);
    let d = podvm_image::digest::Digest::parse(&record.layers[0]).unwrap();
    std::fs::remove_file(store.blob_path(&d)).unwrap();
    let (got, why) = podvm_image::unpacked::complete(&store, record);
    assert_eq!(got.unpacked_bytes, None);
    let why = why.expect("a reason");
    assert!(
        why.contains("is not in the store") && !why.contains('\n'),
        "{why}"
    );
    let doc: serde_json::Value = serde_json::from_str(&inspect_json(&got, &store)).unwrap();
    assert!(doc["UnpackedBytes"].is_null());
    let text = format::render_typed("{{.UnpackedBytes}}", &inspect_fields(&got, &store), &[]);
    assert_eq!(text.unwrap(), "-");
    assert_eq!(store.read_index().unwrap().images[0].unpacked_bytes, None);
    let _ = std::fs::remove_dir_all(store.root());

    // A layer that is not a tar (bytes that fail the stream).
    let (store, record) = image("bad", &[vec![File("a", 10)]]);
    let d = podvm_image::digest::Digest::parse(&record.layers[0]).unwrap();
    let junk = vec![0xa5u8; 1024];
    std::fs::write(store.blob_path(&d), &junk).unwrap();
    let (got, why) = podvm_image::unpacked::complete(&store, record);
    assert_eq!(got.unpacked_bytes, None);
    assert!(why.is_some_and(|w| !w.is_empty() && !w.contains('\n')));
    let _ = std::fs::remove_dir_all(store.root());
}
