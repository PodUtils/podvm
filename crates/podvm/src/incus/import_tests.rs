//! TODO/machine.md T-2085 (PodUtils/podrunner#347 ask 1): the gzip of the
//! upload, and the tree of an import against a stand-in incusd: made for
//! an import only, removed after it, also after a failure.

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};

use serde_json::json;

use super::gzip::{Members, BLOCK};
use super::import::{ensure_image, Tree};
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

/// One rule for each size and each count of threads: the members are one
/// gzip file that gives the input back, and the bytes do not depend on the
/// threads (the fingerprint of T-2081 stays the same for the same tarball).
#[test]
fn the_members_give_the_input_back_with_the_same_bytes_for_each_thread_count() {
    let sizes = [0, 1, 4096, BLOCK - 1, BLOCK, BLOCK + 1, 3 * BLOCK + 5];
    for size in sizes {
        let input: Vec<u8> = noise(size / 2, size as u64)
            .into_iter()
            .chain(std::iter::repeat_n(b'a', size - size / 2))
            .collect();
        let mut first: Option<Vec<u8>> = None;
        for threads in [1, 2, 3, 8, 64] {
            let mut w = Members::new(Vec::new(), threads);
            // Writes of odd sizes, so that the cut of the blocks is the
            // writer's, not the caller's.
            for chunk in input.chunks(7919) {
                w.write_all(chunk).unwrap();
                w.flush().unwrap();
            }
            let out = w.finish().unwrap();
            assert!(gunzip(&out) == input, "size {size}, {threads} threads");
            match &first {
                None => first = Some(out),
                Some(f) => assert!(&out == f, "size {size}: {threads} threads, other bytes"),
            }
        }
    }
}

/// A writer that fails after `left` bytes, as a pipe that incusd closed.
struct Failing {
    left: usize,
}

impl Write for Failing {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.left == 0 {
            return Err(std::io::ErrorKind::BrokenPipe.into());
        }
        let n = buf.len().min(self.left);
        self.left -= n;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn a_reader_that_goes_gives_the_error_of_the_pipe() {
    let input = noise(3 * BLOCK, 9);
    for left in [0, 100, BLOCK] {
        let mut w = Members::new(Failing { left }, 4);
        let wrote = w.write_all(&input);
        let e = match wrote {
            Err(e) => e,
            Ok(()) => w.finish().err().expect("an error"),
        };
        assert_eq!(e.kind(), std::io::ErrorKind::BrokenPipe, "left {left}");
    }
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
    root: &'a std::path::Path,
    fresh: bool,
    fail: bool,
    calls: Mutex<Calls>,
}

impl Tree for Stub<'_> {
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
fn stand_in(sock: &std::path::Path, alias: bool, refuse: bool) -> Arc<Mutex<Vec<u8>>> {
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
        let upload = stand_in(&sock, *alias, *refuse);
        let stub = Stub {
            root: &root,
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
