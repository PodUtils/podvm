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
fn of_image(name: &str) -> bool {
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

/// The attributes of the image on the tree at `path`, with no follow of a
/// link. A file system with no attributes gives none.
pub fn on_disk(path: &Path) -> Vec<Attr> {
    on_disk_where(path, of_image)
}

/// [`on_disk`] with the names that `keep` takes (the root disk of the
/// machine tier also takes the POSIX ACLs, TODO/machine.md T-1994).
pub fn on_disk_where(path: &Path, keep: fn(&str) -> bool) -> Vec<Attr> {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
        return Vec::new();
    };
    let mut names = vec![0u8; 4096];
    // SAFETY: the buffer holds `names.len()` bytes; the path is NUL-ended.
    let n = unsafe { llistxattr(c.as_ptr(), names.as_mut_ptr(), names.len()) };
    if n <= 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for name in names[..n as usize]
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
    {
        let Ok(name) = std::str::from_utf8(name) else {
            continue;
        };
        if !keep(name) {
            continue;
        }
        let Ok(cn) = std::ffi::CString::new(name) else {
            continue;
        };
        let mut v = vec![0u8; 65536];
        // SAFETY: as above, with a NUL-ended name.
        let m = unsafe { lgetxattr(c.as_ptr(), cn.as_ptr(), v.as_mut_ptr(), v.len()) };
        if m >= 0 {
            v.truncate(m as usize);
            if name == "security.capability" {
                v = v2_of(v);
            }
            out.push((name.to_string(), v));
        }
    }
    out
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
}
