//! TODO/cli.md T-2072 (OR-B38-06 of the seventh outside pass): podvm writes
//! no file of its own through a volume into the caller's directory.
//!
//! Before, a foreground run in podvm's own user namespace bound its volumes
//! before the completion, so `-v DIR:/etc` got `passwd`, `group`, `hosts`,
//! `mtab`, `resolv.conf`, `hostname` and the CA bundle in DIR, and
//! `-v DIR:/dev` got `null`, `zero`, `random` and `urandom` as regular files
//! of 1 MiB, links and `pts/` and `shm/`. The test seeds each directory with
//! one file and checks that it holds that file alone after the run, for
//! each guest path whose tree podvm completes. The fault showed as a user
//! with no root; as root the test checks the same rule on the rung of root.

mod host_image;
use host_image::{podvm, text};

const IMAGE: &str = "localhost/t2072:1";

#[test]
fn a_volume_over_a_fixup_target_gets_no_file_of_podvm() {
    let Some(tar) = host_image::layout(IMAGE, &["cat"]) else {
        eprintln!("T-2072: this host has no glibc or dash at the x86_64 paths; skipped");
        return;
    };
    let (dir, store) = host_image::store_with("t2072", tar);
    // Each guest path, with the flags that make podvm write more files of
    // its own there (`--hostname` writes /etc/hostname and a line of
    // /etc/hosts, `--add-host` a line of /etc/hosts).
    let cases: &[(&str, &[&str])] = &[
        ("/etc", &[]),
        (
            "/etc",
            &["--hostname", "hx", "--add-host", "a.example:192.0.2.1"],
        ),
        ("/etc/ssl", &[]),
        ("/etc/ssl/certs", &[]),
        ("/dev", &[]),
        ("/dev/pts", &[]),
    ];
    let mut failures = Vec::new();
    for (i, (guest, flags)) in cases.iter().enumerate() {
        let host = dir.join(format!("vol{i}"));
        std::fs::create_dir_all(&host).unwrap();
        std::fs::write(host.join("mine.txt"), "mine\n").unwrap();
        let bind = format!("{}:{guest}", host.display());
        let mut args = vec!["run", "--rm", "-v", &bind];
        args.extend_from_slice(flags);
        args.extend_from_slice(&[IMAGE, "sh", "-c", "exit 0"]);
        let r = podvm(&store, &args);
        let mut left: Vec<String> = std::fs::read_dir(&host)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        if !r.status.success() {
            failures.push(format!("{args:?}: {:?} {}", r.status, text(&r.stderr)));
        } else if left != ["mine.txt"] {
            failures.push(format!("-v DIR:{guest} {flags:?}: DIR holds {left:?}"));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
