//! TODO/machine.md T-1846: the instance of a failed run goes, an instance
//! that stays is named, and `rm` of an ERROR instance names its state;
//! against a stand-in of the incus API on a unix socket.

use std::io::Write;
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};

use pod_incus::{Client, Settings};
use serde_json::json;

type Seen = Arc<Mutex<Vec<(String, String)>>>;

/// An incusd with one instance `inc1` in state ERROR, whose DELETE fails.
fn stand_in(sock: &std::path::Path) -> Seen {
    let l = UnixListener::bind(sock).unwrap();
    let seen: Seen = Arc::default();
    let log = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            let (m, p, _) = super::tests::read_request(&mut s);
            log.lock().unwrap().push((m.clone(), p.clone()));
            let sync = |v: serde_json::Value| (200, json!({"type": "sync", "metadata": v}));
            let (status, body) = match (m.as_str(), p.as_str()) {
                ("GET", p) if p.starts_with("/1.0/instances/inc1") && !p.contains("/state") => {
                    sync(json!({
                        "name": "inc1", "status": "Error", "status_code": 112,
                        "type": "container", "config": {"user.podvm": "1"},
                        "devices": {}, "profiles": [], "ephemeral": false,
                        "stateful": false, "architecture": "x86_64",
                    }))
                }
                ("GET", p) if p.starts_with("/1.0/instances?") => sync(json!([
                    {"name": "inc1", "status": "Running", "status_code": 103,
                     "type": "container", "created_at": "2026-11-15T06:00:00Z",
                     "config": {"user.podvm": "1", "user.podvm.image": "alpine:3.20"},
                     "devices": {}, "profiles": [], "ephemeral": false,
                     "stateful": false, "architecture": "x86_64"},
                    {"name": "other", "status": "Running", "status_code": 103,
                     "type": "container", "config": {}, "devices": {},
                     "profiles": [], "ephemeral": false, "stateful": false,
                     "architecture": "x86_64"},
                ])),
                ("PUT", _) => (
                    202,
                    json!({"type": "async", "operation": "/1.0/operations/st", "metadata": {}}),
                ),
                ("GET", p) if p.starts_with("/1.0/operations/") => {
                    sync(json!({"status_code": 200, "metadata": {}}))
                }
                ("DELETE", _) => (
                    400,
                    json!({"type": "error", "error": "Instance is busy", "error_code": 400}),
                ),
                _ => sync(json!({})),
            };
            let out = body.to_string();
            let head = format!(
                "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                out.len()
            );
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(out.as_bytes());
        }
    });
    seen
}

#[test]
fn incus_verbs_discard_and_rm_name_what_stays() {
    let d = std::env::temp_dir().join(format!("podvm-t1846-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let sock = d.join("unix.socket");
    let seen = stand_in(&sock);
    let c = Client::new(Settings::new(sock));
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    // A failed start: the instance is stopped and deleted, and one that
    // stays is named with the command that removes it.
    let left = rt.block_on(super::verbs::discard(&c, "inc1"));
    assert!(
        left.contains("the instance inc1 stays") && left.contains("incus delete --force inc1"),
        "{left}"
    );
    assert!(
        seen.lock()
            .unwrap()
            .iter()
            .any(|(m, p)| m == "DELETE" && p.starts_with("/1.0/instances/inc1")),
        "no DELETE: {:?}",
        seen.lock().unwrap()
    );
    // rm of an ERROR instance names the state, not "runs".
    let e = rt
        .block_on(super::verbs::one(&c, "rm", "inc1", false))
        .unwrap_err();
    assert!(e.contains("Error") && !e.contains("runs"), "{e}");
    // ps lists the labelled instance, and not the one of another user.
    let rows = rt.block_on(super::verbs::rows(&c, true)).unwrap();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0][0], "inc1");
    assert_eq!(rows[0][4], "Up");
    let _ = std::fs::remove_dir_all(&d);
}

/// PodUtils/podrunner#316 ask 1 (TODO/machine.md T-1996): the init of the
/// image gets an isolated map of 231,072 ids and `/dev/fuse`; podvm's own
/// init gets neither.
#[test]
fn the_init_of_the_image_gets_the_ids_and_the_fuse_of_the_snapshot() {
    use crate::incus::run::{fuse_device, snapshot_keys, SNAPSHOT_IDS};
    assert!(snapshot_keys(false).is_empty());
    let k = snapshot_keys(true);
    assert!(k.contains(&("security.idmap.isolated", "true".to_string())));
    assert!(k.contains(&("security.idmap.size", SNAPSHOT_IDS.to_string())));
    let (name, d) = fuse_device();
    assert_eq!(name, "podvm-fuse");
    assert_eq!(d.get("type").map(String::as_str), Some("unix-char"));
    assert_eq!(d.get("path").map(String::as_str), Some("/dev/fuse"));
    let e = "Failed initializing instance: Not enough uid/gid available".to_string();
    assert!(crate::incus::run::ids_hint(e.clone(), true).contains("/etc/subuid"));
    assert_eq!(crate::incus::run::ids_hint(e.clone(), false), e);
}
