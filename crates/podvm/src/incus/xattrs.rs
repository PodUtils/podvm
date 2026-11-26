//! PodUtils/podrunner#316 ask 4 (TODO/machine.md T-1999): the extended
//! attributes of an image in the unified tarball of the incus tier.
//!
//! `pod-extract` (podcrates `v0.76.0`, podcrates@T-957) sets each `SCHILY.xattr.*`
//! record of a layer where the kernel lets it, and names each one that it
//! could not set in the sidecar row of its path, with its value (call C791
//! of podcrates): `security.capability` of `ping` is EPERM for a user that
//! is not root. The tarball carries the union as PAX records, so that
//! incusd sets them in the root of the instance.
//!
//! ⛔ Only the names of the image: `security.capability` and `user.*` from
//! the tree, and each name of the sidecar. A name that the host gives a
//! file (`security.selinux`) is the host's, not the image's.

use std::collections::HashMap;
use std::path::Path;

use serde_json::Value;

/// One attribute: its name and its value.
pub type Attr = (String, Vec<u8>);

/// The attributes of the sidecar that the extraction did not set, by path
/// (with no leading `/`), with a value. A row with no value (past a bound
/// of podcrates) carries nothing to set.
pub fn unset_of(sidecar: &str) -> HashMap<String, Vec<Attr>> {
    let mut m: HashMap<String, Vec<Attr>> = HashMap::new();
    for line in sidecar.lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let (Some(p), Some(list)) = (
            v.get("path").and_then(Value::as_str),
            v.get("xattrs").and_then(Value::as_array),
        ) else {
            continue;
        };
        for x in list {
            let (Some(name), Some(value)) = (
                x.get("name").and_then(Value::as_str),
                x.get("value").and_then(Value::as_str).and_then(hex),
            ) else {
                continue;
            };
            m.entry(p.trim_start_matches('/').to_string())
                .or_default()
                .push((name.to_string(), value));
        }
    }
    m
}

/// The value of `setfattr -v` hex form: `0x` and pairs of hex digits.
fn hex(s: &str) -> Option<Vec<u8>> {
    let d = s.strip_prefix("0x")?;
    if d.len() % 2 != 0 {
        return None;
    }
    (0..d.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(d.get(i..i + 2)?, 16).ok())
        .collect()
}

/// Whether a name on the tree is of the image (see the module note).
pub fn of_image(name: &str) -> bool {
    name == "security.capability" || name.starts_with("user.")
}

unsafe extern "C" {
    fn llistxattr(path: *const std::ffi::c_char, list: *mut u8, size: usize) -> isize;
    fn lgetxattr(
        path: *const std::ffi::c_char,
        name: *const std::ffi::c_char,
        value: *mut u8,
        size: usize,
    ) -> isize;
}

/// The longest list of names that the kernel gives for one file
/// (`XATTR_LIST_MAX`), each name with its NUL.
const LIST_MAX: usize = 64 * 1024;
/// The longest value of one attribute (`XATTR_SIZE_MAX`).
const VALUE_MAX: usize = 64 * 1024;
const ENODATA: i32 = 61;
const ERANGE: i32 = 34;
const ENOTSUP: i32 = 95;

/// The attributes of the image on the tree at `path`, with no follow of a
/// link. A file system with no attributes gives none. A list or a value
/// that the kernel does not give is an error with the path, not an empty
/// list (a file with attributes then loses none of them without a word).
/// The owner of the file may lack the read bit (a file
/// of mode 0000 is podvm's own, as the extraction made it): the `user.*`
/// values need it, so a refusal is read again with the owner's bit added
/// for the read. A link has no mode of its own, so it is read once.
pub fn on_disk_readable(
    path: &Path,
    md: &std::fs::Metadata,
    keep: fn(&str) -> bool,
) -> std::io::Result<Vec<Attr>> {
    match read_attrs(path, keep) {
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && !md.is_symlink() => {
            podvm_extract::tree::with_owner_bits(path, 0o400, || read_attrs(path, keep))
        }
        r => r,
    }
    .map_err(|e| in_path(path, e))
}

fn in_path(path: &Path, e: std::io::Error) -> std::io::Error {
    std::io::Error::new(e.kind(), format!("{}: {e}", path.display()))
}

/// The NUL-ended names of `c`: the size from `llistxattr(NULL, 0)`, a
/// read of that size, and a new try where the list grew between the two
/// calls (`ERANGE`). A list past [`LIST_MAX`] is not a list of the kernel.
fn list_names(c: &std::ffi::CStr) -> std::io::Result<Vec<u8>> {
    use std::io::{Error, ErrorKind};
    for _ in 0..4 {
        // SAFETY: a NULL list with size 0 asks for the size; the path is NUL-ended.
        let need = unsafe { llistxattr(c.as_ptr(), std::ptr::null_mut(), 0) };
        if need < 0 {
            return Err(Error::last_os_error());
        }
        let need = need as usize;
        if need == 0 {
            return Ok(Vec::new());
        }
        if need > LIST_MAX {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("the attribute names are {need} bytes, past the limit of {LIST_MAX}"),
            ));
        }
        let mut names = vec![0u8; need];
        // SAFETY: the buffer holds `names.len()` bytes; the path is NUL-ended.
        let n = unsafe { llistxattr(c.as_ptr(), names.as_mut_ptr(), names.len()) };
        if n >= 0 {
            names.truncate(n as usize);
            return Ok(names);
        }
        let e = Error::last_os_error();
        if e.raw_os_error() != Some(ERANGE) {
            return Err(e);
        }
    }
    Err(Error::other(
        "the attribute names changed in each of 4 reads",
    ))
}

/// The attributes of `path` with the names that `keep` takes (the root
/// disk of the machine tier also takes the POSIX ACLs, TODO/machine.md
/// T-1994), with no change of the mode. The errors have no path.
fn read_attrs(path: &Path, keep: fn(&str) -> bool) -> std::io::Result<Vec<Attr>> {
    use std::io::{Error, ErrorKind};
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "a NUL in the path"))?;
    let names = match list_names(&c) {
        Ok(n) => n,
        // A file system with no attributes has none to lose.
        Err(e) if e.raw_os_error() == Some(ENOTSUP) => return Ok(Vec::new()),
        Err(e) => return Err(Error::new(e.kind(), format!("listxattr: {e}"))),
    };
    let mut out = Vec::new();
    for raw in names.split(|&b| b == 0).filter(|s| !s.is_empty()) {
        let name = match std::str::from_utf8(raw) {
            Ok(n) => n,
            // The prefix decides: a name of the image that is not UTF-8
            // cannot go in a record, so it is named, not dropped.
            Err(_) if keep(&String::from_utf8_lossy(raw)) => {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "an attribute name is not UTF-8",
                ));
            }
            Err(_) => continue,
        };
        if !keep(name) {
            continue;
        }
        let cn = std::ffi::CString::new(name)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "a NUL in an attribute name"))?;
        let mut v = vec![0u8; VALUE_MAX];
        // SAFETY: as above, with a NUL-ended name.
        let m = unsafe { lgetxattr(c.as_ptr(), cn.as_ptr(), v.as_mut_ptr(), v.len()) };
        if m < 0 {
            let e = Error::last_os_error();
            // The attribute went between the list and the read.
            if e.raw_os_error() == Some(ENODATA) {
                continue;
            }
            return Err(Error::new(e.kind(), format!("getxattr {name}: {e}")));
        }
        v.truncate(m as usize);
        if name == "security.capability" {
            v = v2_of(v);
        }
        out.push((name.to_string(), v));
    }
    Ok(out)
}

/// A v3 file capability (with the root id of the namespace that set it,
/// here the extracting user's) as v2: the kernel of incusd's instance sets
/// v3 with its own root id from it. Any other value stays.
fn v2_of(v: Vec<u8>) -> Vec<u8> {
    const V3: u32 = 0x0300_0000;
    const V2: u32 = 0x0200_0000;
    if v.len() != 24 {
        return v;
    }
    let magic = u32::from_le_bytes([v[0], v[1], v[2], v[3]]);
    if magic & 0xFF00_0000 != V3 {
        return v;
    }
    let mut out = (V2 | (magic & 0x00FF_FFFF)).to_le_bytes().to_vec();
    out.extend_from_slice(&v[4..20]);
    out
}

/// The data of a PAX header of `attrs`: one record `LEN SCHILY.xattr.NAME=VALUE\n`
/// for each, where LEN counts the whole record with its own digits.
pub fn pax_records(attrs: &[Attr]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, value) in attrs {
        let body = 1 + "SCHILY.xattr.".len() + name.len() + 1 + value.len() + 1;
        let mut len = body + body.to_string().len();
        if len.to_string().len() != body.to_string().len() {
            len = body + len.to_string().len();
        }
        out.extend_from_slice(format!("{len} SCHILY.xattr.{name}=").as_bytes());
        out.extend_from_slice(value);
        out.push(b'\n');
    }
    out
}

/// The union of the tree's and the sidecar's attributes; the sidecar's
/// value wins for one name (the tree could not hold it).
pub fn merge(disk: Vec<Attr>, unset: Option<&Vec<Attr>>) -> Vec<Attr> {
    let mut out = disk;
    for (name, value) in unset.into_iter().flatten() {
        out.retain(|(n, _)| n != name);
        out.push((name.clone(), value.clone()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sidecar_and_the_records_carry_each_attribute() {
        let side = r#"{"path":"usr/bin/ping","uid":0,"gid":0,"mode":"0755","xattrs":[{"name":"security.capability","value":"0x010000020a","reason":"EPERM"},{"name":"user.x","reason":"a bound"}]}"#;
        let m = unset_of(side);
        assert_eq!(
            m["usr/bin/ping"],
            vec![("security.capability".to_string(), vec![1, 0, 0, 2, 10])],
            "a row with no value carries nothing"
        );
        for (rec, want) in [
            (
                pax_records(&[("user.a".into(), b"b".to_vec())]),
                &b"25 SCHILY.xattr.user.a=b\n"[..],
            ),
            (pax_records(&[("user.a".into(), vec![b'x'; 85])]), &[][..]),
        ] {
            let text = String::from_utf8_lossy(&rec);
            let (len, _) = text.split_once(' ').unwrap();
            assert_eq!(len.parse::<usize>().unwrap(), rec.len(), "{text}");
            if !want.is_empty() {
                assert_eq!(rec, want);
            }
        }
        let both = merge(
            vec![
                ("user.k".into(), b"1".to_vec()),
                ("security.capability".into(), b"old".to_vec()),
            ],
            m.get("usr/bin/ping"),
        );
        assert_eq!(both.len(), 2);
        assert!(both.contains(&("security.capability".to_string(), vec![1, 0, 0, 2, 10])));
        assert!(of_image("user.k") && !of_image("security.selinux"));
        assert_eq!(hex("0x0"), None);
        let v3 = hex("0x0100000300200000000000000000000000000000e8030000").unwrap();
        assert_eq!(
            v2_of(v3),
            hex("0x0100000200200000000000000000000000000000").unwrap()
        );
        assert_eq!(v2_of(vec![1, 2, 3]), vec![1, 2, 3]);
    }

    unsafe extern "C" {
        fn lsetxattr(
            path: *const std::ffi::c_char,
            name: *const std::ffi::c_char,
            value: *const u8,
            size: usize,
            flags: i32,
        ) -> i32;
    }

    /// A directory where `n` attributes of `len`-byte names fit, or none:
    /// ext4 keeps the attributes of a file in one block, tmpfs in 128 KiB.
    fn room_for(n: usize, len: usize) -> Option<std::path::PathBuf> {
        use std::os::unix::ffi::OsStrExt;
        for dir in [std::env::temp_dir(), "/dev/shm".into()] {
            let f = dir.join(format!("podvm-xattr-{}-{n}-{len}", std::process::id()));
            if std::fs::write(&f, b"x").is_err() {
                continue;
            }
            let c = std::ffi::CString::new(f.as_os_str().as_bytes()).unwrap();
            let all = (0..n).all(|i| {
                let name = format!("user.{i:04}");
                let name = format!("{name:x<len$}");
                let cn = std::ffi::CString::new(name).unwrap();
                // SAFETY: NUL-ended path and name; the value is 1 byte.
                unsafe { lsetxattr(c.as_ptr(), cn.as_ptr(), b"v".as_ptr(), 1, 0) == 0 }
            });
            if all {
                return Some(f);
            }
            let _ = std::fs::remove_file(&f);
        }
        None
    }

    /// One rule for each size of the list of names (the buffer of 4096
    /// bytes lost every attribute of a file with more): the kernel's
    /// answer is read in full, or the refusal is an error with the path.
    #[test]
    fn a_list_of_names_of_any_size_is_read_in_full_or_refused_by_name() {
        for (label, n, len) in [
            ("none", 0, 0),
            ("few", 3, 20),
            ("past 4096", 60, 100),
            ("past 32 KiB", 300, 120),
        ] {
            let (f, md);
            if n == 0 {
                f = std::env::temp_dir().join(format!("podvm-xattr-none-{}", std::process::id()));
                std::fs::write(&f, b"x").unwrap();
            } else if let Some(p) = room_for(n, len) {
                f = p;
            } else {
                // ext4 keeps the attributes of a file in one block: a host
                // with no tmpfs under /dev/shm cannot hold this case.
                eprintln!("{label}: no directory holds {n} attributes; not run");
                continue;
            }
            md = std::fs::symlink_metadata(&f).unwrap();
            let got = on_disk_readable(&f, &md, |n| n.starts_with("user."));
            let _ = std::fs::remove_file(&f);
            let got = got.unwrap_or_else(|e| panic!("{label}: {e}"));
            assert_eq!(got.len(), n, "{label}: every attribute is read");
            assert!(got.iter().all(|(_, v)| v == b"v"), "{label}");
        }
        let gone = std::env::temp_dir().join("podvm-xattr-gone");
        let md = std::fs::symlink_metadata(std::env::temp_dir()).unwrap();
        let e = on_disk_readable(&gone, &md, of_image)
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("podvm-xattr-gone") && e.contains("listxattr"),
            "{e}"
        );
    }
}
