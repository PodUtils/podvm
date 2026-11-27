//! TODO/cli.md T-2108 (OR-B42-08): docker's `.Mounts`, `.HostConfig.Binds`
//! and `.HostConfig.Tmpfs` of `inspect` of a container, through the real
//! verbs: `create`, `run -d` and a foreground `run` with each kind of mount
//! that podvm takes (`-v` with each mode, `--tmpfs` with and without a
//! limit), then `inspect --format`.
//!
//! Hermetic: the image is the static test program and a `/tmp`, which
//! `podvm import` records. A row whose verb this host refuses by the name
//! of its rung (a tmpfs with a limit and an overlay need the namespace
//! rung) is skipped with a line on stderr; each other failure fails.

use std::path::Path;
use std::process::Command;

use serde_json::{json, Value};

fn podvm(store: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_podvm"))
        // TODO/gate.md T-1859: no detached spare-tree fill after the test.
        .env("PODVM_SPARE_TREE", "0")
        .env_remove("RUST_BACKTRACE")
        .args(args)
        .env("PODVM_STORE", store)
        .output()
        .expect("podvm runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The tar of the image: this static program as `/hello`, and `/tmp`.
fn image_tar() -> Vec<u8> {
    let mut b = tar::Builder::new(Vec::new());
    let mut h = tar::Header::new_gnu();
    h.set_entry_type(tar::EntryType::Directory);
    h.set_mode(0o1777);
    h.set_size(0);
    h.set_cksum();
    b.append_data(&mut h, "tmp/", &[][..]).unwrap();
    let me = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    let mut h = tar::Header::new_gnu();
    h.set_size(me.len() as u64);
    h.set_mode(0o755);
    h.set_cksum();
    b.append_data(&mut h, "hello", &me[..]).unwrap();
    b.into_inner().unwrap()
}

/// What a template gives: a JSON value, or the exact text.
enum Want {
    Json(Value),
    Text(String),
}

/// docker's object of one bind in `.Mounts`.
fn bind(source: &str, destination: &str, mode: &str, rw: bool) -> Value {
    json!({
        "Type": "bind",
        "Source": source,
        "Destination": destination,
        "Mode": mode,
        "RW": rw,
        "Propagation": "rprivate",
    })
}

#[test]
fn inspect_gives_dockers_mounts_for_each_kind_of_mount() {
    let dir = std::env::temp_dir().join(format!("podvm-mounts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let vol = dir.join("vol");
    std::fs::create_dir_all(&vol).unwrap();
    std::fs::write(dir.join("img.tar"), image_tar()).unwrap();
    let store = dir.join("store");
    let (rc, _, err) = podvm(
        &store,
        &[
            "import",
            dir.join("img.tar").to_str().unwrap(),
            "lab/mounts:1",
        ],
    );
    assert_eq!(rc, 0, "import: {err}");
    let h = std::fs::canonicalize(&vol).unwrap();
    let h = h.to_str().unwrap();
    let v = |spec: &str| format!("{h}:{spec}");
    let mounts = "{{json .Mounts}}";
    let range = "{{range .Mounts}}{{.Type}} {{.Source}}:{{.Destination}} {{.Mode}} {{.RW}};{{end}}";
    let binds = "{{json .HostConfig.Binds}}";
    let tmpfs = "{{json .HostConfig.Tmpfs}}";
    let text = |s: String| Want::Text(s);
    use Want::Json;
    // (name, the verb and its flags before the image, the templates)
    type Row<'a> = (&'a str, Vec<String>, Vec<(&'a str, Want)>);
    let rows: Vec<Row> = vec![
        (
            "none",
            vec!["create".into()],
            vec![
                (mounts, Json(json!([]))),
                (range, text(String::new())),
                (binds, Json(Value::Null)),
                (tmpfs, Json(Value::Null)),
                (
                    "{{.HostConfig.Binds}} {{.HostConfig.Tmpfs}}",
                    text("[] map[]".into()),
                ),
            ],
        ),
        (
            "rw",
            vec!["create".into(), "-v".into(), v("/data")],
            vec![
                (mounts, Json(json!([bind(h, "/data", "", true)]))),
                (range, text(format!("bind {h}:/data  true;"))),
                (binds, Json(json!([v("/data")]))),
                (tmpfs, Json(Value::Null)),
            ],
        ),
        (
            "ro",
            vec![
                "create".into(),
                "-v".into(),
                v("/ro:ro,z"),
                "-v".into(),
                v("/rw:rw"),
            ],
            vec![
                (
                    mounts,
                    Json(json!([
                        bind(h, "/ro", "ro,z", false),
                        bind(h, "/rw", "rw", true)
                    ])),
                ),
                (binds, Json(json!([v("/ro:ro,z"), v("/rw:rw")]))),
            ],
        ),
        (
            "overlay",
            vec!["create".into(), "-v".into(), v("/lay:overlay")],
            vec![(mounts, Json(json!([bind(h, "/lay", "overlay", true)])))],
        ),
        (
            "tmpfs",
            vec!["create".into(), "--tmpfs".into(), "/w:size=1m".into()],
            vec![
                (mounts, Json(json!([]))),
                (binds, Json(Value::Null)),
                (tmpfs, Json(json!({"/w": "size=1m"}))),
                ("{{.HostConfig.Tmpfs}}", text("map[/w:size=1m]".into())),
            ],
        ),
        (
            "detached",
            vec!["run".into(), "-d".into(), "-v".into(), v("/data:ro")],
            vec![
                (mounts, Json(json!([bind(h, "/data", "ro", false)]))),
                (range, text(format!("bind {h}:/data ro false;"))),
            ],
        ),
        (
            "foreground",
            vec![
                "run".into(),
                "-v".into(),
                v("/data"),
                "--tmpfs".into(),
                "/x".into(),
            ],
            vec![
                (mounts, Json(json!([bind(h, "/data", "", true)]))),
                (tmpfs, Json(json!({"/x": ""}))),
            ],
        ),
    ];
    let mut bad = Vec::new();
    let mut made = 0;
    for (name, verb, checks) in &rows {
        let mut args: Vec<&str> = verb.iter().map(String::as_str).collect();
        args.extend(["--name", name, "lab/mounts:1", "/hello", "--list"]);
        let (rc, _, err) = podvm(&store, &args);
        if rc == 125 && err.contains("rung") {
            eprintln!("T-2108: {name}: this host refuses it by its rung; skipped: {err}");
            continue;
        }
        if rc != 0 {
            bad.push(format!("{name}: {args:?}: exit {rc}, stderr {err:?}"));
            continue;
        }
        made += 1;
        for (template, want) in checks {
            let (rc, out, err) = podvm(&store, &["inspect", "--format", template, name]);
            let got = out.strip_suffix('\n').unwrap_or(&out);
            let ok = rc == 0
                && match want {
                    Want::Json(j) => serde_json::from_str::<Value>(got).ok().as_ref() == Some(j),
                    Want::Text(t) => got == t,
                };
            if !ok {
                bad.push(format!(
                    "{name}: {template}: exit {rc}, stdout {out:?}, stderr {err:?}"
                ));
            }
        }
        // The document of plain `inspect` holds the same values.
        let (_, out, _) = podvm(&store, &["inspect", name]);
        let doc: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
        if !doc[0]["Mounts"].is_array() || !doc[0]["HostConfig"].is_object() {
            bad.push(format!(
                "{name}: the document has no Mounts or HostConfig: {out}"
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(bad.is_empty(), "{bad:#?}");
    // The rows of `-v` with no overlay need no namespace rung.
    assert!(made >= 3, "only {made} rows made a container");
}
