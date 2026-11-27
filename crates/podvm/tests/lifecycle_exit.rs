//! TODO/supervise.md T-1805: a name that resolves to nothing exits 1 on
//! each lifecycle verb, with one line on stderr, as docker 29.9.2 does
//! (measured 2026-11-14) and as podvm's own table of exit codes says.
//! TODO/cli.md T-2079 (OR-B39-19): `rm -f` names the name and exits 0, as
//! the CLI of docker does for a container that is not there.

use std::process::{Command, Stdio};

fn podvm(args: &[&str], store: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_podvm"))
        .args(args)
        .env("PODVM_STORE", store)
        .stdin(Stdio::null())
        .output()
        .expect("podvm runs")
}

struct Gone(std::path::PathBuf);

impl Drop for Gone {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn lifecycle_exit_of_a_missing_name_is_dockers_code_with_one_line() {
    let base = std::env::temp_dir().join(format!("podvm-lcexit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    // TODO/gate.md T-1859: the directory goes at the drop, also after a
    // panic, and each output path is inside it.
    let _gone = Gone(base.clone());
    let store = base.join("store");
    let (y, tar) = (base.join("y"), base.join("out.tar"));
    let (y, tar) = (y.to_str().unwrap(), tar.to_str().unwrap());
    for (args, code) in [
        (&["start", "nosuch"][..], 1),
        (&["stop", "nosuch"], 1),
        (&["restart", "nosuch"], 1),
        (&["rm", "nosuch"], 1),
        (&["rm", "-f", "nosuch"], 0),
        (&["logs", "nosuch"], 1),
        (&["wait", "nosuch"], 1),
        (&["kill", "nosuch"], 1),
        (&["cp", "nosuch:/x", y], 1),
        // PodUtils/podvm#356: the two verbs that take an image and never
        // pull, as docker 29.9.2 answers `save` (measured 2026-11-14).
        (&["exec", "nosuch", "/bin/true"], 1),
        (&["save", "-o", tar, "nosuch"], 1),
        (&["save", "nosuch"], 1),
    ] {
        let o = podvm(args, &store);
        let err = String::from_utf8_lossy(&o.stderr);
        assert_eq!(o.status.code(), Some(code), "{args:?}: {err}");
        assert_eq!(err.lines().count(), 1, "{args:?}: {err}");
        assert!(err.contains("nosuch"), "{args:?}: {err}");
    }
    let _ = std::fs::remove_dir_all(&store);
}
