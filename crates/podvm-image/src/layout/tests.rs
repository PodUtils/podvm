use super::*;

fn scratch(name: &str) -> Store {
    let d = std::env::temp_dir().join(format!("podvm-layout-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    Store::open(d).unwrap()
}

fn rootfs_tar() -> Vec<u8> {
    let mut archive = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    let bytes = b"hi";
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "etc/hello", &bytes[..])
        .unwrap();
    archive.into_inner().unwrap()
}

/// T-1338: `load` stages inside the store, because the commit is a
/// rename and a rename across filesystems is EXDEV. A staging path
/// outside the store root is the defect, wherever /tmp lives.
#[test]
fn load_stages_inside_the_store() {
    let s = scratch("staging");
    let bytes = b"blob-bytes";
    let blob = stage_entry(&s, std::path::Path::new("x"), &mut &bytes[..]).unwrap();
    assert!(
        blob.path.starts_with(s.root()),
        "staging escaped the store: {}",
        blob.path.display()
    );
    let _ = std::fs::remove_dir_all(s.root());
}

/// T-2104: `import` measures the unpacked bytes from the layer and records
/// them; a failure of the measure leaves the field empty and fails nothing.
#[test]
fn import_records_the_unpacked_bytes_of_the_layer() {
    let s = scratch("unpacked");
    let tarball = std::env::temp_dir().join(format!("podvm-unpacked-{}.tar", std::process::id()));
    std::fs::write(&tarball, rootfs_tar()).unwrap();
    let record = import(&s, &tarball, Some("imp:u")).unwrap();
    // `etc/hello` holds "hi".
    assert_eq!(record.unpacked_bytes, Some(2));
    assert_eq!(s.find_one("imp:u").unwrap().unpacked_bytes, Some(2));
    // A layer that is gone: `fill` gives no value and no failure.
    let manifest: oci::Manifest = serde_json::from_slice(
        &s.read_blob(&Digest::parse(&record.manifest_digest).unwrap())
            .unwrap(),
    )
    .unwrap();
    let blob = s.blob_path(&manifest.layers[0].parsed_digest().unwrap());
    std::fs::remove_file(blob).unwrap();
    let mut again = Record {
        unpacked_bytes: None,
        manifest_digest: format!("sha256:{}", "9".repeat(64)),
        ..record
    };
    crate::unpacked::fill(&s, &manifest, &mut again);
    assert_eq!(again.unpacked_bytes, None);
    let _ = std::fs::remove_dir_all(s.root());
    let _ = std::fs::remove_file(&tarball);
}

/// T-1320: `import` builds a runnable record from a rootfs tar, with
/// the digest recorded and the layer readable back.
#[test]
fn import_builds_a_record_from_a_rootfs_tar() {
    let s = scratch("import");
    let tar_bytes = rootfs_tar();
    let tarball = std::env::temp_dir().join(format!("podvm-import-{}.tar", std::process::id()));
    std::fs::write(&tarball, &tar_bytes).unwrap();
    let record = import(&s, &tarball, Some("imp:v9")).unwrap();
    assert_eq!(record.repository, "docker.io/library/imp");
    assert_eq!(record.tag.as_deref(), Some("v9"));
    let manifest_digest = Digest::parse(&record.manifest_digest).unwrap();
    let manifest_bytes = s.read_blob(&manifest_digest).unwrap();
    let manifest: oci::Manifest = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest.layers.len(), 1);
    let layer_digest = manifest.layers[0].parsed_digest().unwrap();
    assert_eq!(s.read_blob(&layer_digest).unwrap(), tar_bytes);
    let found = s.find_one("imp:v9").unwrap();
    assert_eq!(found.manifest_digest, record.manifest_digest);
    let _ = std::fs::remove_dir_all(s.root());
    let _ = std::fs::remove_file(&tarball);
}

/// T-1320: a compressed file is refused by name, because the record's
/// media type promises a plain tar.
#[test]
fn import_refuses_a_compressed_file_by_name() {
    let s = scratch("import-gz");
    let mut tar_bytes = rootfs_tar();
    tar_bytes[0] = 0x1f;
    tar_bytes[1] = 0x8b;
    let tarball = std::env::temp_dir().join(format!("podvm-import-gz-{}.tar", std::process::id()));
    std::fs::write(&tarball, &tar_bytes).unwrap();
    let e = import(&s, &tarball, None).unwrap_err();
    assert!(format!("{e}").contains("compressed"), "{e}");
    let _ = std::fs::remove_dir_all(s.root());
    let _ = std::fs::remove_file(&tarball);
}

/// A `save` tarball of an imported image, with `edit` applied to the
/// JSON of its `index.json`.
fn saved_with_index(
    s: &Store,
    tag: &str,
    edit: impl Fn(&mut serde_json::Value),
) -> std::path::PathBuf {
    let src =
        std::env::temp_dir().join(format!("podvm-layout-src-{tag}-{}.tar", std::process::id()));
    std::fs::write(&src, rootfs_tar()).unwrap();
    import(s, &src, Some(&format!("lay:{tag}"))).unwrap();
    let _ = std::fs::remove_file(&src);
    let saved = std::env::temp_dir().join(format!("podvm-layout-{tag}-{}.tar", std::process::id()));
    let mut out = std::fs::File::create(&saved).unwrap();
    save(s, &format!("lay:{tag}"), &mut out).unwrap();
    drop(out);
    let mut a = tar::Archive::new(std::fs::File::open(&saved).unwrap());
    let mut b = tar::Builder::new(Vec::new());
    for e in a.entries().unwrap() {
        let mut e = e.unwrap();
        let path = e.path().unwrap().to_string_lossy().to_string();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut e, &mut bytes).unwrap();
        if path == "index.json" {
            let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            edit(&mut v);
            bytes = serde_json::to_vec(&v).unwrap();
        }
        let mut h = tar::Header::new_gnu();
        h.set_size(bytes.len() as u64);
        h.set_mode(0o644);
        h.set_entry_type(e.header().entry_type());
        h.set_cksum();
        b.append_data(&mut h, &path, &bytes[..]).unwrap();
    }
    std::fs::write(&saved, b.into_inner().unwrap()).unwrap();
    saved
}

/// TODO/image.md T-1839: an index of schemaVersion 9 is refused, and an
/// index entry with no name gives a record with no tag.
#[test]
fn layout_index_refuses_schema_9_and_records_no_tag_with_no_name() {
    let s = scratch("index9");
    let nine = saved_with_index(&s, "nine", |v| v["schemaVersion"] = 9.into());
    let e = load(&s, &nine, &nine).unwrap_err().to_string();
    assert!(e.contains("schemaVersion 9"), "{e}");
    let unnamed = saved_with_index(&s, "anon", |v| {
        for m in v["manifests"].as_array_mut().unwrap() {
            m.as_object_mut().unwrap().remove("annotations");
        }
    });
    let r = load(&s, &unnamed, &unnamed).unwrap();
    assert_eq!(r.tag, None);
    assert_eq!(r.repository, "<none>");
    let _ = std::fs::remove_file(&nine);
    let _ = std::fs::remove_file(&unnamed);
    let _ = std::fs::remove_dir_all(s.root());
}

/// TODO/image.md T-1839: `import` refuses a member that leaves the root.
#[test]
fn layout_index_import_refuses_an_escaping_member() {
    let s = scratch("escape");
    for name in ["../escape", "/abs-escape"] {
        let mut h = tar::Header::new_gnu();
        h.set_size(1);
        h.set_mode(0o644);
        h.as_gnu_mut().unwrap().name[..name.len()].copy_from_slice(name.as_bytes());
        h.set_cksum();
        let mut b = tar::Builder::new(Vec::new());
        b.append(&h, &b"x"[..]).unwrap();
        let t = std::env::temp_dir().join(format!("podvm-escape-{}.tar", std::process::id()));
        std::fs::write(&t, b.into_inner().unwrap()).unwrap();
        let e = import(&s, &t, Some("esc:1")).unwrap_err().to_string();
        assert!(e.contains(name) && e.contains("T-1839"), "{e}");
        let _ = std::fs::remove_file(&t);
    }
    let _ = std::fs::remove_dir_all(s.root());
}

/// TODO/image.md T-1836: a blob whose bytes changed is refused with the
/// entry of the tarball and its computed digest, not "absent".
#[test]
fn messages_name_the_entry_of_a_changed_blob() {
    let s = scratch("changed");
    let saved = saved_with_index(&s, "chg", |_| {});
    let mut a = tar::Archive::new(std::fs::File::open(&saved).unwrap());
    let mut b = tar::Builder::new(Vec::new());
    let mut changed = false;
    for e in a.entries().unwrap() {
        let mut e = e.unwrap();
        let path = e.path().unwrap().to_string_lossy().to_string();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut e, &mut bytes).unwrap();
        // The layer: the largest blob; one byte of it changes.
        if path.starts_with("blobs/sha256/") && bytes.len() > 512 && !changed {
            bytes[100] ^= 0xff;
            changed = true;
        }
        let mut h = tar::Header::new_gnu();
        h.set_size(bytes.len() as u64);
        h.set_mode(0o644);
        h.set_entry_type(e.header().entry_type());
        h.set_cksum();
        b.append_data(&mut h, &path, &bytes[..]).unwrap();
    }
    assert!(changed, "no blob to change");
    std::fs::write(&saved, b.into_inner().unwrap()).unwrap();
    let fresh = scratch("changed-load");
    let e = load(&fresh, &saved, &saved).unwrap_err().to_string();
    assert!(e.contains("for the entry blobs/sha256/"), "{e}");
    assert!(!e.contains("absent"), "{e}");
    let _ = std::fs::remove_file(&saved);
    let _ = std::fs::remove_dir_all(s.root());
    let _ = std::fs::remove_dir_all(fresh.root());
}

/// The orders of the entries of an OCI-layout tarball: the index can come
/// first or last, and the blobs in any order (`save` sorts them by name).
#[derive(Debug, Clone, Copy)]
enum Order {
    IndexFirst,
    IndexLast,
    Reversed,
}

/// An OCI-layout tarball of `n` layers (each a tar of one file), in
/// `order`, at a new path; with the digest of the manifest.
fn layered_layout(tag: &str, n: usize, order: Order) -> (std::path::PathBuf, Digest) {
    let contents = (0..n).map(|i| {
        let mut b = tar::Builder::new(Vec::new());
        let body = format!("layer {i}\n");
        let mut h = tar::Header::new_gnu();
        h.set_size(body.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, format!("t2068/layer-{i}"), body.as_bytes())
            .unwrap();
        b.into_inner().unwrap()
    });
    layout_from_layers(tag, contents, order)
}

fn layout_from_layers(
    tag: &str,
    contents: impl IntoIterator<Item = Vec<u8>>,
    order: Order,
) -> (std::path::PathBuf, Digest) {
    let mut blobs: Vec<(Digest, Vec<u8>)> = Vec::new();
    let mut layers = Vec::new();
    let mut diff_ids = Vec::new();
    for bytes in contents {
        let d = Digest::of(&bytes);
        layers.push(serde_json::json!({
            "mediaType": MEDIA_UNTAR, "digest": d.to_string(), "size": bytes.len(),
        }));
        diff_ids.push(d.to_string());
        if !blobs.iter().any(|(digest, _)| digest == &d) {
            blobs.push((d, bytes));
        }
    }
    let host = crate::platform::Platform::host();
    let config = serde_json::to_vec(&serde_json::json!({
        "architecture": host.arch, "os": "linux",
        "rootfs": {"type": "layers", "diff_ids": diff_ids}, "config": {},
    }))
    .unwrap();
    let config_digest = Digest::of(&config);
    let manifest = serde_json::to_vec(&serde_json::json!({
        "schemaVersion": 2,
        "mediaType": oci::MEDIA_OCI_MANIFEST,
        "config": {
            "mediaType": "application/vnd.oci.image.config.v1+json",
            "digest": config_digest.to_string(), "size": config.len(),
        },
        "layers": layers,
    }))
    .unwrap();
    let manifest_digest = Digest::of(&manifest);
    let index = pod_oci::layout::index_json(
        oci::MEDIA_OCI_MANIFEST,
        &manifest_digest,
        manifest.len(),
        &format!("localhost/t2068:{tag}"),
    )
    .unwrap();
    blobs.push((config_digest, config));
    blobs.push((manifest_digest.clone(), manifest));
    let entries: Vec<(String, Vec<u8>)> = blobs
        .into_iter()
        .map(|(d, b)| {
            (
                format!(
                    "blobs/sha256/{}",
                    d.to_string().trim_start_matches("sha256:")
                ),
                b,
            )
        })
        .collect();
    let meta = vec![
        ("oci-layout".to_string(), OCI_LAYOUT.as_bytes().to_vec()),
        ("index.json".to_string(), index),
    ];
    let entries: Vec<(String, Vec<u8>)> = match order {
        Order::IndexFirst => meta.into_iter().chain(entries).collect(),
        Order::IndexLast => entries.into_iter().chain(meta).collect(),
        Order::Reversed => entries.into_iter().rev().chain(meta).collect(),
    };
    let mut b = tar::Builder::new(Vec::new());
    for (path, bytes) in &entries {
        let mut h = tar::Header::new_gnu();
        h.set_size(bytes.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        b.append_data(&mut h, path, &bytes[..]).unwrap();
    }
    let path = std::env::temp_dir().join(format!(
        "podvm-t2068-{tag}-{}-{}.tar",
        std::process::id(),
        crate::digest::hex_of(&rand_bytes())
    ));
    std::fs::write(&path, b.into_inner().unwrap()).unwrap();
    (path, manifest_digest)
}

/// Eight bytes of the kernel's entropy: a name of a test is unique across
/// PID namespaces too (AGENTS.md 2b).
fn rand_bytes() -> [u8; 8] {
    let mut r = [0u8; 8];
    std::io::Read::read_exact(&mut std::fs::File::open("/dev/urandom").unwrap(), &mut r).unwrap();
    r
}

#[test]
fn repeated_empty_layers_use_one_blob_from_the_archive() {
    let base = rootfs_tar();
    let empty = tar::Builder::new(Vec::new()).into_inner().unwrap();
    let base_digest = Digest::of(&base).to_string();
    let empty_digest = Digest::of(&empty).to_string();
    let (tarball, manifest_digest) =
        layout_from_layers("repeated", [empty.clone(), base, empty], Order::IndexFirst);
    let store = scratch("repeated");
    let record = load(&store, &tarball, &tarball).unwrap();
    assert_eq!(record.manifest_digest, manifest_digest.to_string());
    assert_eq!(
        record.layers,
        [empty_digest.clone(), base_digest, empty_digest]
    );
    let _ = std::fs::remove_file(tarball);
    let _ = std::fs::remove_dir_all(store.root());
}

/// TODO/image.md T-2068 (PodUtils/podrunner#344): a verb that walks the
/// layers of an image holds one staging lock at a time, so an image with
/// more layers than the slots of the lock table (`FORK_CLOSE_SLOTS`)
/// loads, in each order of the entries, and `save` gives it back whole.
/// Before, `load` held one lock for each blob until the end, and the 15th
/// was refused ("this process already holds 16 locks").
#[test]
fn an_image_with_more_layers_than_the_lock_slots_loads_and_saves() {
    let n = 2 * podvm_probe::sys::FORK_CLOSE_SLOTS + 8;
    for order in [Order::IndexFirst, Order::IndexLast, Order::Reversed] {
        let tag = format!("{order:?}").to_lowercase();
        let (tarball, manifest_digest) = layered_layout(&tag, n, order);
        let s = scratch(&format!("t2068-{tag}"));
        let r = load(&s, &tarball, &tarball).unwrap_or_else(|e| panic!("{order:?}: load: {e}"));
        assert_eq!(r.layers.len(), n, "{order:?}");
        assert_eq!(r.manifest_digest, manifest_digest.to_string(), "{order:?}");
        for l in &r.layers {
            s.read_blob(&Digest::parse(l).unwrap())
                .unwrap_or_else(|e| panic!("{order:?}: {e}"));
        }
        let left: Vec<_> = std::fs::read_dir(s.root().join("staging"))
            .map(|d| d.flatten().map(|e| e.file_name()).collect())
            .unwrap_or_default();
        assert!(left.is_empty(), "{order:?}: staging keeps {left:?}");
        // `save` of the record and `load` of that tarball into a new store.
        let saved = tarball.with_extension("saved.tar");
        let mut out = std::fs::File::create(&saved).unwrap();
        let done = save(&s, &format!("localhost/t2068:{tag}"), &mut out)
            .unwrap_or_else(|e| panic!("{order:?}: {e}"));
        drop(out);
        assert_eq!(done.blobs, n + 2, "{order:?}");
        let again = scratch(&format!("t2068-{tag}-again"));
        let back =
            load(&again, &saved, &saved).unwrap_or_else(|e| panic!("{order:?}: reload: {e}"));
        assert_eq!(back.manifest_digest, r.manifest_digest, "{order:?}");
        for p in [&tarball, &saved] {
            let _ = std::fs::remove_file(p);
        }
        let _ = std::fs::remove_dir_all(s.root());
        let _ = std::fs::remove_dir_all(again.root());
    }
}

/// TODO/image.md T-2068: a writer commits each blob before its record, so
/// `rmi` or a prune of another image that shares a blob can remove it in
/// between. The record of a writer is written only where each blob that
/// it names is in the store, and the refusal names the blob.
#[test]
fn a_record_is_written_only_where_each_blob_that_it_names_is_there() {
    let s = scratch("t2068-whole");
    let tarball = std::env::temp_dir().join(format!(
        "podvm-t2068-whole-{}.tar",
        crate::digest::hex_of(&rand_bytes())
    ));
    std::fs::write(&tarball, rootfs_tar()).unwrap();
    let record = import(&s, &tarball, Some("whole:1")).unwrap();
    let _ = std::fs::remove_file(&tarball);
    let cases: [(&str, &str); 3] = [
        ("the manifest", &record.manifest_digest),
        ("the config", &record.config_digest),
        ("the layer", &record.layers[0]),
    ];
    for (what, blob) in cases {
        let path = s.blob_path(&Digest::parse(blob).unwrap());
        let bytes = std::fs::read(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        let e = s.put_written(record.clone()).expect_err(what).to_string();
        assert!(e.contains(blob) && e.contains("rmi"), "{what}: {e}");
        std::fs::write(&path, bytes).unwrap();
        s.put_written(record.clone())
            .unwrap_or_else(|e| panic!("{what}: whole: {e}"));
    }
    let _ = std::fs::remove_dir_all(s.root());
}
