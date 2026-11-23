//! An OCI layout of one image made of the host's programs: the loader and
//! libc of glibc on x86_64, `dash` as `/bin/sh`, and the named programs of
//! the host's `/usr/bin` (TODO/cli.md T-2071, T-2072). A host that lacks a
//! file gives `None`, and the test skips with a line on stderr.

#![allow(dead_code)]

use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::{Command, Output};

/// The loader, libc and the shell, inside and the host's candidates.
const BASE: &[(&str, &[&str])] = &[
    (
        "lib64/ld-linux-x86-64.so.2",
        &["/lib64/ld-linux-x86-64.so.2"],
    ),
    (
        "lib/x86_64-linux-gnu/libc.so.6",
        &[
            "/lib/x86_64-linux-gnu/libc.so.6",
            "/usr/lib/x86_64-linux-gnu/libc.so.6",
        ],
    ),
    ("bin/sh", &["/usr/bin/dash", "/bin/dash"]),
];

fn add(b: &mut tar::Builder<Vec<u8>>, name: &str, mode: u32, bytes: &[u8]) -> Option<()> {
    let mut h = tar::Header::new_gnu();
    h.set_size(bytes.len() as u64);
    h.set_mode(mode);
    h.set_cksum();
    b.append_data(&mut h, name, bytes).ok()
}

fn sha(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

fn host_file(candidates: &[&str]) -> Option<Vec<u8>> {
    let host = candidates.iter().map(Path::new).find(|p| p.exists())?;
    std::fs::read(std::fs::canonicalize(host).ok()?).ok()
}

/// The layout of the image `name` (`localhost/NAME:TAG`) with the programs
/// `bins` of the host as `/bin/PROGRAM`, as a tar; `None` where the host
/// lacks a file or the machine is not x86_64.
pub fn layout(name: &str, bins: &[&str]) -> Option<Vec<u8>> {
    if !cfg!(target_arch = "x86_64") {
        return None;
    }
    let mut b = tar::Builder::new(Vec::new());
    for dir in ["tmp", "etc", "proc", "dev", "sys", "root"] {
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Directory);
        h.set_mode(if dir == "tmp" { 0o1777 } else { 0o755 });
        h.set_size(0);
        h.set_cksum();
        b.append_data(&mut h, format!("{dir}/"), &[][..]).ok()?;
    }
    add(
        &mut b,
        "etc/passwd",
        0o644,
        b"root:x:0:0:root:/root:/bin/sh\n",
    )?;
    for (inside, candidates) in BASE {
        add(&mut b, inside, 0o755, &host_file(candidates)?)?;
    }
    for bin in bins {
        let bytes = host_file(&[&format!("/usr/bin/{bin}"), &format!("/bin/{bin}")])?;
        add(&mut b, &format!("bin/{bin}"), 0o755, &bytes)?;
    }
    let layer = b.into_inner().ok()?;
    let lh = sha(&layer);
    let config = serde_json::json!({
        "architecture": "amd64", "os": "linux",
        "config": {"Env": ["PATH=/bin"]},
        "rootfs": {"type": "layers", "diff_ids": [format!("sha256:{lh}")]},
    })
    .to_string();
    let ch = sha(config.as_bytes());
    let manifest = serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "config": {"mediaType": "application/vnd.oci.image.config.v1+json",
                   "digest": format!("sha256:{ch}"), "size": config.len()},
        "layers": [{"mediaType": "application/vnd.oci.image.layer.v1.tar",
                    "digest": format!("sha256:{lh}"), "size": layer.len()}],
    })
    .to_string();
    let mh = sha(manifest.as_bytes());
    let index = serde_json::json!({
        "schemaVersion": 2,
        "manifests": [{"mediaType": "application/vnd.oci.image.manifest.v1+json",
                       "digest": format!("sha256:{mh}"), "size": manifest.len(),
                       "annotations": {"org.opencontainers.image.ref.name": name}}],
    })
    .to_string();
    let mut b = tar::Builder::new(Vec::new());
    add(
        &mut b,
        "oci-layout",
        0o644,
        br#"{"imageLayoutVersion":"1.0.0"}"#,
    )?;
    add(&mut b, "index.json", 0o644, index.as_bytes())?;
    add(
        &mut b,
        &format!("blobs/sha256/{ch}"),
        0o644,
        config.as_bytes(),
    )?;
    add(&mut b, &format!("blobs/sha256/{lh}"), 0o644, &layer)?;
    add(
        &mut b,
        &format!("blobs/sha256/{mh}"),
        0o644,
        manifest.as_bytes(),
    )?;
    b.into_inner().ok()
}

/// podvm with `args` on the store `store`, with no detached fill of a
/// spare tree (T-1859).
pub fn podvm(store: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_podvm"))
        .env("PODVM_SPARE_TREE", "0")
        .env("PODVM_STORE", store)
        .args(args)
        .output()
        .unwrap()
}

pub fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// A new directory for one test, and the store in it with the image of
/// `tar` loaded; a failed load fails the test.
pub fn store_with(tag: &str, tar: Vec<u8>) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("podvm-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let tar_path = dir.join("img.tar");
    std::fs::write(&tar_path, tar).unwrap();
    let store = dir.join("store");
    let l = podvm(&store, &["load", "-i", tar_path.to_str().unwrap()]);
    assert!(l.status.success(), "load: {}", text(&l.stderr));
    (dir, store)
}
