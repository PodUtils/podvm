use super::*;

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("podvm-t1994-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("machine")).unwrap();
    d
}

/// The files of the image `hex`, as the writers name them (`archive.rs`,
/// `disk/root.rs`, `state/store.rs`), each of 10 bytes, with the partial
/// files and the locks.
fn make(root: &Path, hex: &str) -> Vec<PathBuf> {
    let names = [
        format!("base-{hex}-v3.cpio"),
        format!("base-{hex}-v3.partial-0123"),
        format!("root-{hex}-v1.img"),
        format!("root-{hex}-v1.ok"),
        format!("root-{hex}-v1.lock"),
        format!("root-{hex}-v1.img.partial-0123"),
        format!("state-{hex}-0123456789abcdef01234567.vm"),
        format!("state-{hex}-0123456789abcdef01234567.record"),
        format!("state-{hex}-0123456789abcdef01234567.lock"),
        format!("state-{hex}-0123456789abcdef01234567.bad"),
    ];
    names
        .iter()
        .map(|n| {
            let p = root.join("machine").join(n);
            std::fs::write(&p, [7u8; 10]).unwrap();
            p
        })
        .collect()
}

/// The files that are no image's: the kernel, a run directory, a name
/// that is not a digest, a name with a digest that is too short.
fn others(root: &Path) -> Vec<PathBuf> {
    let m = root.join("machine");
    let files = [
        "vmlinuz-virt",
        "base-abc-v3.cpio",
        "root-NOTHEX-v1.img",
        "state-.vm",
    ];
    let mut v: Vec<PathBuf> = files.iter().map(|n| m.join(n)).collect();
    for p in &v {
        std::fs::write(p, b"k").unwrap();
    }
    let run = m.join("run-1-abcd");
    std::fs::create_dir_all(&run).unwrap();
    std::fs::write(run.join(format!("base-{A}-v3.cpio")), b"x").unwrap();
    v.push(run);
    v
}

fn record(name: &str, hex: &str) -> podvm_image::Record {
    podvm_image::Record {
        repository: "docker.io/library/alpine".into(),
        tag: Some(name.into()),
        digest: format!("sha256:{}", "1".repeat(64)),
        digest_media_type: "application/vnd.oci.image.index.v1+json".into(),
        manifest_digest: format!("sha256:{hex}"),
        config_digest: format!("sha256:{}", "3".repeat(64)),
        platform: "linux/amd64".into(),
        layers: vec![],
        stored_bytes: 0,
        unpacked_bytes: None,
        architecture: "amd64".into(),
        os: "linux".into(),
        created: None,
        pulled_at: "2024-01-01T00:00:00Z".into(),
    }
}

/// One rule for each way an image goes (TODO/machine.md T-1994): the
/// machine files go with the last record of the digest; a second name, or
/// a record that stays, keeps them; a lock that a run holds keeps its
/// group; what is no image's stays.
#[test]
fn the_machine_files_go_with_the_last_name_of_an_image_and_only_then() {
    // (case, names of A, a held lock of A, the name that `rmi` removes,
    // the files of A that stay, the files of B that stay)
    type Case = (
        &'static str,
        &'static [&'static str],
        bool,
        &'static str,
        usize,
        usize,
    );
    let cases: [Case; 4] = [
        ("last name", &["a1"], false, "a1", 0, 10),
        ("second name keeps them", &["a1", "a2"], false, "a1", 10, 10),
        (
            "a held root lock keeps its group",
            &["a1"],
            true,
            "a1",
            4,
            10,
        ),
        ("an image of other files", &["a1"], false, "b1", 10, 0),
    ];
    for (case, names, held, rmi, stay, b_stay) in cases {
        let root = scratch(&case.replace(' ', "-"));
        let store = podvm_image::Store::open(&root).unwrap();
        for n in names {
            store.put_record(record(n, A)).unwrap();
        }
        store.put_record(record("b1", B)).unwrap();
        let a = make(&root, A);
        let b = make(&root, B);
        let rest = others(&root);
        let want = format!("alpine:{rmi}");
        let digests: Vec<String> = store
            .find(&want)
            .unwrap()
            .iter()
            .map(|r| r.manifest_digest.clone())
            .collect();
        // What `df` says would go, before it goes.
        let all = store.list().unwrap();
        let doomed: Vec<_> = all
            .iter()
            .filter(|r| r.tag.as_deref() == Some(rmi))
            .cloned()
            .collect();
        let would = reclaimable(&root, &doomed, &all);
        let guard = held.then(|| {
            Lock::try_exclusive(&root.join(format!("machine/root-{A}-v1.lock")))
                .unwrap()
                .unwrap()
        });
        store.remove(&want).unwrap();
        let freed = crate::images::drop_extractions(&store, &digests, "rmi");
        drop(guard);
        let left = a.iter().filter(|p| p.exists()).count();
        assert_eq!(left, stay, "{case}: the files of the image that stay");
        assert_eq!(
            b.iter().filter(|p| p.exists()).count(),
            b_stay,
            "{case}: the files of the other image that stay"
        );
        assert!(
            rest.iter().all(|p| p.exists()),
            "{case}: a file that is no image's went"
        );
        let gone = ((a.len() - left) + (b.len() - b_stay)) as u64 * 10;
        assert_eq!(freed, gone, "{case}: the freed bytes");
        // `df` counted the files that were not held; a held lock is a
        // skip that only the run knows.
        if !held {
            assert_eq!(would, gone, "{case}: df said what would go");
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}

/// The digests that machine files name and no record names are found with
/// their bytes, and a container's reference keeps them.
#[test]
fn machine_files_that_no_record_names_are_orphans() {
    let root = scratch("orphans");
    let store = podvm_image::Store::open(&root).unwrap();
    store.put_record(record("a1", A)).unwrap();
    make(&root, A);
    make(&root, B);
    others(&root);
    let found = orphans(&store);
    assert_eq!(found, vec![(format!("sha256:{B}"), 100)], "{found:?}");
    assert_eq!(all_bytes(&root), 200);
    assert_eq!(remove(&root, &format!("sha256:{B}"), "image prune"), 100);
    assert!(orphans(&store).is_empty());
    assert_eq!(bytes_of(&root, &[format!("sha256:{A}")]), 100);
    let _ = std::fs::remove_dir_all(&root);
}

/// The names of the machine directory, one table: the digits and the group
/// of each, or none.
#[test]
fn only_a_name_with_a_sha256_digest_is_an_image_file() {
    let ok = |n: &str| parts(n).map(|(h, s)| (h.len(), s.to_string()));
    for (name, want) in [
        (
            format!("base-{A}-v3.cpio"),
            Some((64, format!("base-{A}-v3"))),
        ),
        (
            format!("root-{A}-v1.img.partial-9"),
            Some((64, format!("root-{A}-v1"))),
        ),
        (
            format!("state-{A}-0123.vm"),
            Some((64, format!("state-{A}-0123"))),
        ),
        (format!("base-{}-v3.cpio", &A[..63]), None),
        (format!("base-{}-v3.cpio", A.to_uppercase()), None),
        (format!("base-{A}.cpio"), None),
        (format!("run-1-{A}"), None),
        ("vmlinuz-virt".to_string(), None),
        (format!("state-{}x{}-0.vm", &A[..30], &A[31..]), None),
    ] {
        assert_eq!(ok(&name), want, "{name}");
    }
    assert!(hex_of("sha512:abc").is_none() && hex_of(&format!("sha256:{A}")).is_some());
}
