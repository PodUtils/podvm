//! TODO/cli.md T-2100 (OR-B41-17): a store root that a file blocks names
//! the store and the file, at each depth of the path. Before, the error
//! was a bare `PATH: File exists (os error 17)`.

use super::*;

#[test]
fn a_file_on_the_path_of_the_store_root_is_named() {
    let d = std::env::temp_dir().join(format!(
        "podvm-store-file-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |t| t.as_nanos())
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let file = d.join("afile");
    std::fs::write(&file, b"x").unwrap();
    // (the root, from the file)
    for root in [
        file.clone(),
        file.join("sub"),
        file.join("sub").join("deep"),
    ] {
        let e = Store::open(&root).expect_err("a file blocks the root");
        let text = e.to_string();
        assert!(
            text.contains(&format!("{} cannot be the store root", root.display()))
                && text.contains(&format!("{} is a file", file.display())),
            "{text}"
        );
        assert_eq!(e.exit_code(), crate::error::EXIT_RUNTIME_ERROR, "{text}");
    }
    let _ = std::fs::remove_dir_all(&d);
}
