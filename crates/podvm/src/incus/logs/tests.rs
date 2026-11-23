//! TODO/cli.md T-2046: `logs --podvm-tier=incus` with `--limit-bytes`,
//! against a stand-in of the incus API on a unix socket.

use std::io::Write;
use std::os::unix::net::UnixListener;

use pod_incus::{Client, Settings};
use serde_json::json;

const LOG: &[u8] = b"one\ntwo\nthree\n";

/// The log of the instance `big`: three pieces of 64 KiB and a part, each
/// byte its offset modulo 251.
fn big() -> Vec<u8> {
    (0..200_000u32).map(|i| (i % 251) as u8).collect()
}

/// An incusd with the stopped instance `inc1` of podvm, whose log is
/// [`LOG`], and the instance `other` of another user.
fn stand_in(sock: &std::path::Path) {
    let l = UnixListener::bind(sock).unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            let (_, p, _) = crate::incus::tests::read_request(&mut s);
            let instance = |name: &str, config: serde_json::Value| {
                json!({"type": "sync", "metadata": {
                    "name": name, "status": "Stopped", "status_code": 102,
                    "type": "container", "config": config, "devices": {},
                    "profiles": [], "ephemeral": false, "stateful": false,
                    "architecture": "x86_64",
                }})
                .to_string()
                .into_bytes()
            };
            let (head, body) = if p.starts_with("/1.0/instances/inc1/files?") {
                ("X-Incus-type: file\r\n", LOG.to_vec())
            } else if p.starts_with("/1.0/instances/inc1") {
                ("", instance("inc1", json!({"user.podvm": "1"})))
            } else if p.starts_with("/1.0/instances/big/files?") {
                ("X-Incus-type: file\r\n", big())
            } else if p.starts_with("/1.0/instances/big") {
                ("", instance("big", json!({"user.podvm": "1"})))
            } else {
                ("", instance("other", json!({})))
            };
            let first = format!(
                "HTTP/1.1 200 OK\r\n{head}Content-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = s.write_all(first.as_bytes());
            let _ = s.write_all(&body);
        }
    });
}

#[test]
fn the_log_of_an_instance_stops_at_the_limit() {
    let d = crate::test_socket::SocketDir::new("t2046");
    let sock = d.join("unix.socket");
    stand_in(&sock);
    let c = Client::new(Settings::new(&sock));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let pause = std::time::Duration::from_millis(1);
    // (follow, limit, the bytes written)
    let cases: &[(bool, Option<u64>, &[u8])] = &[
        (false, None, LOG),
        (false, Some(1), b"o"),
        (false, Some(5), b"one\nt"),
        (false, Some(1000), LOG),
        (true, None, LOG),
        (true, Some(3), b"one"),
    ];
    for &(follow, limit, want) in cases {
        let mut out = Vec::new();
        rt.block_on(super::copy(&c, "inc1", follow, limit, pause, &mut out))
            .unwrap();
        assert_eq!(out, want, "follow={follow} limit={limit:?}");
    }
    // T-2046 (podcrates `v0.81.0`, #631): the read has no bound of
    // `max_body`; a log larger than it is copied in pieces, whole or to the
    // limit.
    let mut small = Settings::new(&sock);
    small.max_body = 4096;
    let small = Client::new(small);
    let log = big();
    for (follow, limit, want) in [
        (false, None, &log[..]),
        (false, Some(70_000), &log[..70_000]),
        (true, None, &log[..]),
        (true, Some(65_537), &log[..65_537]),
    ] {
        let mut out = Vec::new();
        rt.block_on(super::copy(&small, "big", follow, limit, pause, &mut out))
            .unwrap();
        assert!(
            out == want,
            "max_body 4096: follow={follow} limit={limit:?}: {} bytes",
            out.len()
        );
    }
    // An instance with no label of podvm is refused by name.
    let mut out = Vec::new();
    let e = rt
        .block_on(super::copy(&c, "other", false, None, pause, &mut out))
        .unwrap_err();
    assert!(
        matches!(&e, super::Fault::Incus(t) if t.contains("not an instance of podvm")),
        "{e:?}"
    );
}
