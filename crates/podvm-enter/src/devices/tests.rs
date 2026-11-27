use super::*;

use std::os::unix::fs::symlink;

fn tree(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let d = std::env::temp_dir().join(format!("podvm-dev-{tag}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(d.join("dev")).unwrap();
    d
}

/// T-2107: one rule for each kind that the image can hold at `/dev/NAME`,
/// as `pod_enter::choose` decides it: a path that is no regular file, no
/// character device and not missing stays as it is, with one line that
/// names the path and the cause, and nothing is mounted on it. (The binds
/// of the other kinds need a user namespace: the test `overlay_devices`.)
#[test]
fn a_path_of_the_image_that_is_not_a_file_stays_with_one_line() {
    let r = tree("kinds");
    std::fs::create_dir(r.join("dev/null")).unwrap();
    symlink("/dev/zero", r.join("dev/zero")).unwrap();
    let lines = nodes(&r, &["null", "zero"]);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[0].contains("/dev/null") && lines[0].contains("a directory"),
        "{lines:?}"
    );
    assert!(
        lines[1].contains("/dev/zero") && lines[1].contains("symbolic link"),
        "{lines:?}"
    );
    assert!(std::fs::symlink_metadata(r.join("dev/null"))
        .unwrap()
        .is_dir());
    assert!(std::fs::symlink_metadata(r.join("dev/zero"))
        .unwrap()
        .file_type()
        .is_symlink());
    let _ = std::fs::remove_dir_all(&r);
}

/// T-2107: a tree whose `/dev` is missing or a link takes no bind: the
/// bind would follow the link out of the tree.
#[test]
fn a_tree_without_a_dev_directory_takes_no_bind() {
    let r = tree("nodev");
    std::fs::remove_dir(r.join("dev")).unwrap();
    let lines = bind(&r, &["null"]);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("no /dev directory"), "{lines:?}");
    let out = tree("out");
    symlink(out.join("dev"), r.join("dev")).unwrap();
    let lines = bind(&r, &["null"]);
    assert!(
        lines.iter().any(|l| l.contains("no /dev directory")),
        "{lines:?}"
    );
    assert!(
        !out.join("dev/null").exists(),
        "a file was made through the link"
    );
    let _ = std::fs::remove_dir_all(&r);
    let _ = std::fs::remove_dir_all(&out);
}
