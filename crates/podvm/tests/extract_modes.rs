//! TODO/extract.md T-1813: a layer row of mode 4755, 2755 or 6755 ends
//! with that mode in the tree, as root and as uid 1000. Before podcrates
//! `v0.44.0` (podcrates@T-581) the extraction dropped `S_ISUID` and
//! `S_ISGID` of a file whose mode already had owner write.
//!
//! TODO/image.md T-2065: a layer of GNU tar `--format=posix --acls` (each
//! ACL in its text form in a PAX record `SCHILY.acl.*`, one entry on each
//! line) extracts with the mode of each entry, as root and as uid 1000. Each
//! ACL is on disk in its binary form, or the sidecar names it as not set.
//! Before podcrates `v0.82.0` (podcrates@T-1086) the newline byte in the
//! record refused the layer whole (PodUtils/podrunner#339).

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const ROWS: [(&str, u32); 3] = [("bin/u", 0o4755), ("bin/g", 0o2755), ("bin/ug", 0o6755)];

fn layer(dir: &Path) -> PathBuf {
    let mut b = tar::Builder::new(Vec::new());
    for (name, mode) in ROWS {
        let mut h = tar::Header::new_gnu();
        h.set_size(3);
        h.set_mode(mode);
        h.set_cksum();
        b.append_data(&mut h, name, &b"abc"[..]).unwrap();
    }
    write_layer(dir, b.into_inner().unwrap())
}

fn write_layer(dir: &Path, bytes: Vec<u8>) -> PathBuf {
    let tar = dir.join("a.tar");
    std::fs::write(&tar, bytes).unwrap();
    std::fs::set_permissions(&tar, std::fs::Permissions::from_mode(0o644)).unwrap();
    tar
}

/// One row of the ACL layer: the path, a directory or not, the mode, the
/// PAX key, the text of GNU tar `--acls --numeric-owner`, the attribute and
/// the value (hex) that Linux gave for the same ACL (the test of the
/// module `drive::acls` of `pod-extract`, podcrates@T-1086).
type AclRow = (
    &'static str,
    bool,
    u32,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
);

const ACL_ROWS: [AclRow; 2] = [
    (
        "d",
        true,
        0o775,
        "SCHILY.acl.default",
        "user::rwx\ngroup::r-x\ngroup:1000:rwx\nmask::rwx\nother::r-x\n",
        "system.posix_acl_default",
        "0200000001000700ffffffff04000500ffffffff08000700e803000010000700ffffffff20000500ffffffff",
    ),
    (
        "d/f",
        false,
        0o664,
        "SCHILY.acl.access",
        "user::rw-\nuser:1000:rw-\nuser:12345:r--\ngroup::r--\ngroup:0:r-x\ngroup:4:--x\nmask::rw-\nother::r--",
        "system.posix_acl_access",
        "0200000001000600ffffffff02000600e8030000020004003930000004000400ffffffff\
         0800050000000000080001000400000010000600ffffffff20000400ffffffff",
    ),
];

fn acl_layer(dir: &Path) -> PathBuf {
    let mut b = tar::Builder::new(Vec::new());
    for (path, is_dir, mode, key, text, _, _) in ACL_ROWS {
        b.append_pax_extensions([
            ("mtime", &b"1795232499.649341102"[..]),
            (key, text.as_bytes()),
        ])
        .unwrap();
        let mut h = tar::Header::new_ustar();
        h.set_mode(mode);
        if is_dir {
            h.set_entry_type(tar::EntryType::Directory);
            h.set_size(0);
            b.append_data(&mut h, path, std::io::empty()).unwrap();
        } else {
            h.set_entry_type(tar::EntryType::Regular);
            h.set_size(4);
            b.append_data(&mut h, path, &b"data"[..]).unwrap();
        }
    }
    write_layer(dir, b.into_inner().unwrap())
}

/// Import and extract as the caller of `prefix` (empty: this process),
/// and return the rootfs that `extract` prints.
fn extract(prefix: &[&str], podvm: &Path, dir: &Path, tar: &Path, tag: &str) -> PathBuf {
    let run = |args: &[&str]| {
        let mut argv: Vec<&str> = prefix.to_vec();
        argv.push(podvm.to_str().unwrap());
        argv.extend_from_slice(args);
        let out = Command::new(argv[0])
            .args(&argv[1..])
            .env("HOME", dir)
            .env("PODVM_STORE", dir.join("store"))
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(out.status.success(), "{args:?}: {err}");
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    run(&["import", tar.to_str().unwrap(), tag]);
    let out = run(&["extract", tag]);
    let line = out
        .lines()
        .find(|l| l.ends_with("/rootfs"))
        .unwrap_or_else(|| panic!("no rootfs line in {out}"));
    PathBuf::from(line)
}

fn assert_modes(rootfs: &Path, who: &str) {
    for (name, mode) in ROWS {
        let m = std::fs::metadata(rootfs.join(name))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777;
        assert_eq!(m, mode, "{who}: {name} is {m:o}, the layer says {mode:o}");
    }
}

unsafe extern "C" {
    fn lgetxattr(
        path: *const std::ffi::c_char,
        name: *const std::ffi::c_char,
        value: *mut u8,
        size: usize,
    ) -> isize;
}

/// The value of the attribute `name` of `path`, with no follow of a link.
fn attribute(path: &Path, name: &str) -> Option<Vec<u8>> {
    use std::os::unix::ffi::OsStrExt;
    let p = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    let n = std::ffi::CString::new(name).ok()?;
    let mut v = vec![0u8; 4096];
    // SAFETY: the buffer holds `v.len()` bytes; both strings are NUL-ended.
    let got = unsafe { lgetxattr(p.as_ptr(), n.as_ptr(), v.as_mut_ptr(), v.len()) };
    let len = usize::try_from(got).ok()?;
    v.truncate(len);
    Some(v)
}

fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

/// Each entry has its mode, and each ACL is on disk or named in the sidecar
/// (`<rootfs>/../.meta.jsonl`) as not set.
fn assert_acls(rootfs: &Path, who: &str) {
    let sidecar = std::fs::read_to_string(rootfs.join("../.meta.jsonl")).unwrap_or_default();
    for (path, _, mode, _, _, name, value) in ACL_ROWS {
        let p = rootfs.join(path);
        let m = std::fs::metadata(&p).unwrap().permissions().mode() & 0o7777;
        assert_eq!(m, mode, "{who}: {path} is {m:o}, the layer says {mode:o}");
        let named = sidecar.lines().any(|l| {
            l.contains(&format!(r#""path":"{path}""#)) && l.contains(&format!(r#""name":"{name}""#))
        });
        match attribute(&p, name) {
            Some(v) => assert_eq!(v, bytes(value), "{who}: {path}: {name} on disk"),
            None => assert!(
                named,
                "{who}: {path}: {name} is neither on disk nor named in the sidecar: {sidecar}"
            ),
        }
    }
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("podvm-t1813-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The prefix that runs a command as uid 1000: only where this test runs
/// as root and `setpriv` exists.
fn as_uid_1000() -> Option<[&'static str; 6]> {
    let root = Command::new("id").arg("-u").output().ok();
    let is_root = root.is_some_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "0");
    let setpriv = ["/usr/bin/setpriv", "/bin/setpriv"]
        .into_iter()
        .find(|p| Path::new(p).exists());
    let (true, Some(setpriv)) = (is_root, setpriv) else {
        eprintln!("SKIP: the uid 1000 half needs root and setpriv");
        return None;
    };
    Some([
        setpriv,
        "--reuid",
        "1000",
        "--regid",
        "1000",
        "--clear-groups",
    ])
}

/// A scratch directory and a copy of podvm that uid 1000 can use: the
/// build tree can be private.
fn shared_scratch(tag: &str, podvm: &Path) -> (PathBuf, PathBuf) {
    let d = scratch(tag);
    std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o777)).unwrap();
    let copy = d.join("podvm");
    std::fs::copy(podvm, &copy).unwrap();
    std::fs::set_permissions(&copy, std::fs::Permissions::from_mode(0o755)).unwrap();
    (d, copy)
}

#[test]
fn setuid_and_setgid_modes_survive_extraction() {
    let podvm = PathBuf::from(env!("CARGO_BIN_EXE_podvm"));
    let d = scratch("self");
    let tar = layer(&d);
    assert_modes(&extract(&[], &podvm, &d, &tar, "t1813:1"), "this process");
    let _ = std::fs::remove_dir_all(&d);

    let Some(prefix) = as_uid_1000() else {
        return;
    };
    let (d, copy) = shared_scratch("u1000", &podvm);
    let tar = layer(&d);
    let rootfs = extract(&prefix, &copy, &d, &tar, "t1813:1");
    assert_modes(&rootfs, "uid 1000");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_layer_with_the_acls_of_gnu_tar_extracts_with_its_modes() {
    let podvm = PathBuf::from(env!("CARGO_BIN_EXE_podvm"));
    let d = scratch("acl-self");
    let tar = acl_layer(&d);
    assert_acls(&extract(&[], &podvm, &d, &tar, "t2065:1"), "this process");
    let _ = std::fs::remove_dir_all(&d);

    let Some(prefix) = as_uid_1000() else {
        return;
    };
    let (d, copy) = shared_scratch("acl-u1000", &podvm);
    let tar = acl_layer(&d);
    assert_acls(&extract(&prefix, &copy, &d, &tar, "t2065:1"), "uid 1000");
    let _ = std::fs::remove_dir_all(&d);
}
