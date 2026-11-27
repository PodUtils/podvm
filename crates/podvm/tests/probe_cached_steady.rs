//! TODO/enter.md T-1953 (PodUtils/podvm#558): `probe --cached` is served
//! from the second call on, for one host and one user.
//!
//! A caller with no capability runs `probe` in a user namespace and a
//! mount namespace that podvm makes at its start. The key of the cache
//! read that namespace, which is fresh at each call, so no call matched the
//! one before. The test runs where the caller is that user (CI) and where it
//! is not (root has no such namespace): the rule is the same, a key that
//! holds for the next call.

use std::path::PathBuf;
use std::process::{Command, Output};

fn podvm() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_podvm"))
}

fn cached(store: &std::path::Path) -> Output {
    Command::new(podvm())
        .args(["probe", "--cached"])
        .env("PODVM_STORE", store)
        .env("HOME", store)
        .output()
        .expect("podvm starts")
}

#[test]
fn the_second_and_third_call_are_served_from_the_cache() {
    let store = std::env::temp_dir().join(format!(
        "podvm-cached-steady-{}-{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos())
    ));
    std::fs::create_dir_all(&store).unwrap();
    let says: Vec<String> = (0..3)
        .map(|_| String::from_utf8_lossy(&cached(&store).stderr).into_owned())
        .collect();
    let _ = std::fs::remove_dir_all(&store);
    assert!(says[0].contains("measured now"), "{}", says[0]);
    for (n, said) in says.iter().enumerate().skip(1) {
        assert!(
            said.contains("served from") && !said.contains("measured now"),
            "call {} of `probe --cached`: {said}",
            n + 1
        );
    }
}
