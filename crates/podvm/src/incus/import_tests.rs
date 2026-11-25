//! TODO/machine.md T-2085 (PodUtils/podrunner#347 ask 1): the gzip of the
//! upload, and the tree of an import against a stand-in incusd: made for
//! an import only, removed after it, also after a failure.

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::json;

use pod_gzip::BLOCK;

use super::image::Layers;
use super::import::{ensure_image, ensure_image_with, Tree};
use super::run::Job;
use super::tests::{read_request, scratch};

/// Bytes that gzip cannot shrink much: a xorshift stream.
fn noise(n: usize, seed: u64) -> Vec<u8> {
    let mut x = seed | 1;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect()
}

fn gunzip(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    flate2::read::MultiGzDecoder::new(b)
        .read_to_end(&mut out)
        .expect("one gzip file");
    out
}

/// The calls that the import made of its tree.
#[derive(Default)]
struct Calls {
    ensured: usize,
    released: usize,
}

/// A tree of a test: `fresh` where `ensure` makes it; `fail` where the
/// extraction fails.
struct Stub<'a> {
    root: &'a Path,
    layers: Option<Vec<podvm_extract::Layer>>,
    fresh: bool,
    fail: bool,
    calls: Mutex<Calls>,
}

impl Tree for Stub<'_> {
    fn layers(&self) -> Result<Option<Layers>, String> {
        Ok(self
            .layers
            .clone()
            .map(|blobs| Layers { blobs, loud: false }))
    }

    fn ensure(&self) -> Result<bool, String> {
        self.calls.lock().unwrap().ensured += 1;
        if self.fail {
            return Err("the extraction failed (a test)".to_string());
        }
        std::fs::create_dir_all(self.root.join("bin")).unwrap();
        std::fs::write(self.root.join("bin/sh"), b"#!sh").unwrap();
        std::fs::write(self.root.join("bin/big"), noise(BLOCK + 3, 5)).unwrap();
        Ok(self.fresh)
    }
    fn layer_bytes(&self) -> u64 {
        1234
    }
    fn release(&self) -> Option<String> {
        self.calls.lock().unwrap().released += 1;
        let _ = std::fs::remove_dir_all(self.root);
        None
    }
}

/// A stand-in incusd: the alias is there where `alias`; an import answers
/// 500 where `refuse`. It keeps the body of the import.
fn stand_in(
    sock: &Path,
    alias: bool,
    refuse: bool,
    pool_bytes: Option<u64>,
) -> Arc<Mutex<Vec<u8>>> {
    let l = UnixListener::bind(sock).unwrap();
    let upload = Arc::new(Mutex::new(Vec::new()));
    let keep = upload.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            let (m, p, body) = read_request(&mut s);
            let sync = |v: serde_json::Value| (200, json!({"type": "sync", "metadata": v}));
            let (status, out) = match (m.as_str(), p.as_str()) {
                ("GET", p) if p.starts_with("/1.0/images/aliases/") && alias => {
                    sync(json!({"name": "a", "target": "f00d"}))
                }
                ("GET", p) if p.starts_with("/1.0/images/aliases/") => (
                    404,
                    json!({"type": "error", "error": "not found", "error_code": 404}),
                ),
                ("GET", "/1.0/profiles/default") if pool_bytes.is_some() => sync(json!({
                    "name": "default", "devices": {"root": {"type": "disk", "path": "/", "pool": "tiny"}}
                })),
                ("GET", "/1.0/storage-pools/tiny") => sync(json!({
                    "name": "tiny", "driver": "btrfs", "config": {"source": "tiny.img"}
                })),
                ("GET", "/1.0/storage-pools/tiny/resources") => sync(json!({
                    "space": {"used": 0, "total": pool_bytes}
                })),
                ("POST", "/1.0/images") if refuse => (
                    500,
                    json!({"type": "error", "error": "no room (a test)", "error_code": 500}),
                ),
                ("POST", "/1.0/images") => {
                    *keep.lock().unwrap() = body;
                    (
                        202,
                        json!({"type": "async", "operation": "/1.0/operations/i", "metadata": {}}),
                    )
                }
                (_, p) if p.starts_with("/1.0/operations/i/wait") => sync(
                    json!({"status_code": 200, "metadata": {"fingerprint": "f00d", "size": 1}}),
                ),
                _ => sync(json!({})),
            };
            let out = out.to_string();
            let _ = s.write_all(
                format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}",
                    out.len()
                )
                .as_bytes(),
            );
        }
    });
    upload
}

/// One rule: the tree is made only where incusd lacks the image, and a
/// tree that the import made goes after it, also after a failure; the
/// upload is gzip of the unified tarball.
#[test]
fn the_tree_is_made_for_an_import_only_and_goes_after_it() {
    // (what, alias there, incusd refuses, ensure makes it, ensure fails,
    //  incusd's images have room, Ok(cached) or Err, ensured, released)
    type Case = (
        &'static str,
        bool,
        bool,
        bool,
        bool,
        bool,
        Result<bool, ()>,
        usize,
        usize,
    );
    #[rustfmt::skip]
    let cases: &[Case] = &[
        ("incus has the image", true, false, true, false, false, Ok(true), 0, 0),
        ("an import of a tree made now", false, false, true, false, false, Ok(false), 1, 1),
        ("an import with room: plain", false, false, true, false, true, Ok(false), 1, 1),
        ("an import of a tree that was there", false, false, false, false, false, Ok(false), 1, 0),
        ("a refused import of a tree made now", false, true, true, false, false, Err(()), 1, 1),
        ("a refused import of a tree that was there", false, true, false, false, false, Err(()), 1, 0),
        ("an extraction that fails", false, false, true, true, false, Err(()), 1, 0),
    ];
    for (i, (what, alias, refuse, fresh, fail, room, want, ensured, released)) in
        cases.iter().enumerate()
    {
        let d = scratch(&format!("t2085-{i}"));
        let root = d.join("rootfs");
        let sd = crate::test_socket::SocketDir::new(&format!("t2085{i}"));
        let sock = sd.join("unix.socket");
        // The images of incusd beside its socket: the free space of the
        // file system of the tests is room for a plain upload.
        if *room {
            std::fs::create_dir_all(sd.join("images")).unwrap();
        }
        let upload = stand_in(&sock, *alias, *refuse, None);
        let stub = Stub {
            root: &root,
            layers: None,
            fresh: *fresh,
            fail: *fail,
            calls: Mutex::default(),
        };
        let job = Job {
            verb: "create",
            rootfs: &root,
            digest: "sha256:00112233445566778899",
            argv: vec!["true".into()],
            env: vec![],
            workdir: "/".into(),
            stdin: None,
            limits: vec![],
            volumes: vec![],
            detach: None,
            net_none: false,
            channel: None,
            store: None,
            tree: Some(&stub),
            plan: std::sync::OnceLock::new(),
            name: None,
        };
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let c = pod_incus::Client::new(pod_incus::Settings::new(&sock));
        let got = rt.block_on(ensure_image(&c, &job, "btrfs"));
        let calls = stub.calls.lock().unwrap();
        assert_eq!(
            got.as_ref().map(|(_, cached)| *cached).map_err(|_| ()),
            *want,
            "{what}: {got:?}"
        );
        assert_eq!(
            (calls.ensured, calls.released),
            (*ensured, *released),
            "{what}"
        );
        if *fail {
            let (line, _) = got.as_ref().unwrap_err();
            assert_eq!(line, "the extraction failed (a test)", "{what}: one line");
        }
        if *refuse {
            let (line, _) = got.as_ref().unwrap_err();
            assert!(line.contains("no room (a test)"), "{what}: {line}");
        }
        if want == &Ok(false) {
            // The plan of the import is kept for the lines after a failure.
            let plan = job.plan.get().expect("the plan of the import");
            assert_eq!(plan.upload, 1234, "{what}");
            assert_eq!(plan.freed.is_some(), *fresh, "{what}");
            assert_eq!(plan.gzip, !*room, "{what}");
            // The upload is the unified tarball, with podvm in it: gzip
            // where the images lack room.
            let body = upload.lock().unwrap().clone();
            assert_eq!(body.starts_with(&[0x1f, 0x8b]), !*room, "{what}");
            let tar = if *room { body } else { gunzip(&body) };
            let mut a = tar::Archive::new(&tar[..]);
            let names: Vec<String> = a
                .entries()
                .unwrap()
                .map(|e| e.unwrap().path().unwrap().display().to_string())
                .collect();
            for n in [
                "metadata.yaml",
                "rootfs/bin/sh",
                "rootfs/bin/big",
                "rootfs/.podvm/podvm",
            ] {
                assert!(names.iter().any(|x| x == n), "{what}: {n} not in {names:?}");
            }
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}

fn layer(path: &Path, fill: impl FnOnce(&mut tar::Builder<Vec<u8>>)) -> podvm_extract::Layer {
    let mut tar = tar::Builder::new(Vec::new());
    fill(&mut tar);
    let bytes = tar.into_inner().unwrap();
    std::fs::write(path, &bytes).unwrap();
    podvm_extract::Layer {
        digest: podvm_image::digest::Digest::of(&bytes).to_string(),
        media_type: "application/vnd.oci.image.layer.v1.tar".into(),
        size: bytes.len() as u64,
        blob: path.into(),
    }
}

fn file(tar: &mut tar::Builder<Vec<u8>>, path: &str, bytes: &[u8]) {
    let mut h = tar::Header::new_ustar();
    h.set_size(bytes.len() as u64);
    h.set_uid(0);
    h.set_gid(42);
    h.set_mode(0o640);
    tar.append_data(&mut h, path, bytes).unwrap();
}

#[test]
fn the_layers_supply_the_archive_and_the_space_check_with_no_tree() {
    let d = scratch("t2085-layers");
    let root = d.join("rootfs");
    let base = layer(&d.join("base.tar"), |tar| {
        file(tar, "bin/gone", b"gone");
        file(tar, ".podvm/note", b"image note");
        tar.append_pax_extensions([("SCHILY.xattr.user.note", b"note".as_slice())])
            .unwrap();
        file(tar, "bin/target", b"old");
    });
    let links = layer(&d.join("links.tar"), |tar| {
        for path in ["bin/first", "bin/second"] {
            let mut h = tar::Header::new_ustar();
            h.set_entry_type(tar::EntryType::Link);
            h.set_size(0);
            h.set_uid(0);
            h.set_gid(42);
            h.set_mode(0o640);
            tar.append_link(&mut h, path, "bin/target").unwrap();
        }
    });
    let new = layer(&d.join("new.tar"), |tar| {
        file(tar, "bin/.wh.gone", b"");
        file(tar, "bin/target", b"new");
    });
    let replacement = layer(&d.join("replacement.tar"), |tar| {
        file(tar, "bin/child", b"child");
        file(tar, "bin", b"a file in place of the directory");
    });
    let reserved = layer(&d.join("reserved.tar"), |tar| {
        file(tar, ".podvm/podvm", b"the image's older podvm");
    });
    let embedded = b"test podvm";
    let compressed_bytes = base.size + links.size + new.size;
    let need = super::space::Need {
        bytes: (b"image noteoldnew".len() + embedded.len()) as u64,
        entries: 6,
    };
    let store = podvm_image::Store::open(d.join("store")).unwrap();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut plain_bytes = Vec::new();
    for (i, what) in [
        "plain",
        "gzip",
        "no room",
        "bad digest",
        "fallback",
        "reserved init",
    ]
    .into_iter()
    .enumerate()
    {
        let sd = crate::test_socket::SocketDir::new(&format!("layers{i}"));
        let sock = sd.join("unix.socket");
        if what == "plain" {
            std::fs::create_dir_all(sd.join("images")).unwrap();
        }
        let captured = stand_in(&sock, false, false, (what == "no room").then_some(1));
        let reserved_init = what == "reserved init";
        let mut blobs = vec![base.clone(), links.clone(), new.clone()];
        if what == "bad digest" {
            blobs[0].digest = podvm_image::digest::Digest::of(b"other content").to_string();
        } else if what == "fallback" {
            blobs = vec![replacement.clone()];
        } else if reserved_init {
            blobs = vec![reserved.clone()];
            std::fs::create_dir_all(root.join(".podvm")).unwrap();
            std::fs::write(root.join(".podvm/podvm"), b"the image's older podvm").unwrap();
        }
        let stub = Stub {
            root: &root,
            layers: Some(blobs),
            fresh: !reserved_init,
            fail: false,
            calls: Mutex::default(),
        };
        let job = Job {
            verb: "create",
            rootfs: &root,
            digest: "sha256:ddeeff0011223344556677",
            argv: vec!["true".into()],
            env: vec![],
            workdir: "/".into(),
            stdin: None,
            limits: vec![],
            volumes: vec![],
            detach: None,
            net_none: false,
            channel: None,
            store: Some(&store),
            tree: Some(&stub),
            plan: std::sync::OnceLock::new(),
            name: None,
        };
        let c = pod_incus::Client::new(pod_incus::Settings::new(&sock));
        let result = rt.block_on(ensure_image_with(&c, &job, "btrfs", embedded.to_vec()));
        let calls = stub.calls.lock().unwrap();
        assert_eq!(root.exists(), reserved_init, "{what}");
        if reserved_init {
            assert!(!result.unwrap().1, "the fallback imported the image");
            assert_eq!((calls.ensured, calls.released), (1, 0));
            let data = gunzip(&captured.lock().unwrap());
            let mut archive = tar::Archive::new(data.as_slice());
            let mut init_files = Vec::new();
            for entry in archive.entries().unwrap() {
                let mut entry = entry.unwrap();
                if entry.path_bytes() == b"rootfs/.podvm/podvm".as_slice() {
                    let mut data = Vec::new();
                    entry.read_to_end(&mut data).unwrap();
                    init_files.push(data);
                }
            }
            assert_eq!(init_files.len(), 1);
            assert_eq!(init_files[0].as_slice(), embedded);
            let plan = job.plan.get().unwrap();
            assert_eq!(plan.tree.bytes, (BLOCK + 3 + 4 + embedded.len()) as u64);
            assert_eq!(plan.tree.entries, 5);
            continue;
        }
        if what == "fallback" {
            assert!(!result.unwrap().1, "the fallback imported the image");
            assert_eq!((calls.ensured, calls.released), (1, 1));
            continue;
        }
        assert_eq!((calls.ensured, calls.released), (0, 0), "{what}");
        let body = captured.lock().unwrap();
        if matches!(what, "no room" | "bad digest") {
            let (line, _) = result.unwrap_err();
            let expected = if what == "no room" {
                "the storage pool `tiny`"
            } else {
                "digest mismatch"
            };
            assert!(line.contains(expected), "{what}: {line}");
            assert!(body.is_empty(), "{what}: no import reached incusd");
            continue;
        }
        let (name, cached) = result.unwrap();
        assert!(!cached);
        let plan = job.plan.get().unwrap();
        assert_eq!(plan.tree, need, "{what}");
        assert_eq!(plan.upload, compressed_bytes);
        assert_eq!(plan.freed, None);
        assert_eq!(super::held::read(&store, &name), Some(need));
        assert_eq!(body.starts_with(&[0x1f, 0x8b]), what == "gzip");
        if what == "gzip" {
            assert!(
                gunzip(&body) == plain_bytes,
                "the upload bytes after decompression"
            );
            continue;
        }
        plain_bytes.clone_from(&body);
        let mut archive = tar::Archive::new(body.as_slice());
        let mut data_path = None;
        let mut names = Vec::new();
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let path = entry.path().unwrap().into_owned();
            match path.to_str().unwrap() {
                "metadata.yaml" => {
                    let mut meta = String::new();
                    entry.read_to_string(&mut meta).unwrap();
                    assert!(meta.contains("creation_date: 1\n"));
                }
                "rootfs/.podvm/podvm" | "rootfs/.podvm/note" | "rootfs/bin/target" => {
                    let mut data = Vec::new();
                    entry.read_to_end(&mut data).unwrap();
                    match path.to_str().unwrap() {
                        "rootfs/.podvm/podvm" => {
                            assert_eq!(data, embedded);
                            assert_eq!(entry.header().mode().unwrap(), 0o755);
                        }
                        "rootfs/.podvm/note" => assert_eq!(data, b"image note"),
                        _ => assert_eq!(data, b"new"),
                    }
                }
                "rootfs/bin/first" | "rootfs/bin/second" => {
                    if entry.header().entry_type().is_hard_link() {
                        assert_eq!(entry.link_name().unwrap().as_deref(), data_path.as_deref());
                        assert!(data_path.is_some(), "the hard link follows its data");
                    } else {
                        assert_eq!(entry.header().gid().unwrap(), 42);
                        assert_eq!(entry.header().mode().unwrap(), 0o640);
                        let mut pax = entry.pax_extensions().unwrap().unwrap();
                        assert!(pax.any(|r| {
                            let r = r.unwrap();
                            r.key_bytes() == b"SCHILY.xattr.user.note" && r.value_bytes() == b"note"
                        }));
                        let mut data = Vec::new();
                        entry.read_to_end(&mut data).unwrap();
                        assert_eq!(data, b"old");
                        assert!(data_path.replace(path.clone()).is_none());
                    }
                }
                "rootfs" | "rootfs/bin" | "rootfs/.podvm" => {
                    assert!(entry.header().entry_type().is_dir());
                }
                _ => panic!("unexpected member {}", path.display()),
            }
            names.push(path);
        }
        assert_eq!(names.len(), 9, "{names:?}");
    }
    std::fs::remove_dir_all(d).unwrap();
}
