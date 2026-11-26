//! TODO/cli.md T-1829: refusals lead with the cause and name the spelling
//! that the caller wrote. The text of the entrypoint on the userland rung
//! is a unit test (`lifecycle::script`), as that path needs chroot denied.

use std::process::{Command, Stdio};

fn podvm(args: &[&str], store: &std::path::Path) -> (Option<i32>, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_podvm"))
        .args(args)
        .env("PODVM_STORE", store)
        .env("HOME", store)
        .env("PODVM_SPARE_TREE", "0")
        .current_dir(store)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (
        o.status.code(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// A ustar archive of one executable file, in pure std.
fn write_ustar(path: &std::path::Path, name: &str, data: &[u8]) {
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

#[test]
fn refusal_text_names_the_cause() {
    let d = std::env::temp_dir().join(format!("podvm-t1829-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();

    // 1. no image: the usage line and the cause, exit 1, not 11 kB.
    let (rc, err) = podvm(&["run"], &d);
    assert_eq!(rc, Some(1), "{err}");
    assert!(
        err.contains("an image is needed") && err.len() < 400,
        "{err}"
    );

    // 2. the spelling of the caller.
    let (_, err) = podvm(&["run", "--env"], &d);
    assert!(err.contains("--env needs a value"), "{err}");

    // 4. a known option with a bad value.
    let (rc, err) = podvm(
        &["run", "--security-opt", "no-new-privileges=bogus", "x"],
        &d,
    );
    assert_eq!(rc, Some(125), "{err}");
    assert!(
        err.contains("takes true or false") && err.contains("bogus"),
        "{err}"
    );

    // 6. a negative count of --tail.
    let (rc, err) = podvm(&["logs", "--tail", "-1", "x"], &d);
    assert_eq!(rc, Some(125), "{err}");
    assert!(err.contains("non-negative line count"), "{err}");

    // 7. `,` where the rules part with `;`.
    let (rc, err) = podvm(&["run", "--podvm-confine", "read=/a,exec=/b", "x"], &d);
    assert_eq!(rc, Some(125), "{err}");
    assert!(err.contains("the rules part with `;`"), "{err}");

    // 8. TODO/cli.md T-2100: `load` of an empty stdin names stdin, not the
    // spool of the store that took it.
    let (rc, err) = podvm(&["load"], &d);
    assert_eq!(rc, Some(1), "{err}");
    assert!(
        err.contains("the archive on stdin has no index.json") && !err.contains("staging"),
        "{err}"
    );

    // 5. a confine path that the image lacks: the guest path, no host path.
    let tar = d.join("r.tar");
    write_ustar(
        &tar,
        "hello",
        &std::fs::read(std::env::current_exe().unwrap()).unwrap(),
    );
    let (rc, err) = podvm(&["import", tar.to_str().unwrap(), "t1829:1"], &d);
    assert_eq!(rc, Some(0), "{err}");
    let (rc, err) = podvm(
        &[
            "run",
            "--rm",
            "--podvm-confine",
            "read=/definitely-not-in-image",
            "t1829:1",
            "/hello",
        ],
        &d,
    );
    assert_ne!(rc, Some(0), "{err}");
    let line = err
        .lines()
        .find(|l| l.contains("definitely-not-in-image"))
        .unwrap_or_else(|| panic!("no refusal of the path: {err}"));
    assert!(line.contains("\"/definitely-not-in-image\""), "{line}");
    assert!(!line.contains(&*d.to_string_lossy()), "a host path: {line}");
    let _ = std::fs::remove_dir_all(&d);
}
