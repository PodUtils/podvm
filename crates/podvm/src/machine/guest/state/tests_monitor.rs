//! TODO/machine.md T-2008 and T-2048: the save against a stand-in of the
//! monitor of an emulator that takes one client at a time, as QEMU does.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::time::Duration;

use super::tests::{put, scratch};
use super::*;

/// What a stand-in of the monitor of an emulator does with a `migrate`.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Peer {
    /// Each transport works.
    Works,
    /// `file:` is refused, as a QEMU before 8.2 does; the pipe works.
    NoFile,
    /// Each migration fails.
    Fails,
    /// The migration writes no byte.
    Empty,
    /// The connection ends at the `migrate`.
    Leaves,
    /// The guest does not go on after the save.
    NoCont,
    /// The first `query-migrate` gets its answer after the read bound of
    /// the monitor, on the same connection (the lock of the emulator,
    /// T-2008); a second client is taken only when the first one closed,
    /// as QEMU does (T-2048).
    Late,
    /// After the first `query-migrate` the emulator answers nothing and
    /// takes no client, and its queue of connections is full (T-2048).
    Hangs,
}

unsafe extern "C" {
    fn listen(fd: i32, backlog: i32) -> i32;
}

/// A listener on `sock` with the queue of the socket of QEMU's monitor:
/// one connection that is not taken yet (`listen` with a backlog of 1).
/// A `connect` to a full queue waits in the kernel.
fn qemu_listener(sock: &Path) -> UnixListener {
    use std::os::fd::AsRawFd;
    let l = UnixListener::bind(sock).unwrap();
    // SAFETY: a plain system call on the open descriptor of `l`.
    assert_eq!(unsafe { listen(l.as_raw_fd(), 1) }, 0);
    l
}

/// A monitor on `sock` that answers as `peer` says, one client at a time,
/// and gives the requests that it read. A monitor that hangs holds until
/// the sender gives a word or goes.
fn monitor(
    sock: &Path,
    peer: Peer,
) -> (
    std::thread::JoinHandle<Vec<String>>,
    std::sync::mpsc::Sender<()>,
) {
    let l = qemu_listener(sock);
    let own = sock.to_path_buf();
    let (release, held) = std::sync::mpsc::channel::<()>();
    let server = std::thread::spawn(move || {
        let mut seen = Vec::new();
        let mut status = "none".to_string();
        let mut late = peer == Peer::Late;
        let mut again = false;
        'conn: loop {
            let (s, _) = l.accept().unwrap();
            let mut w = s.try_clone().unwrap();
            // A client that closed in the queue is dropped, as QEMU does.
            if w.write_all(b"{\"QMP\": {\"version\": {}}}\n").is_err() {
                continue 'conn;
            }
            for line in BufReader::new(s).lines().map_while(Result::ok) {
                let v: serde_json::Value = serde_json::from_str(&line).unwrap();
                let verb = v["execute"].as_str().unwrap().to_string();
                seen.push(verb.clone());
                let mut answer = serde_json::json!({"return": {}});
                match verb.as_str() {
                    "migrate" => {
                        let uri = v["arguments"]["uri"].as_str().unwrap();
                        if peer == Peer::Leaves {
                            return seen;
                        }
                        let path = uri
                            .strip_prefix("file:")
                            .map(str::to_string)
                            .or_else(|| {
                                let quoted = uri.strip_prefix("exec:cat > '")?;
                                Some(quoted.trim_end_matches('\'').to_string())
                            })
                            .unwrap();
                        if peer == Peer::NoFile && uri.starts_with("file:") {
                            answer = serde_json::json!({"error": {"class": "GenericError",
                            "desc": "unknown migration protocol"}});
                        } else if peer == Peer::Fails {
                            std::fs::write(&path, b"half").unwrap();
                            status = "failed".to_string();
                        } else {
                            let bytes: &[u8] = if peer == Peer::Empty { b"" } else { b"STATE" };
                            std::fs::write(&path, bytes).unwrap();
                            status = "completed".to_string();
                        }
                    }
                    "query-migrate" if late => {
                        // The answer comes after the bound of the reader,
                        // on this connection; the next client waits.
                        late = false;
                        again = true;
                        std::thread::sleep(std::time::Duration::from_millis(5600));
                        answer = serde_json::json!({"return": {"status": status}});
                    }
                    "query-migrate" if peer == Peer::Hangs => {
                        // Two connections that nobody takes fill the queue.
                        let _full: Vec<_> = (0..2)
                            .filter_map(|_| std::os::unix::net::UnixStream::connect(&own).ok())
                            .collect();
                        let _ = held.recv();
                        return seen;
                    }
                    "query-migrate" => {
                        answer = serde_json::json!({"return": {"status": status,
                        "error-desc": "the disk is full"}});
                    }
                    "cont" if peer == Peer::NoCont => {
                        answer =
                            serde_json::json!({"error": {"class": "GenericError", "desc": "no"}});
                    }
                    _ => {}
                }
                if w.write_all(format!("{answer}\n").as_bytes()).is_err() {
                    break;
                }
            }
            // The client of the late answer closed: take the next one.
            if std::mem::take(&mut again) {
                continue 'conn;
            }
            break;
        }
        seen
    });
    (server, release)
}

/// A save leaves a whole state with its record, or nothing: no partial
/// file, and no record of a state that is not there. The guest goes on
/// after each save that the monitor answers.
#[test]
fn a_save_leaves_a_whole_state_or_nothing() {
    for (peer, first, want_ok, want_runs, transport, error) in [
        (
            Peer::Works,
            Transport::File,
            true,
            true,
            Transport::File,
            "",
        ),
        (
            Peer::Works,
            Transport::Pipe,
            true,
            true,
            Transport::Pipe,
            "",
        ),
        (
            Peer::NoFile,
            Transport::File,
            true,
            true,
            Transport::Pipe,
            "",
        ),
        (
            Peer::Fails,
            Transport::File,
            false,
            true,
            Transport::File,
            "the disk is full",
        ),
        (
            Peer::Empty,
            Transport::File,
            false,
            true,
            Transport::File,
            "wrote no byte",
        ),
        (
            Peer::Leaves,
            Transport::File,
            false,
            false,
            Transport::File,
            "qmp",
        ),
        (
            Peer::NoCont,
            Transport::File,
            true,
            false,
            Transport::File,
            "",
        ),
        (Peer::Late, Transport::File, true, true, Transport::File, ""),
        (
            Peer::Hangs,
            Transport::File,
            false,
            false,
            Transport::File,
            "no connection within",
        ),
    ] {
        let d = scratch("save");
        let f = files(&d, "sha256:aa", "print");
        // The record of an older state must not stay beside a new file.
        put(&f, "older", b"older state");
        let sock = d.join("qmp");
        let (server, release) = monitor(&sock, peer);
        // T-2048: each wait of a save has a bound, also where the emulator
        // takes no client: the save ends, or the test fails.
        let bounds = Bounds {
            save: Duration::from_secs(if peer == Peer::Hangs { 6 } else { 30 }),
            connect: Duration::from_secs(2),
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let (s2, f2) = (sock.clone(), f.clone());
        std::thread::spawn(move || {
            let _ = tx.send(save_within(&s2, &f2, "print", 3072, first, "u1", bounds));
        });
        let most = bounds.save + bounds.connect + Duration::from_secs(15);
        let (saved, runs) = rx
            .recv_timeout(most)
            .unwrap_or_else(|_| panic!("{peer:?}: the save did not end in {most:?}"));
        drop(release);
        let seen = server.join().unwrap();
        assert_eq!(saved.is_ok(), want_ok, "{saved:?}");
        assert_eq!(runs.is_ok(), want_runs, "{runs:?}");
        if let Err(why) = &runs {
            assert!(why.contains("cont"), "the request is not named: {why}");
        }
        if peer != Peer::Leaves {
            assert_eq!(seen.first().map(String::as_str), Some("qmp_capabilities"));
            assert!(
                seen.iter().position(|v| v == "stop") < seen.iter().position(|v| v == "migrate"),
                "the guest stops before the save: {seen:?}"
            );
        }
        if peer != Peer::Leaves && peer != Peer::Hangs {
            assert_eq!(seen.last().map(String::as_str), Some("cont"), "{seen:?}");
        }
        match &saved {
            Ok(s) => {
                assert_eq!(s.bytes, 5);
                assert_eq!(std::fs::read(&f.state).unwrap(), b"STATE");
                let meta = valid(&f, "print").expect("the record of the new state");
                assert_eq!((meta.mem_mib, meta.transport), (3072, transport));
            }
            Err(e) => {
                assert!(e.contains(error), "{error:?} is not in {e:?}");
                // Each failed migration names its transport, and the second
                // path after the first one.
                assert!(e.contains("through file"), "no transport in {e:?}");
                if peer == Peer::Fails {
                    assert!(e.contains("then through pipe"), "{e:?}");
                    assert!(e.contains("MiB written"), "no size in {e:?}");
                }
            }
        }
        let partial: Vec<_> = std::fs::read_dir(d.join("machine"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".part-"))
            .collect();
        assert!(partial.is_empty(), "a partial file stays: {partial:?}");
        let _ = std::fs::remove_dir_all(&d);
    }
}
