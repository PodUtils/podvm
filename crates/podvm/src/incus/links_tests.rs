//! TODO/cli.md T-2079 (OR-B39-08): the unified tarball keeps the hard links
//! of an image, on each path of the import: the stream of the layers and the
//! tree of the extraction. A link whose first name is not in the stream (a
//! removed or replaced target) gets the data, never a dangling link.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use super::image::{unified_tarball_to, Layers};
use super::tests::scratch;

/// An entry of a test layer.
enum E {
    File(&'static str, &'static [u8]),
    Link(&'static str, &'static str),
}

fn layer(path: &Path, entries: &[E]) -> podvm_extract::Layer {
    let mut tar = tar::Builder::new(Vec::new());
    for e in entries {
        let mut h = tar::Header::new_ustar();
        h.set_uid(0);
        h.set_gid(0);
        h.set_mode(0o755);
        match e {
            E::File(p, data) => {
                h.set_size(data.len() as u64);
                tar.append_data(&mut h, p, *data).unwrap();
            }
            E::Link(p, target) => {
                h.set_entry_type(tar::EntryType::Link);
                h.set_size(0);
                tar.append_link(&mut h, p, target).unwrap();
            }
        }
    }
    let bytes = tar.into_inner().unwrap();
    std::fs::write(path, &bytes).unwrap();
    podvm_extract::Layer {
        digest: podvm_image::digest::Digest::of(&bytes).to_string(),
        media_type: "application/vnd.oci.image.layer.v1.tar".into(),
        size: bytes.len() as u64,
        blob: path.into(),
    }
}

/// What a reader of the archive gets: each path of the image (not
/// `/.podvm`) and its inode, and the data of each inode. A hard link entry
/// whose first name is not a file before it fails the test.
#[derive(Default)]
struct Model {
    paths: HashMap<String, usize>,
    data: Vec<Vec<u8>>,
}

fn rel(p: &[u8]) -> Option<String> {
    let p = String::from_utf8(p.to_vec()).unwrap();
    let p = p.strip_prefix("rootfs/")?.trim_end_matches('/').to_string();
    (!p.is_empty() && p != ".podvm" && !p.starts_with(".podvm/")).then_some(p)
}

/// Read `archive` into a [`Model`], and unpack it at `dir` where given, as
/// GNU tar does (a hard link entry links to the earlier name).
fn read(archive: &[u8], dir: Option<&Path>, what: &str) -> Model {
    let mut m = Model::default();
    let mut a = tar::Archive::new(archive);
    for e in a.entries().unwrap() {
        let mut e = e.unwrap();
        let Some(p) = rel(&e.path_bytes()) else {
            continue;
        };
        let kind = e.header().entry_type();
        let at = dir.map(|d| d.join(&p));
        if let Some(at) = &at {
            std::fs::create_dir_all(at.parent().unwrap()).unwrap();
            if !kind.is_dir() {
                let _ = std::fs::remove_file(at);
            }
        }
        if kind.is_file() {
            let mut data = Vec::new();
            e.read_to_end(&mut data).unwrap();
            if let Some(at) = &at {
                std::fs::write(at, &data).unwrap();
            }
            m.data.push(data);
            m.paths.insert(p, m.data.len() - 1);
        } else if kind.is_hard_link() {
            let target = e.link_name_bytes().unwrap();
            let t = rel(&target).unwrap_or_else(|| panic!("{what}: {p}: a link out of rootfs"));
            let Some(&ino) = m.paths.get(&t) else {
                panic!("{what}: the link {p} names {t}, which is not before it");
            };
            if let (Some(at), Some(d)) = (&at, dir) {
                std::fs::hard_link(d.join(&t), at).unwrap();
            }
            m.paths.insert(p, ino);
        } else if kind.is_dir()
            && let Some(at) = &at
        {
            std::fs::create_dir_all(at).unwrap();
        }
    }
    m
}

/// A case: the layers, oldest first; the names of each inode of the final
/// tree with its data. A path in no group is not in the tree.
type Case = (
    &'static str,
    Vec<Vec<E>>,
    Vec<(Vec<&'static str>, &'static [u8])>,
);

#[test]
fn each_path_of_the_import_writes_one_inode_for_the_names_of_a_file() {
    use E::{File, Link};
    let cases: Vec<Case> = vec![
        (
            "links of one layer (busybox)",
            vec![vec![
                File("bin/busybox", b"BB"),
                Link("bin/sh", "bin/busybox"),
                Link("usr/bin/env", "bin/busybox"),
            ]],
            vec![(vec!["bin/busybox", "bin/sh", "usr/bin/env"], b"BB")],
        ),
        (
            "a link across layers",
            vec![
                vec![File("bin/busybox", b"BB")],
                vec![Link("bin/sh", "bin/busybox")],
            ],
            vec![(vec!["bin/busybox", "bin/sh"], b"BB")],
        ),
        (
            "a whiteout removes the first name",
            vec![
                vec![File("bin/busybox", b"BB"), Link("bin/sh", "bin/busybox")],
                vec![File("bin/.wh.busybox", b"")],
            ],
            vec![(vec!["bin/sh"], b"BB")],
        ),
        (
            "a later layer removes the target of a link",
            vec![
                vec![File("lib/t", b"T"), File("bin/x", b"X")],
                vec![Link("bin/l", "lib/t"), Link("bin/m", "lib/t")],
                vec![File(".wh.lib", b"")],
            ],
            vec![(vec!["bin/l", "bin/m"], b"T"), (vec!["bin/x"], b"X")],
        ),
        (
            "a later layer replaces the target of a link",
            vec![
                vec![File("bin/t", b"old"), Link("bin/l", "bin/t")],
                vec![File("bin/t", b"new")],
            ],
            vec![(vec!["bin/l"], b"old"), (vec!["bin/t"], b"new")],
        ),
    ];
    let me = b"PODVM";
    for (i, (what, layers, want)) in cases.into_iter().enumerate() {
        let d = scratch(&format!("t2079-links{i}"));
        let blobs = layers
            .iter()
            .enumerate()
            .map(|(n, l)| layer(&d.join(format!("{n}.tar")), l))
            .collect();
        let layers = Layers { blobs, loud: false };
        let bytes: u64 = want.iter().map(|(_, b)| b.len() as u64).sum();
        let check = |m: &Model, path: &str| {
            let names: usize = want.iter().map(|(n, _)| n.len()).sum();
            assert_eq!(m.paths.len(), names, "{what}, {path}: {:?}", m.paths);
            let mut seen = Vec::new();
            for (names, data) in &want {
                let ino = m.paths.get(names[0]).copied();
                let ino = ino.unwrap_or_else(|| panic!("{what}, {path}: no {}", names[0]));
                assert_eq!(&m.data[ino], data, "{what}, {path}: {}", names[0]);
                for n in names {
                    assert_eq!(m.paths.get(*n), Some(&ino), "{what}, {path}: {n}");
                }
                assert!(
                    !seen.contains(&ino),
                    "{what}, {path}: two groups, one inode"
                );
                seen.push(ino);
            }
            // One copy of the data of each inode goes in the archive.
            let stored: u64 = m.data.iter().map(|b| b.len() as u64).sum();
            assert_eq!(stored, bytes, "{what}, {path}: the data");
        };

        // The stream of the layers (T-2085).
        let mut stream = Vec::new();
        layers.flatten(&mut stream, me, false).unwrap();
        let tree = d.join("tree");
        check(&read(&stream, Some(&tree), what), "the stream");
        let need = layers.measure(me.len() as u64).unwrap().expect("no tree");
        assert_eq!(
            need.bytes,
            bytes + me.len() as u64,
            "{what}: the stream's need"
        );

        // The tree of the extraction path, with its links on the disk.
        let tarball = unified_tarball_to(&tree, Vec::new(), me).unwrap();
        check(&read(&tarball, None, what), "the tree");
        let need = super::space::need_of(&tree, me.len() as u64);
        assert_eq!(
            need.bytes,
            bytes + me.len() as u64,
            "{what}: the tree's need"
        );
        let _ = std::fs::remove_dir_all(&d);
    }
}
