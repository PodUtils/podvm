use std::io::Write;
use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use super::*;

fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

#[test]
fn update_maps_only_the_live_limit_flags_as_run_does() {
    type Case<'a> = (&'a [&'a str], &'a [(&'a str, &'a str)]);
    let cases: &[Case<'_>] = &[
        (
            &["--podvm-tier=incus", "worker", "-m256m", "--pids-limit=64"],
            &[("limits.memory", "268435456B"), ("limits.processes", "64")],
        ),
        (
            &["--podvm-tier", "incus", "--cpus", "2", "--", "worker"],
            &[(CPU, "2")],
        ),
        (
            &["--podvm-tier=incus", "worker", "--cpus=2", "--cpus", "1.5"],
            &[(CPU_ALLOWANCE, "150%")],
        ),
        (
            &["--podvm-tier=incus", "worker", "--cpus=1.5", "--cpus=2"],
            &[(CPU, "2")],
        ),
    ];
    for (args, want) in cases {
        let parsed = parse(&words(args)).unwrap().unwrap();
        assert_eq!(parsed.name, "worker");
        assert_eq!(
            parsed.config,
            want.iter()
                .map(|&(k, v)| (k.to_string(), v.to_string()))
                .collect::<BTreeMap<_, _>>()
        );
    }
    assert!(parse(&words(&["--help"])).unwrap().is_none());
    for args in [
        &["worker", "--memory", "256m"][..],
        &["--podvm-tier=machine", "worker", "--memory=256m"],
        &["--podvm-tier=incus", "worker"],
        &["--podvm-tier=incus", "worker", "--cpus"],
        &["--podvm-tier=incus", "worker", "--pids-limit=0"],
        &["--podvm-tier=incus", "worker", "--memory=1k"],
        &["--podvm-tier=incus", "worker", "--storage-opt=size=1G"],
        &["--podvm-tier=incus", "worker", "other", "--memory=256m"],
        &["--podvm-tier=incus", "bad/name", "--memory=256m"],
    ] {
        assert_eq!(
            parse(&words(args)).unwrap_err(),
            EXIT_FLAG_ERROR,
            "{args:?}"
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Case {
    Saved,
    Refused,
    Conflict,
    NoEtag,
    WildcardEtag,
    Stopped,
    NotOurs,
    IgnoredMemory,
    IgnoredInheritedCpu,
    Replaced,
}

fn instance(case: Case) -> Value {
    let config = json!({"user.podvm": "1", "user.keep": "value", "volatile.uuid": "instance-1"});
    let mut i = json!({
        "name": "worker",
        "type": "container",
        "status": "Running",
        "created_at": "2026-11-26T03:00:00Z",
        "config": config,
        "expanded_config": config,
        "devices": {"root": {"type": "disk"}},
        "profiles": ["default"],
    });
    // The previous CPU count is inherited and must be cleared locally.
    i["expanded_config"][CPU] = json!("2");
    if case == Case::Stopped {
        i["status"] = json!("Stopped");
    }
    if case == Case::NotOurs {
        i["config"]["user.podvm"] = json!("0");
    }
    i
}

#[test]
fn update_needs_a_guarded_write_and_a_verified_read_back() {
    let request = parse(&words(&[
        "--podvm-tier=incus",
        "worker",
        "--memory=256m",
        "--cpus=1.5",
        "--pids-limit=64",
    ]))
    .unwrap()
    .unwrap();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for (case, expected, calls) in [
        (Case::Saved, Ok(()), &["GET", "PATCH", "GET"][..]),
        (
            Case::Refused,
            Err("limits.processes refused"),
            &["GET", "PATCH"],
        ),
        (
            Case::Conflict,
            Err("config changed during update (HTTP 412)"),
            &["GET", "PATCH"],
        ),
        (Case::NoEtag, Err("no usable ETag"), &["GET"]),
        (Case::WildcardEtag, Err("no usable ETag"), &["GET"]),
        (Case::Stopped, Err("must be Running"), &["GET"]),
        (
            Case::NotOurs,
            Err("did not return a podvm container"),
            &["GET"],
        ),
        (
            Case::IgnoredMemory,
            Err("limits.memory did not hold"),
            &["GET", "PATCH", "GET"],
        ),
        (
            Case::IgnoredInheritedCpu,
            Err("limits.cpu still limits CPU"),
            &["GET", "PATCH", "GET"],
        ),
        (
            Case::Replaced,
            Err("instance was replaced"),
            &["GET", "PATCH", "GET"],
        ),
    ] {
        let dir = crate::test_socket::SocketDir::new("t2093");
        let socket = dir.join("incus.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let done = Arc::clone(&stop);
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            let mut current = instance(case);
            let mut patched = false;
            while !done.load(Ordering::Acquire) {
                let (mut s, _) = match listener.accept() {
                    Ok(conn) => conn,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{case:?}: {e}"),
                };
                s.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
                let (headers, method, path, body) = super::super::tests::read_request_head(&mut s);
                assert_eq!(path, "/1.0/instances/worker");
                let (status, etag, body) = match method.as_str() {
                    "GET" => {
                        let etag = match case {
                            Case::NoEtag => "",
                            Case::WildcardEtag => "ETag: *\r\n",
                            _ if patched => "ETag: \"v2\"\r\n",
                            _ => "ETag: \"v1\"\r\n",
                        };
                        (200, etag, json!({"type": "sync", "metadata": current}))
                    }
                    "PATCH" => {
                        assert!(headers
                            .lines()
                            .any(|line| line.eq_ignore_ascii_case("If-Match: \"v1\"")));
                        let patch: Value = serde_json::from_slice(&body).unwrap();
                        assert_eq!(
                            patch,
                            json!({"config": {
                                "limits.memory": "268435456B",
                                "limits.cpu": "",
                                "limits.cpu.allowance": "150%",
                                "limits.processes": "64",
                            }})
                        );
                        match case {
                            Case::Refused => (
                                400,
                                "",
                                json!({"type": "error", "error_code": 400, "error": "limits.processes refused"}),
                            ),
                            Case::Conflict => (
                                412,
                                "",
                                json!({"type": "error", "error_code": 412, "error": "ETag differs"}),
                            ),
                            _ => {
                                for (k, v) in patch["config"].as_object().unwrap() {
                                    current["config"][k] = v.clone();
                                    current["expanded_config"][k] = v.clone();
                                }
                                match case {
                                    Case::IgnoredMemory => {
                                        current["config"]["limits.memory"] = json!("536870912B")
                                    }
                                    Case::IgnoredInheritedCpu => {
                                        current["expanded_config"][CPU] = json!("2")
                                    }
                                    Case::Replaced => {
                                        current["config"]["volatile.uuid"] = json!("instance-2")
                                    }
                                    _ => {}
                                }
                                patched = true;
                                (200, "", json!({"type": "sync", "metadata": {}}))
                            }
                        }
                    }
                    _ => panic!("{case:?}: unexpected request {method}"),
                };
                requests.push(method);
                write!(s, "HTTP/1.1 {status} X\r\n{etag}Content-Length: {}\r\nConnection: close\r\n\r\n{body}", body.to_string().len()).unwrap();
            }
            requests
        });
        let mut settings = pod_incus::Settings::new(&socket);
        settings.answer_timeout = Some(Duration::from_secs(2));
        let r = rt.block_on(apply(&Client::new(settings), &request));
        stop.store(true, Ordering::Release);
        assert_eq!(server.join().unwrap(), calls, "{case:?}");
        match expected {
            Ok(()) => assert_eq!(r, Ok(()), "{case:?}"),
            Err(text) => assert!(
                r.as_ref().is_err_and(|e| e.contains(text)),
                "{case:?}: {r:?}"
            ),
        }
    }
}
