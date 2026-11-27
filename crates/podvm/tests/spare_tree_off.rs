//! TODO/gate.md T-1859: a test leaves no `podvm-*` directory. A `create` or
//! a `run` of an image starts a detached spare-tree fill at podvm's exit
//! (TODO/image.md T-1781), which can write into the store after the test
//! removed it (gate 45960813246: `cp_stream.rs` left
//! `/tmp/podvm-t2108-exit-PID`). So each test file that gets an image into
//! its store and creates a container from it sets `PODVM_SPARE_TREE=0`.

use std::path::{Path, PathBuf};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Whether `text` holds one of `words` as a quoted argument.
fn quoted(text: &str, words: &[&str]) -> bool {
    words.iter().any(|w| text.contains(&format!("\"{w}\"")))
}

#[test]
fn each_test_that_creates_a_container_turns_the_spare_tree_off() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut all = Vec::new();
    files(&root, &mut all);
    assert!(all.len() > 10, "{}: too few test files", root.display());
    let me = Path::new(file!()).file_name().unwrap();
    // The names are split, so that this file does not match itself.
    let binary = concat!("CARGO_BIN_", "EXE_podvm");
    let off = concat!("PODVM_SPARE_", "TREE");
    let mut missing = Vec::new();
    for path in all {
        if path.file_name() == Some(me) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let image = quoted(&text, &["import", "pull", "load"]);
        let container = quoted(&text, &["create", "run"]);
        if text.contains(binary) && image && container && !text.contains(off) {
            missing.push(
                path.strip_prefix(&root)
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
            );
        }
    }
    assert!(
        missing.is_empty(),
        "these tests create a container and leave the spare-tree fill on; \
         give each podvm it starts PODVM_SPARE_TREE=0: {missing:?}"
    );
}
