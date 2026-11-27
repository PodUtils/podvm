//! TODO/cli.md T-2087 (OR-B40-15) and T-2108 (OR-B42-08): docker's
//! `--format` templates through each verb that takes one: `{{json .}}`,
//! `json`, `table`, `range`, `index`, `join` and the case functions, and a
//! refusal of one true line with exit 1 for each form that podvm does not
//! read.
//!
//! Hermetic: the image is a one-file rootfs that `podvm import` records,
//! and `create` leaves a record with no run.

use std::path::{Path, PathBuf};
use std::process::Command;

fn podvm(store: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_podvm"))
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

/// A ustar archive of one executable file, in pure std.
fn write_ustar(path: &Path, name: &str, data: &[u8]) {
    let mut hdr = [0u8; 512];
    hdr[..name.len()].copy_from_slice(name.as_bytes());
    hdr[100..108].copy_from_slice(b"0000755\0");
    hdr[108..116].copy_from_slice(b"0000000\0");
    hdr[116..124].copy_from_slice(b"0000000\0");
    hdr[124..136].copy_from_slice(format!("{:011o}\0", data.len()).as_bytes());
    hdr[136..148].copy_from_slice(b"00000000000\0");
    hdr[148..156].copy_from_slice(b"        ");
    hdr[156] = b'0';
    hdr[257..262].copy_from_slice(b"ustar");
    hdr[263..265].copy_from_slice(b"00");
    let sum: u32 = hdr.iter().map(|b| u32::from(*b)).sum();
    hdr[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
    let mut tar = hdr.to_vec();
    tar.extend_from_slice(data);
    tar.resize(tar.len() + (512 - data.len() % 512) % 512, 0);
    tar.resize(tar.len() + 1024, 0);
    std::fs::write(path, tar).unwrap();
}

/// A store with the image `lab/fmt:1` and the created container `fmt`.
fn store() -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("podvm-format-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let tar = dir.join("r.tar");
    let me = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    write_ustar(&tar, "hello", &me);
    let store = dir.join("store");
    let (rc, _, err) = podvm(&store, &["import", tar.to_str().unwrap(), "lab/fmt:1"]);
    assert_eq!(rc, 0, "import: {err}");
    let (rc, _, err) = podvm(
        &store,
        &[
            "create",
            "--name",
            "fmt",
            "--label",
            "com.x.k=v",
            "lab/fmt:1",
            "/hello",
            "--list",
        ],
    );
    assert_eq!(rc, 0, "create: {err}");
    (dir, store)
}

/// What a row expects: the exact stdout, a part of it, or exit 1 with a
/// part of the one line on stderr.
enum Want {
    Is(&'static str),
    Has(&'static str),
    Refused(&'static str),
}

#[test]
fn each_verb_reads_dockers_templates_or_refuses_with_one_line() {
    let (dir, store) = store();
    use Want::*;
    let rows: Vec<(Vec<&str>, Want)> = vec![
        (
            vec!["ps", "-a", "--format", "{{json .}}"],
            Has(r#""Names":"fmt""#),
        ),
        (
            vec!["ps", "-a", "--format", "json"],
            Has(r#""State":"created""#),
        ),
        (
            vec!["ps", "-a", "--format", "table {{.Names}}\\t{{.State}}"],
            Is("NAMES     STATE\nfmt       created\n"),
        ),
        (
            vec!["ps", "-a", "--format", "table"],
            Has("CONTAINER ID   IMAGE"),
        ),
        (
            vec!["ps", "-a", "--format", "{{.Names | upper}}"],
            Is("FMT\n"),
        ),
        (
            vec!["images", "--format", "table {{.Repository}}\\t{{.Tag}}"],
            Is("REPOSITORY   TAG\nlab/fmt      1\n"),
        ),
        (
            vec!["images", "--format", "{{json .}}"],
            Has(r#""Repository":"lab/fmt""#),
        ),
        (
            vec!["images", "--format", "{{lower .Repository}}:{{.Tag}}"],
            Is("lab/fmt:1\n"),
        ),
        (
            vec![
                "inspect",
                "--format",
                r#"{{index .Config.Labels "com.x.k"}}"#,
                "fmt",
            ],
            Is("v\n"),
        ),
        (
            vec!["inspect", "--format", "{{index .Config.Cmd 0}}", "fmt"],
            Is("/hello\n"),
        ),
        (
            vec![
                "inspect",
                "--format",
                "{{range .Config.Cmd}}[{{.}}]{{end}}",
                "fmt",
            ],
            Is("[/hello][--list]\n"),
        ),
        (
            vec!["inspect", "--format", r#"{{join .Config.Cmd ","}}"#, "fmt"],
            Is("/hello,--list\n"),
        ),
        (
            vec![
                "inspect",
                "--format",
                "{{.State.Running}} {{.Config.Image}} {{index .Config.Cmd 0}}",
                "fmt",
            ],
            Is("false lab/fmt:1 /hello\n"),
        ),
        (
            vec!["inspect", "--format", "{{json .State}}", "fmt"],
            Has(r#""Status":"created""#),
        ),
        (
            vec!["inspect", "--format", "{{json .}}", "fmt"],
            Has(r#""Name":"fmt""#),
        ),
        (
            vec!["inspect", "--format", "{{index .RepoTags 0}}", "lab/fmt:1"],
            Is("lab/fmt:1\n"),
        ),
        (
            vec!["system", "info", "--format", "{{len .Parity}}"],
            Has(""),
        ),
        // The refusals: exit 1 and the part that fails.
        (
            vec!["ps", "--format", "{{if .Names}}x{{end}}"],
            Refused("`if` is not a part"),
        ),
        (
            vec!["images", "--format", "table {{.Nope}}"],
            Refused("no field \".Nope\""),
        ),
        (
            vec!["images", "--format", "{{printf \"%s\" .Tag}}"],
            Refused("`printf (a function)`"),
        ),
        (
            vec!["inspect", "--format", "{{index .Config.Cmd 9}}", "fmt"],
            Refused("the index 9 is out of range: the list has 2"),
        ),
        (
            vec!["inspect", "--format", "table {{.Name}}", "fmt"],
            Refused("the `table` prefix"),
        ),
        (
            vec!["inspect", "--format", "{{range .Config.Cmd}}", "fmt"],
            Refused("has no `{{end}}`"),
        ),
        (
            vec!["inspect", "--format", "{{.Foo}}", "fmt"],
            Refused("no image and no container has the field \".Foo\""),
        ),
        (
            vec!["system", "info", "--format", "{{.Parity"],
            Refused("never closed"),
        ),
    ];
    let mut bad = Vec::new();
    for (args, want) in &rows {
        let (rc, out, err) = podvm(&store, args);
        let ok = match want {
            Is(s) => rc == 0 && out == *s,
            Has(s) => rc == 0 && out.contains(s) && !out.is_empty(),
            Refused(s) => rc == 1 && out.is_empty() && err.contains(s) && err.lines().count() == 1,
        };
        if !ok {
            bad.push(format!(
                "{args:?}: exit {rc}, stdout {out:?}, stderr {err:?}"
            ));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(bad.is_empty(), "{bad:#?}");
}
