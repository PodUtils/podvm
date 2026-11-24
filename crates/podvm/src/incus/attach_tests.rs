//! TODO/machine.md T-2080: a command of the incus tier with its websockets,
//! against a stand-in incusd that speaks them (the upgrade, the frames, the
//! barrier at the end of each stream, the control socket), and the start of
//! a command by podvm in the image. One table of the ends of a command
//! (OR-B39-01, OR-B39-03, OR-B39-05, OR-B39-06).

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::json;

use super::attach::{code_of_failure, exec_streamed, Outcome, Signals};
use super::guest_exec;

/// What the command of the stand-in does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Plan {
    /// Writes "out" and "err", echoes its stdin to stdout, ends with 7.
    Exit7,
    /// Writes "before", ends with 127: the wait answers incusd's error.
    NotFound,
    /// Ends with 143 at the first signal.
    Trap,
    /// Ignores each signal but SIGKILL; ends with 137.
    Ignore,
}

/// What the stand-in saw.
#[derive(Debug, Default)]
pub(super) struct Log {
    pub stdin: Vec<u8>,
    pub signals: Vec<i32>,
    pub calls: Vec<String>,
    pub exec_body: Option<serde_json::Value>,
}

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

fn accept_key(key: &str) -> String {
    use base64::Engine;
    use sha1::Digest;
    let sum = sha1::Sha1::digest(format!("{key}{GUID}").as_bytes());
    base64::engine::general_purpose::STANDARD.encode(sum)
}

/// One frame of the server: no mask.
fn send(s: &mut UnixStream, opcode: u8, payload: &[u8]) {
    let mut f = vec![0x80 | opcode];
    if payload.len() < 126 {
        f.push(payload.len() as u8);
    } else {
        f.push(126);
        f.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    }
    f.extend_from_slice(payload);
    let _ = s.write_all(&f);
}

/// One frame of the client (masked): its opcode and payload; `None` at
/// the end of the connection.
fn recv(s: &mut UnixStream) -> Option<(u8, Vec<u8>)> {
    let mut h = [0u8; 2];
    s.read_exact(&mut h).ok()?;
    let mut len = u64::from(h[1] & 0x7f);
    if len == 126 {
        let mut b = [0u8; 2];
        s.read_exact(&mut b).ok()?;
        len = u64::from(u16::from_be_bytes(b));
    } else if len == 127 {
        let mut b = [0u8; 8];
        s.read_exact(&mut b).ok()?;
        len = u64::from_be_bytes(b);
    }
    let mut mask = [0u8; 4];
    if h[1] & 0x80 != 0 {
        s.read_exact(&mut mask).ok()?;
    }
    let mut p = vec![0u8; len as usize];
    s.read_exact(&mut p).ok()?;
    for (i, b) in p.iter_mut().enumerate() {
        *b ^= mask[i % 4];
    }
    Some((h[0] & 0x0f, p))
}

/// The answer of an HTTP request of the stand-in.
fn reply(s: &mut UnixStream, status: u16, body: &serde_json::Value) {
    let b = body.to_string();
    let head = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n",
        b.len()
    );
    let _ = s.write_all(head.as_bytes());
    let _ = s.write_all(b.as_bytes());
}

/// The websockets of the operation `exec` of the stand-in, by secret.
#[derive(Default)]
pub(super) struct Sockets(BTreeMap<String, UnixStream>);

/// Serve one request of the exec of `plan`, where it is one: `true` where
/// it was.
#[allow(clippy::too_many_arguments)]
pub(super) fn serve_exec(
    s: &mut UnixStream,
    head: &str,
    method: &str,
    path: &str,
    body: &[u8],
    plan: Plan,
    sockets: &mut Sockets,
    log: &Arc<Mutex<Log>>,
) -> bool {
    log.lock().unwrap().calls.push(format!("{method} {path}"));
    if method == "POST" && path.ends_with("/exec") {
        log.lock().unwrap().exec_body = serde_json::from_slice(body).ok();
        let op = json!({"id": "exec", "class": "websocket", "status_code": 103,
            "metadata": {"fds": {"0": "s0", "1": "s1", "2": "s2", "control": "sc"}}});
        reply(
            s,
            202,
            &json!({"type": "async", "operation": "/1.0/operations/exec", "metadata": op}),
        );
        return true;
    }
    if method == "GET" && path.starts_with("/1.0/operations/exec/websocket?secret=") {
        let secret = path.rsplit('=').next().unwrap_or_default().to_string();
        let key = head
            .lines()
            .find_map(|l| {
                let (k, v) = l.split_once(':')?;
                k.eq_ignore_ascii_case("sec-websocket-key")
                    .then(|| v.trim().to_string())
            })
            .unwrap_or_default();
        let answer = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Accept: {}\r\n\r\n",
            accept_key(&key)
        );
        let _ = s.write_all(answer.as_bytes());
        sockets.0.insert(secret, s.try_clone().unwrap());
        if ["s0", "s1", "s2", "sc"]
            .iter()
            .all(|k| sockets.0.contains_key(*k))
        {
            act(plan, sockets, log);
        }
        return true;
    }
    if method == "GET" && path.starts_with("/1.0/operations/exec/wait") {
        let code = match plan {
            Plan::Exit7 => 7,
            Plan::Trap => 143,
            Plan::Ignore => 137,
            Plan::NotFound => {
                reply(
                    s,
                    200,
                    &json!({"type": "error", "error_code": 400, "error": "Command not found"}),
                );
                return true;
            }
        };
        reply(
            s,
            200,
            &json!({"type": "sync", "metadata": {"id": "exec", "status_code": 200,
                "metadata": {"return": code}}}),
        );
        return true;
    }
    if method == "GET" && path.starts_with("/1.0/operations/exec") {
        reply(
            s,
            200,
            &json!({"type": "sync", "metadata": {"id": "exec", "status_code": 400,
                "err": "Command not found", "metadata": {"return": 127}}}),
        );
        return true;
    }
    false
}

/// The command of `plan`, once its four sockets are there.
fn act(plan: Plan, sockets: &mut Sockets, log: &Arc<Mutex<Log>>) {
    let mut take = |k: &str| sockets.0.remove(k).unwrap();
    let (mut s0, mut s1, mut s2, mut sc) = (take("s0"), take("s1"), take("s2"), take("sc"));
    match plan {
        Plan::NotFound => send(&mut s1, 2, b"before\n"),
        _ => {
            send(&mut s1, 2, b"out\n");
            send(&mut s2, 2, b"err\n");
        }
    }
    // The stdin to its end (an empty text message, or a Close).
    while let Some((op, p)) = recv(&mut s0) {
        if op == 8 || (op == 1 && p.is_empty()) {
            break;
        }
        if op == 2 {
            log.lock().unwrap().stdin.extend_from_slice(&p);
        }
    }
    if plan == Plan::Exit7 {
        let echo = log.lock().unwrap().stdin.clone();
        send(&mut s1, 2, &echo);
    }
    if matches!(plan, Plan::Trap | Plan::Ignore) {
        while let Some((op, p)) = recv(&mut sc) {
            if op != 1 {
                continue;
            }
            let v: serde_json::Value = serde_json::from_slice(&p).unwrap_or_default();
            let n = v["signal"].as_i64().unwrap_or(0) as i32;
            log.lock().unwrap().signals.push(n);
            if plan == Plan::Trap || n == 9 {
                break;
            }
        }
    }
    // The barrier of each output, then the sockets close with no Close.
    send(&mut s1, 1, b"");
    send(&mut s2, 1, b"");
}

/// A stand-in that serves the exec of `plan` and answers 404 to the rest.
fn stand(sock: &std::path::Path, plan: Plan) -> Arc<Mutex<Log>> {
    let l = UnixListener::bind(sock).unwrap();
    let log = Arc::new(Mutex::new(Log::default()));
    let seen = log.clone();
    std::thread::spawn(move || {
        let mut sockets = Sockets::default();
        for s in l.incoming() {
            let mut s = s.unwrap();
            let (head, m, p, body) = super::tests::read_request_head(&mut s);
            if !serve_exec(&mut s, &head, &m, &p, &body, plan, &mut sockets, &seen) {
                reply(
                    &mut s,
                    404,
                    &json!({"type": "error", "error_code": 404, "error": "not found"}),
                );
            }
        }
    });
    log
}

/// The outcome, stdout, stderr and the log of one exec.
type Ran = (Result<Outcome, String>, Vec<u8>, Vec<u8>, Arc<Mutex<Log>>);

/// One exec of `plan` with `input` and the signals of `script`: the
/// outcome, stdout, stderr and the log of the stand-in.
fn one(plan: Plan, input: &[u8], script: &[(Duration, i32)]) -> Ran {
    let sd = crate::test_socket::SocketDir::new("t2079");
    let sock = sd.join("unix.socket");
    let log = stand(&sock, plan);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = rt.block_on(async {
        let c = pod_incus::Client::new(pod_incus::Settings::new(&sock));
        let mut sig = Signals::scripted(script);
        let x = pod_incus::Exec::new(["cat"]);
        exec_streamed(&c, "run", "i1", &x, input, &mut sig, &mut out, &mut err).await
    });
    (r, out, err, log)
}

/// Each end of a command, in one table: its exit code, its output, its
/// stdin, and the signals that reached it.
#[test]
fn each_end_of_a_command_gives_its_code_its_output_and_its_signals() {
    let ms = Duration::from_millis;
    struct Case {
        plan: Plan,
        input: &'static [u8],
        script: Vec<(Duration, i32)>,
        code: i32,
        out: &'static [u8],
        signals: &'static [i32],
    }
    let cases = [
        // OR-B39-05 and 06: stdin goes in, the streams stay apart.
        Case {
            plan: Plan::Exit7,
            input: b"in",
            script: vec![],
            code: 7,
            out: b"out\nin",
            signals: &[],
        },
        // OR-B39-01: an end with 127 is the code and the output of the
        // command, not a refusal of incusd.
        Case {
            plan: Plan::NotFound,
            input: b"",
            script: vec![],
            code: 127,
            out: b"before\n",
            signals: &[],
        },
        // OR-B39-03: the first signal goes to the command.
        Case {
            plan: Plan::Trap,
            input: b"",
            script: vec![(ms(100), 15)],
            code: 143,
            out: b"out\n",
            signals: &[15],
        },
        // A second signal: SIGKILL goes to the command.
        Case {
            plan: Plan::Ignore,
            input: b"",
            script: vec![(ms(100), 15), (ms(100), 2)],
            code: 137,
            out: b"out\n",
            signals: &[15, 9],
        },
    ];
    for c in cases {
        let (r, out, err, log) = one(c.plan, c.input, &c.script);
        let o = r.unwrap_or_else(|e| panic!("{:?}: {e}", c.plan));
        assert_eq!(o.code, c.code, "{:?}", c.plan);
        assert_eq!(out, c.out, "{:?}", c.plan);
        let log = log.lock().unwrap();
        assert_eq!(log.stdin, c.input, "{:?}", c.plan);
        assert_eq!(log.signals, c.signals, "{:?}", c.plan);
        if c.plan != Plan::NotFound {
            assert_eq!(err, b"err\n", "{:?}", c.plan);
        }
        let body = log.exec_body.clone().unwrap();
        assert_eq!(
            (
                body["wait-for-websocket"].as_bool(),
                body["record-output"].as_bool()
            ),
            (Some(true), Some(false)),
            "{:?}",
            c.plan
        );
    }
}

/// incusd's two words of a failed exec are 127 and 126; the `return` of
/// the operation wins where it is a code; another failure is no code.
#[test]
fn the_words_of_a_failed_exec_give_its_code() {
    for (words, ret, want) in [
        ("Command not found", Some(127), Some(127)),
        ("Command not found", None, Some(127)),
        ("Command not executable", Some(126), Some(126)),
        ("Command not executable", Some(-1), Some(126)),
        ("Command not executable", Some(300), Some(126)),
        ("Instance is not running", Some(1), None),
        ("", None, None),
    ] {
        assert_eq!(code_of_failure(words, ret), want, "{words:?} {ret:?}");
    }
}

/// podvm in the image: the argv of a run, its parse, the search of the
/// program and the line and code of each start that fails (OR-B39-01,
/// OR-B39-09, OR-B39-16).
#[test]
fn podvm_in_the_image_names_the_cause_of_a_start_that_fails() {
    use std::os::unix::fs::PermissionsExt;
    let argv = vec!["sh".to_string(), "-c".to_string(), "a b".to_string()];
    let w = guest_exec::wrap(&argv, "/w d");
    assert_eq!(
        &w[..5],
        ["/.podvm/podvm", "guest-exec", "--cwd", "/w d", "--"]
    );
    let (cwd, rest) = guest_exec::parse(&w[2..]).unwrap();
    assert_eq!((cwd.as_str(), rest), ("/w d", argv.as_slice()));
    assert!(guest_exec::parse(&["x".to_string()]).is_err());

    let d = std::env::temp_dir().join(format!("podvm-t2079-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("a")).unwrap();
    std::fs::create_dir_all(d.join("b")).unwrap();
    std::fs::write(d.join("a/plain"), b"x").unwrap();
    std::fs::write(d.join("b/plain"), b"x").unwrap();
    std::fs::set_permissions(d.join("b/plain"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths([d.join("a"), d.join("b")]).unwrap();
    assert_eq!(
        guest_exec::resolve("plain", Some(&path)),
        Some(d.join("b/plain")),
        "the first file with an execute bit"
    );
    assert_eq!(guest_exec::resolve("nosuch", Some(&path)), None);
    std::fs::remove_file(d.join("b/plain")).unwrap();
    assert_eq!(
        guest_exec::resolve("plain", Some(&path)),
        Some(d.join("a/plain")),
        "a file with no bit: its start gives EACCES"
    );
    let _ = std::fs::remove_dir_all(&d);

    assert_eq!(
        guest_exec::not_found("nosuchcmd"),
        (
            127,
            "\"nosuchcmd\": executable file not found in $PATH".to_string()
        )
    );
    assert_eq!(guest_exec::not_found("/x/y").0, 127);
    let mut arm = vec![0u8; 20];
    arm[..4].copy_from_slice(b"\x7fELF");
    arm[5] = 1;
    arm[18..20].copy_from_slice(&183u16.to_le_bytes());
    assert_eq!(guest_exec::elf_arch(&arm), Some("aarch64"));
    assert_eq!(guest_exec::elf_arch(b"#!/bin/sh\n0123456789"), None);
    let e = |n| std::io::Error::from_raw_os_error(n);
    let f = std::path::Path::new("/bin/true");
    for (errno, head, code, has) in [
        (
            8,
            &arm[..],
            126,
            "an ELF for aarch64, and this machine runs x86_64",
        ),
        (8, &b"#!x"[..], 126, "exec format error"),
        (13, &b""[..], 126, "permission denied"),
        (2, &b""[..], 127, "its interpreter or its loader is not"),
        (20, &b""[..], 126, "/bin/true"),
    ] {
        let (c, line) = guest_exec::explain("true", f, &e(errno), head, "x86_64");
        assert_eq!(c, code, "{errno}: {line}");
        assert!(line.contains(has), "{errno}: {line}");
    }
}
