//! TODO/cli.md T-2087 (OR-B40-04, OR-B40-18) and T-2079 (OR-B39-09,
//! OR-B39-11, OR-B39-17): `wait`, `kill`, the state `Created`, the kind of a
//! path after its links, and the lines of a missing log, a missing working
//! directory and a missing init; against a stand-in of incusd whose
//! instances and files each test gives.

use std::collections::HashMap;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pod_incus::{Client, FileKind, Settings};
use serde_json::json;

use super::waitkill::{kill_one, signal_of, wait_one, Pace, GUEST_ENDED};
use super::walk::{find, join_link, normal, Found};

/// The instances and the files of the stand-in.
#[derive(Default)]
struct World {
    /// The states of each instance, one for each read; the last one stays.
    /// `Gone` is a 404.
    states: HashMap<String, Vec<&'static str>>,
    ephemeral: Vec<String>,
    /// `(instance, path)`: the kind and the bytes (a link: its target).
    files: HashMap<(String, String), (&'static str, Vec<u8>)>,
    /// The exit code and the stderr of `podvm guest-kill` in an instance.
    guest: HashMap<String, (i32, &'static str)>,
    /// The body of each exec.
    execs: Vec<serde_json::Value>,
}

type Shared = Arc<Mutex<World>>;

fn instance_json(name: &str, state: &str, ephemeral: bool) -> serde_json::Value {
    let (status, used) = match state {
        "Created" => ("Stopped", "1970-01-01T00:00:00Z"),
        s => (s, "2026-11-26T08:00:00Z"),
    };
    let label = match name.starts_with("other") {
        true => json!({}),
        false => json!({"user.podvm": "1"}),
    };
    json!({"name": name, "status": status, "status_code": 102, "type": "container",
           "config": label, "devices": {}, "profiles": [], "ephemeral": ephemeral,
           "stateful": false, "architecture": "x86_64", "last_used_at": used,
           "created_at": "2026-11-26T07:00:00Z"})
}

fn reply(s: &mut std::os::unix::net::UnixStream, status: u16, head: &[(&str, &str)], body: &[u8]) {
    let mut h = format!(
        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (k, v) in head {
        h.push_str(&format!("{k}: {v}\r\n"));
    }
    h.push_str("\r\n");
    let _ = s.write_all(h.as_bytes());
    let _ = s.write_all(body);
}

fn stand_in(sock: &std::path::Path, world: Shared) {
    let l = UnixListener::bind(sock).unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let mut s = s.unwrap();
            let (m, p, body) = super::tests::read_request(&mut s);
            let (route, query) = p.split_once('?').unwrap_or((p.as_str(), ""));
            let parts: Vec<&str> = route.trim_start_matches("/1.0/").split('/').collect();
            let sync = |v: serde_json::Value| json!({"type": "sync", "metadata": v}).to_string();
            let missing =
                json!({"type": "error", "error": "Not Found", "error_code": 404}).to_string();
            let mut w = world.lock().unwrap();
            match (m.as_str(), parts.as_slice()) {
                ("GET", ["instances", name]) => {
                    let eph = w.ephemeral.iter().any(|e| e == name);
                    let list = w.states.entry(name.to_string()).or_default();
                    let state = match list.len() {
                        0 => "Gone",
                        1 => list[0],
                        _ => list.remove(0),
                    };
                    match state {
                        "Gone" => reply(&mut s, 404, &[], missing.as_bytes()),
                        st => reply(
                            &mut s,
                            200,
                            &[],
                            sync(instance_json(name, st, eph)).as_bytes(),
                        ),
                    }
                }
                ("GET", ["instances", name, "files"]) => {
                    let path = query.trim_start_matches("path=").to_string();
                    match w.files.get(&(name.to_string(), path)) {
                        Some(("directory", _)) => {
                            let b = sync(json!(["a"]));
                            reply(&mut s, 200, &[("X-Incus-Type", "directory")], b.as_bytes())
                        }
                        Some((kind, b)) => {
                            let b = b.clone();
                            reply(&mut s, 200, &[("X-Incus-Type", kind)], &b)
                        }
                        None => reply(&mut s, 404, &[], missing.as_bytes()),
                    }
                }
                ("POST", ["instances", name, "exec"]) => {
                    w.execs.push(serde_json::from_slice(&body).unwrap());
                    let op = format!("/1.0/operations/x-{name}");
                    let b = json!({"type": "async", "operation": op, "metadata": {}}).to_string();
                    reply(&mut s, 202, &[], b.as_bytes())
                }
                ("GET", ["operations", id, "wait"]) => {
                    let name = id.trim_start_matches("x-");
                    let code = w.guest.get(name).map_or(0, |g| g.0);
                    let logs = format!("/1.0/instances/{name}/logs/exec-output");
                    let op = json!({"id": id, "status_code": 200, "metadata": {"return": code,
                        "output": {"1": format!("{logs}/o1"), "2": format!("{logs}/o2")}}});
                    reply(&mut s, 200, &[], sync(op).as_bytes())
                }
                ("GET", ["instances", name, "logs", "exec-output", f]) => {
                    let err = w.guest.get(*name).map_or("", |g| g.1);
                    let b = if *f == "o2" { err.as_bytes() } else { b"" };
                    reply(&mut s, 200, &[], b)
                }
                _ => reply(&mut s, 200, &[], sync(json!({})).as_bytes()),
            }
        }
    });
}

fn world() -> (
    Client,
    Shared,
    tokio::runtime::Runtime,
    crate::test_socket::SocketDir,
) {
    let d = crate::test_socket::SocketDir::new("t2087");
    let sock = d.join("unix.socket");
    let w: Shared = Arc::default();
    stand_in(&sock, w.clone());
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    (Client::new(Settings::new(sock)), w, rt, d)
}

fn file(w: &Shared, inst: &str, path: &str, kind: &'static str, body: &[u8]) {
    let key = (inst.to_string(), path.to_string());
    w.lock().unwrap().files.insert(key, (kind, body.to_vec()));
}

/// One rule for each end of an instance: `wait` prints the exit code that
/// the init wrote once the instance stopped; each other end is a refusal
/// with its code and a word of the reason.
#[test]
fn wait_gives_the_exit_code_of_each_end_or_says_why_there_is_none() {
    let (c, w, rt, _d) = world();
    let pace = Pace {
        pause: Duration::from_millis(1),
        settle: Duration::from_millis(30),
        bound: Duration::from_secs(5),
    };
    // (case, states, ephemeral, exit file, the code or the refusal)
    type Want = Result<i32, (i32, &'static str)>;
    type Row = (
        &'static str,
        Vec<&'static str>,
        bool,
        Option<&'static [u8]>,
        Want,
    );
    let rows: Vec<Row> = vec![
        (
            "ends",
            vec!["Running", "Running", "Stopped"],
            false,
            Some(b"3\n"),
            Ok(3),
        ),
        ("ended before", vec!["Stopped"], false, Some(b"0\n"), Ok(0)),
        (
            "a signal",
            vec!["Running", "Stopped"],
            false,
            Some(b"137\n"),
            Ok(137),
        ),
        (
            "created",
            vec!["Created"],
            false,
            None,
            Err((125, "it is Created")),
        ),
        (
            "no exit file",
            vec!["Stopped"],
            false,
            None,
            Err((125, "forced stop")),
        ),
        (
            "not a code",
            vec!["Stopped"],
            false,
            Some(b"x"),
            Err((125, "not an exit code")),
        ),
        (
            "rm: the code before it went",
            vec!["Running", "Gone"],
            true,
            Some(b"4\n"),
            Ok(4),
        ),
        (
            "rm: went with no code",
            vec!["Running", "Gone"],
            true,
            None,
            Err((125, "run --rm")),
        ),
        (
            "no such",
            vec!["Gone"],
            false,
            None,
            Err((1, "no such instance")),
        ),
        (
            "other",
            vec!["Stopped"],
            false,
            Some(b"0\n"),
            Err((1, "not an instance of podvm")),
        ),
    ];
    for (i, (case, states, eph, exit, want)) in rows.into_iter().enumerate() {
        let name = match case {
            "other" => format!("other{i}"),
            _ => format!("w{i}"),
        };
        {
            let mut g = w.lock().unwrap();
            g.states.insert(name.clone(), states);
            if eph {
                g.ephemeral.push(name.clone());
            }
        }
        if let Some(b) = exit {
            file(&w, &name, super::EXIT, "file", b);
        }
        match (rt.block_on(wait_one(&c, &name, pace)), want) {
            (Ok(got), Ok(want)) => assert_eq!(got, want, "{case}"),
            (Err((code, line)), Err((want, word))) => {
                assert_eq!(code, want, "{case}: {line}");
                assert!(line.contains(word), "{case}: {line}");
            }
            (got, want) => panic!("{case}: {got:?}, want {want:?}"),
        }
    }
}

/// `kill` runs podvm in the instance with the signal; an instance that does
/// not run, a command that ended and an older podvm in the image are
/// refusals with docker's codes.
#[test]
fn kill_signals_the_command_by_podvm_in_the_instance() {
    let (c, w, rt, _d) = world();
    // (instance, state, the end of guest-kill, the code or the refusal)
    type Want = Result<(), (i32, &'static str)>;
    let rows: Vec<(&str, &'static str, (i32, &'static str), Want)> = vec![
        ("k1", "Running", (0, ""), Ok(())),
        (
            "k2",
            "Stopped",
            (0, ""),
            Err((1, "is Stopped, not running")),
        ),
        (
            "k3",
            "Created",
            (0, ""),
            Err((1, "is Created, not running")),
        ),
        (
            "k4",
            "Running",
            (GUEST_ENDED, "the command (pid 9) has ended"),
            Err((1, "has ended")),
        ),
        (
            "k5",
            "Running",
            (2, "/.podvm/pid: No such file"),
            Err((125, "/.podvm/pid")),
        ),
        (
            "k6",
            "Running",
            (1, ""),
            Err((125, "guest-kill in the instance: ended with 1")),
        ),
        ("k7", "Gone", (0, ""), Err((1, "no such instance"))),
    ];
    for (name, state, guest, want) in rows {
        {
            let mut g = w.lock().unwrap();
            g.states.insert(name.to_string(), vec![state]);
            g.guest.insert(name.to_string(), guest);
        }
        match (rt.block_on(kill_one(&c, name, 15)), want) {
            (Ok(()), Ok(())) => {}
            (Err((code, line)), Err((want, word))) => {
                assert_eq!(code, want, "{name}: {line}");
                assert!(line.contains(word), "{name}: {line}");
            }
            (got, want) => panic!("{name}: {got:?}, want {want:?}"),
        }
    }
    let execs = &w.lock().unwrap().execs;
    assert_eq!(execs.len(), 4, "an exec only where the instance runs");
    assert_eq!(
        execs[0]["command"],
        json!(["/.podvm/podvm", "guest-kill", "15"])
    );
    // The signal of each form of the flag; SIGKILL where none is given.
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    for (args, sig, rest) in [
        (s(&["k"]), 9, s(&["k"])),
        (s(&["-s", "TERM", "k"]), 15, s(&["k"])),
        (s(&["--signal", "SIGHUP", "k"]), 1, s(&["k"])),
        (s(&["--signal=USR1", "k"]), 10, s(&["k"])),
        (s(&["-s2", "k"]), 2, s(&["k"])),
    ] {
        assert_eq!(signal_of(&args).unwrap(), (sig, rest), "{args:?}");
    }
    for bad in [
        s(&["-s"]),
        s(&["-s", "NOPE", "k"]),
        s(&["--signal=99", "k"]),
    ] {
        assert_eq!(signal_of(&bad).unwrap_err().0, 125, "{bad:?}");
    }
    // podvm in the instance refuses a bad signal and a missing pid file.
    assert_eq!(super::waitkill::guest_kill(&s(&["0"])), 2);
    assert_eq!(super::waitkill::guest_kill(&s(&[])), 2);
    if !std::path::Path::new(super::PID).exists() {
        assert_eq!(super::waitkill::guest_kill(&s(&["15"])), 2);
    }
}

/// incusd says `Stopped` for an instance that never ran; podvm says
/// `Created` only where `last_used_at` is the zero time of incusd.
#[test]
fn an_instance_that_never_started_is_created() {
    use super::ps::{never_started, status};
    for (st, used, want) in [
        ("Stopped", "1970-01-01T00:00:00Z", "Created"),
        ("Stopped", "0001-01-01T00:00:00Z", "Created"),
        ("Stopped", "2026-11-26T08:37:37.46Z", "Stopped"),
        // Not known: not called Created.
        ("Stopped", "", "Stopped"),
        ("Running", "1970-01-01T00:00:00Z", "Running"),
        ("Error", "1970-01-01T00:00:00Z", "Error"),
    ] {
        let i = pod_incus::Instance {
            status: st.into(),
            last_used_at: used.into(),
            ..Default::default()
        };
        assert_eq!(status(&i), want, "{st} {used}");
        assert_eq!(never_started(&i), want == "Created");
        let f = super::inspect::fields(&i, "-", &[], &Default::default());
        let get = |k: &str| f.iter().find(|(n, _)| *n == k).unwrap().1.clone();
        assert_eq!(get("Status"), want);
        assert_eq!(get("State.Status"), want.to_lowercase());
    }
}

/// The kind of a path after its links, and the lines that refuse a
/// working directory, an init and name a missing log, from one table.
#[test]
fn a_path_is_found_after_its_links_and_each_refusal_names_it() {
    for (link, target, want) in [
        ("/a/b", "c", "/a/c"),
        ("/a/b", "/x/y", "/x/y"),
        ("/a/b", "../../../z", "/z"),
        ("/b", "./c/", "/c"),
    ] {
        assert_eq!(join_link(link, target), want, "{link} -> {target}");
    }
    assert_eq!(normal("//w/./x/../y"), "/w/y");
    let (c, w, rt, _d) = world();
    file(&w, "p1", "/w", "directory", b"");
    file(&w, "p1", "/f", "file", b"x");
    file(&w, "p1", "/l1", "symlink", b"l2");
    file(&w, "p1", "/l2", "symlink", b"/w");
    file(&w, "p1", "/dangle", "symlink", b"nowhere");
    file(&w, "p1", "/loop", "symlink", b"loop");
    file(&w, "p1", "/sbin/init", "symlink", b"../lib/systemd/systemd");
    file(&w, "p1", "/lib/systemd/systemd", "file", b"\x7fELF");
    let dir = Found::Kind(FileKind::Directory);
    for (path, want) in [
        ("/", dir.clone()),
        ("/w", dir.clone()),
        ("/w/", dir.clone()),
        ("/f", Found::Kind(FileKind::File)),
        ("/l1", dir.clone()),
        ("/none", Found::Missing),
        ("/dangle", Found::Missing),
        ("/sbin/init", Found::Kind(FileKind::File)),
    ] {
        assert_eq!(rt.block_on(find(&c, "p1", path)), want, "{path}");
    }
    assert!(
        matches!(rt.block_on(find(&c, "p1", "/loop")), Found::Unknown(e) if e.contains("ELOOP"))
    );
    // exec -w: refused where the directory is not there or is a file.
    use super::exec::workdir_refusal;
    let missing = workdir_refusal("e1", "/nonexistent", Found::Missing).unwrap();
    assert!(
        missing.contains("\"/nonexistent\" is not there") && missing.contains("126"),
        "{missing}"
    );
    assert!(workdir_refusal("e1", "/f", Found::Kind(FileKind::File))
        .unwrap()
        .contains("is a file"));
    assert_eq!(workdir_refusal("e1", "/w", dir.clone()), None);
    assert_eq!(
        workdir_refusal("e1", "/w", Found::Unknown("x".into())),
        None
    );
    assert_eq!(workdir_refusal("e1", "", Found::Missing), None);
    // --podvm-init=image: refused where the image has no init.
    use super::verbs::init_refusal;
    assert!(init_refusal(Found::Missing)
        .unwrap()
        .contains("has no /sbin/init"));
    assert!(init_refusal(dir)
        .unwrap()
        .contains("a directory at /sbin/init"));
    assert_eq!(init_refusal(Found::Kind(FileKind::File)), None);
    assert_eq!(init_refusal(Found::Unknown("x".into())), None);
    // logs: the payload removed the directory of podvm, or only the log.
    w.lock()
        .unwrap()
        .states
        .insert("p1".into(), vec!["Stopped"]);
    let gone = rt.block_on(super::logs::missing_log(&c, "p1"));
    assert!(
        gone.contains("/.podvm is not there") && gone.contains("payload removed it"),
        "{gone}"
    );
    file(&w, "p1", super::DIR, "directory", b"");
    let only = rt.block_on(super::logs::missing_log(&c, "p1"));
    assert!(
        only.contains("/.podvm/log is not there") && only.contains("ExitCode"),
        "{only}"
    );
}
