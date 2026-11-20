//! The root of a test of the native-static rung, shared by the files of
//! `tests/`: `etc`, `bin` with the probe, `tmp`, `proc` and `dev`.

use std::path::PathBuf;
use std::process::Command;

pub struct Root(pub PathBuf);

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn root(name: &str) -> Option<Root> {
    if let Err(e) = podvm_dispatch::available() {
        eprintln!("SKIP: {e}");
        return None;
    }
    let d = std::env::temp_dir().join(format!("podvm-t1652-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    for sub in ["etc", "bin", "tmp", "proc", "dev"] {
        std::fs::create_dir_all(d.join(sub)).unwrap();
    }
    std::fs::write(d.join("etc/podvm-marker"), "inroot\n").unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_podvm-dispatch-probe"),
        d.join("bin/probe"),
    )
    .unwrap();
    std::os::unix::fs::symlink("/etc/podvm-marker", d.join("tmp/abs")).unwrap();
    std::fs::write(d.join("bin/script"), "#!/bin/probe cat\n").unwrap();
    std::os::unix::fs::PermissionsExt::set_mode(
        &mut std::fs::metadata(d.join("bin/script"))
            .unwrap()
            .permissions(),
        0o755,
    );
    Command::new("chmod")
        .arg("755")
        .arg(d.join("bin/script"))
        .status()
        .unwrap();
    Some(Root(d))
}
