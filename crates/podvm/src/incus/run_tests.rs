//! TODO/machine.md T-1718: one `run` against a stand-in incusd on a unix
//! socket: the import, the create, the start, the exec with its
//! websockets (T-2080) and the delete.

use std::io::Write;
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use super::run::Job;
use super::tests::{read_request_head, scratch};
/// One request the stand-in got: the method, the path, and the body.
type Seen = Arc<Mutex<Vec<(String, String, Vec<u8>)>>>;

/// The answer of the stand-in: a sync envelope, an async one, an error,
/// or raw bytes (chunked, as incus sends a log).
fn answer(method: &str, path: &str, inst: &str) -> (u16, Vec<u8>, bool) {
    let sync = |m: Value| {
        (
            200,
            json!({"type": "sync", "metadata": m})
                .to_string()
                .into_bytes(),
            false,
        )
    };
    let op = |id: &str| {
        let v =
            json!({"type": "async", "operation": format!("/1.0/operations/{id}"), "metadata": {}});
        (202, v.to_string().into_bytes(), false)
    };
    let done = |m: Value| sync(json!({"status_code": 200, "metadata": m}));
    let log = format!("/1.0/instances/{inst}/logs/exec-output/exec_1");
    match (method, path) {
        ("GET", "/1.0") => sync(json!({
            "api_extensions": pod_incus::NEEDED,
            "api_status": "stable",
            "api_version": "1.0",
            "auth": "trusted",
            "public": false,
            "auth_methods": [],
            "config": {},
            "environment": {"server_version": "6.0.0", "driver": "lxc", "server": "incus"},
        })),
        ("GET", p) if p.starts_with("/1.0/images/aliases/") => (
            404,
            br#"{"type":"error","error":"not found","error_code":404}"#.to_vec(),
            false,
        ),
        ("POST", p) if p.starts_with("/1.0/images/aliases") => sync(json!({})),
        ("POST", "/1.0/images") => op("import"),
        ("GET", p) if p.starts_with("/1.0/operations/import/wait") => {
            done(json!({"fingerprint": "f00d", "size": 1}))
        }
        ("POST", "/1.0/instances") => op("create"),
        ("POST", p) if p.ends_with("/exec") => op("exec"),
        ("GET", p) if p.starts_with("/1.0/operations/exec/wait") => done(json!({
            "return": 7,
            "output": {"1": format!("{log}.stdout"), "2": format!("{log}.stderr")},
        })),
        ("GET", p) if p.ends_with(".stdout") => (200, b"5\r\nhello\r\n0\r\n\r\n".to_vec(), true),
        ("GET", p) if p.ends_with(".stderr") => (200, b"0\r\n\r\n".to_vec(), true),
        ("DELETE", p) if p.contains("/logs/") => sync(json!({})),
        ("GET", p) if p.starts_with("/1.0/operations/") => done(json!({})),
        ("PUT", _) | ("DELETE", _) => op("state"),
        _ => sync(json!({})),
    }
}

fn stand_in(sock: &std::path::Path) -> (Seen, Arc<Mutex<super::attach_tests::Log>>) {
    let l = UnixListener::bind(sock).unwrap();
    let seen: Seen = Arc::default();
    let log = seen.clone();
    let ws = Arc::new(Mutex::new(super::attach_tests::Log::default()));
    let ws_log = ws.clone();
    std::thread::spawn(move || {
        let mut inst = String::new();
        let mut sockets = super::attach_tests::Sockets::default();
        for s in l.incoming() {
            let mut s = s.unwrap();
            let (head, m, p, body) = read_request_head(&mut s);
            // TODO/machine.md T-2080: the exec and its websockets.
            let plan = super::attach_tests::Plan::Exit7;
            if (p.ends_with("/exec") || p.starts_with("/1.0/operations/exec"))
                && super::attach_tests::serve_exec(
                    &mut s,
                    &head,
                    &m,
                    &p,
                    &body,
                    plan,
                    &mut sockets,
                    &ws_log,
                )
            {
                log.lock().unwrap().push((m, p, body));
                continue;
            }
            if m == "POST" && p.ends_with("/exec") {
                inst = p
                    .trim_start_matches("/1.0/instances/")
                    .trim_end_matches("/exec")
                    .to_string();
            }
            let (status, out, chunked) = answer(&m, &p, &inst);
            log.lock().unwrap().push((m, p, body));
            let te = if chunked {
                "Transfer-Encoding: chunked\r\n"
            } else {
                ""
            };
            let head = format!("HTTP/1.1 {status} X\r\n{te}Connection: close\r\n\r\n");
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(&out);
        }
    });
    (seen, ws)
}

#[test]
fn a_run_imports_creates_pushes_starts_execs_and_deletes() {
    let d = scratch("run");
    std::fs::create_dir_all(d.join("root/bin")).unwrap();
    std::fs::write(d.join("root/bin/sh"), b"#!sh").unwrap();
    let sd = crate::test_socket::SocketDir::new("t1718");
    let sock = sd.join("unix.socket");
    let (seen, ws) = stand_in(&sock);
    // SAFETY: only this test reads PODVM_INCUS_SOCKET.
    unsafe { std::env::set_var("PODVM_INCUS_SOCKET", &sock) };
    let code = super::run::run(&Job {
        verb: "run",
        rootfs: &d.join("root"),
        digest: "sha256:00112233445566778899",
        argv: vec!["echo".into(), "hello".into()],
        env: vec!["PATH=/bin".into()],
        workdir: "/".into(),
        stdin: Some(b"in".to_vec()),
        limits: vec![("limits.processes", "64".into())],
        volumes: vec![
            ("/h/ws".into(), "/workspace".into(), false),
            ("/h/cfg".into(), "/cfg".into(), true),
        ],
        detach: None,
        net_none: false,
        channel: None,
        store: None,
        tree: None,
        plan: std::sync::OnceLock::new(),
        name: None,
    });
    unsafe { std::env::remove_var("PODVM_INCUS_SOCKET") };
    assert_eq!(code, 7, "the exit code of the command");
    let seen = seen.lock().unwrap().clone();
    let calls: Vec<String> = seen.iter().map(|(m, p, _)| format!("{m} {p}|")).collect();
    let at = |want: &str| {
        calls
            .iter()
            .position(|c| c.starts_with(want))
            .unwrap_or_else(|| panic!("{want} in {calls:#?}"))
    };
    let order = [
        "POST /1.0/images",
        "POST /1.0/images/aliases",
        "POST /1.0/instances|",
        "PUT /1.0/instances/podvm-",
        "POST /1.0/instances/podvm-",
        "DELETE /1.0/instances/podvm-",
    ];
    let pos: Vec<usize> = order.iter().map(|w| at(w)).collect();
    assert!(pos.windows(2).all(|w| w[0] < w[1]), "{calls:#?}");
    let body = |want: &str| -> Value {
        let (_, _, b) = &seen[at(want)];
        serde_json::from_slice(b).unwrap()
    };
    let create = body("POST /1.0/instances|");
    assert_eq!(
        create["config"]["raw.lxc"],
        "lxc.init.cmd = /.podvm/podvm guest-hold"
    );
    // TODO/machine.md T-1897: a limit of the run is a key of the instance.
    assert_eq!(create["config"]["limits.processes"], "64");
    assert_eq!(body("POST /1.0/images/aliases")["target"], "f00d");
    // TODO/machine.md T-1722: each -v is a disk device; :ro is read-only.
    assert_eq!(
        create["devices"],
        json!({
            "podvm-vol0": {"type": "disk", "source": "/h/ws", "path": "/workspace"},
            "podvm-vol1": {"type": "disk", "source": "/h/cfg", "path": "/cfg", "readonly": "true"}
        })
    );
    let exec = seen
        .iter()
        .find(|(m, p, _)| m == "POST" && p.ends_with("/exec"))
        .map(|(_, _, b)| serde_json::from_slice::<Value>(b).unwrap())
        .unwrap();
    // TODO/machine.md T-2080: podvm in the image starts the command; the
    // stdin goes by its websocket, not as a file.
    assert_eq!(
        exec["command"],
        json!([
            "/.podvm/podvm",
            "guest-exec",
            "--cwd",
            "/",
            "--",
            "echo",
            "hello"
        ])
    );
    assert_eq!(exec["environment"], json!({"PATH": "/bin"}));
    assert_eq!(exec["record-output"], false);
    assert_eq!(exec["wait-for-websocket"], true);
    assert!(
        !calls
            .iter()
            .any(|c| c.ends_with("files?path=/.podvm/stdin|")),
        "{calls:#?}"
    );
    // TODO/machine.md T-1722: podvm is in the image, not pushed for a run.
    assert!(
        !calls
            .iter()
            .any(|c| c.ends_with("files?path=/.podvm/podvm|")),
        "{calls:#?}"
    );
    assert_eq!(ws.lock().unwrap().stdin, b"in");
    let tars = std::fs::read_dir(d.join("incus"))
        .map(|r| r.count())
        .unwrap_or(0);
    assert_eq!(tars, 0, "the tarball of the import goes");
    let _ = std::fs::remove_dir_all(&d);
}

/// TODO/machine.md T-2082 (PodUtils/podrunner#347 ask 2): an incusd that
/// closes the connection during the upload of the image gives an error and
/// the line of a cut upload, not a SIGPIPE that ends podvm.
#[test]
fn an_upload_that_incusd_cuts_gives_a_line_not_a_sigpipe() {
    use std::io::Read;
    let d = scratch("cut");
    std::fs::create_dir_all(d.join("root/bin")).unwrap();
    // T-2085: bytes that the gzip of the upload cannot shrink, so that the
    // upload is longer than the buffers of the socket.
    let mut x = 0x9e37_79b9_7f4a_7c15u64;
    let big: Vec<u8> = (0..4 << 20)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x as u8
        })
        .collect();
    std::fs::write(d.join("root/bin/big"), big).unwrap();
    let sd = crate::test_socket::SocketDir::new("t2082");
    let sock = sd.join("unix.socket");
    let l = UnixListener::bind(&sock).unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            // The head only: a POST of the image is cut after it.
            let mut head = Vec::new();
            let mut b = [0u8; 1];
            while !head.ends_with(b"\r\n\r\n") && s.read(&mut b).unwrap_or(0) == 1 {
                head.push(b[0]);
            }
            let head = String::from_utf8_lossy(&head).into_owned();
            if head.starts_with("POST /1.0/images ") {
                drop(s);
                continue;
            }
            let body = if head.starts_with("GET /1.0/profiles/default") {
                json!({"type": "sync", "metadata": {"name": "default", "config": {},
                    "devices": {"root": {"type": "disk", "path": "/", "pool": "bt"}}}})
            } else {
                json!({"type": "error", "error_code": 404, "error": "not found"})
            };
            let status = if body["type"] == "sync" { 200 } else { 404 };
            let out = body.to_string();
            let _ = s.write_all(
                format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}",
                    out.len()
                )
                .as_bytes(),
            );
        }
    });
    let job = Job {
        verb: "create",
        rootfs: &d.join("root"),
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
        tree: None,
        plan: std::sync::OnceLock::new(),
        name: None,
    };
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let c = pod_incus::Client::new(pod_incus::Settings::new(&sock));
    let r = rt.block_on(super::import::ensure_image(&c, &job, "btrfs"));
    let (line, hint) = r.expect_err("a cut upload is a failure");
    assert!(line.starts_with("POST /1.0/images"), "{line}");
    let hint = hint.unwrap_or_default();
    assert!(
        hint.contains("incusd closed the connection during the upload of the image"),
        "{line} / {hint}"
    );
    let _ = std::fs::remove_dir_all(&d);
}
