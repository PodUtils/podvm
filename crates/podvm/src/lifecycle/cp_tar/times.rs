//! TODO/cli.md T-2108 (OR-B42-07): `cp` keeps the modification time, as
//! docker does in both directions. docker sets the access time to the
//! modification time too (its tar stream has no other time), so one pair
//! of values goes to both. The call is `utimensat` with
//! `AT_SYMLINK_NOFOLLOW`: a link of the tree takes its own time and a link
//! is never followed to the file behind it.

use std::ffi::{c_char, c_int, c_long};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

#[repr(C)]
struct Timespec {
    tv_sec: c_long,
    tv_nsec: c_long,
}

unsafe extern "C" {
    fn utimensat(dirfd: c_int, path: *const c_char, times: *const Timespec, flags: c_int) -> c_int;
}

const AT_FDCWD: c_int = -100;
const AT_SYMLINK_NOFOLLOW: c_int = 0x100;

/// Set the access and modification time of `path` itself (never the
/// target of a link) to `sec` seconds and `nsec` nanoseconds after the
/// epoch. A value out of range is refused, not clamped.
pub(in crate::lifecycle) fn set(path: &Path, sec: i64, nsec: i64) -> std::io::Result<()> {
    if !(0..1_000_000_000).contains(&nsec) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{nsec} ns is not a valid fraction of a second"),
        ));
    }
    let sec = c_long::try_from(sec).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{sec} s does not fit the time of this host"),
        )
    })?;
    let mut name = path.as_os_str().as_bytes().to_vec();
    if name.contains(&0) {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
    }
    name.push(0);
    let at = || Timespec {
        tv_sec: sec,
        tv_nsec: nsec as c_long,
    };
    let times = [at(), at()];
    // SAFETY: `name` is NUL-ended and lives through the call; `times` holds
    // the two `timespec` values that the call reads.
    let r = unsafe {
        utimensat(
            AT_FDCWD,
            name.as_ptr().cast(),
            times.as_ptr(),
            AT_SYMLINK_NOFOLLOW,
        )
    };
    if r == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Give `dst` the modification time of the source whose metadata is `from`
/// (read without following a link).
pub(in crate::lifecycle) fn keep(from: &std::fs::Metadata, dst: &Path) -> std::io::Result<()> {
    set(dst, from.mtime(), from.mtime_nsec())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One table of the times that `set` takes and refuses, and the proof
    /// that a link takes its own time and its target keeps its own.
    #[test]
    fn a_time_is_set_on_the_name_and_never_on_the_target_of_a_link() {
        let dir = std::env::temp_dir().join(format!(
            "podvm-times-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("f");
        std::fs::write(&file, b"x").unwrap();
        let link = dir.join("l");
        std::os::unix::fs::symlink("f", &link).unwrap();
        let before = std::fs::metadata(&file).unwrap().mtime();

        set(&link, 1_000_000_000, 5).unwrap();
        let l = std::fs::symlink_metadata(&link).unwrap();
        assert_eq!((l.mtime(), l.mtime_nsec()), (1_000_000_000, 5));
        assert_eq!(std::fs::metadata(&file).unwrap().mtime(), before);

        set(&file, 1_100_000_000, 999_999_999).unwrap();
        let f = std::fs::metadata(&file).unwrap();
        assert_eq!((f.mtime(), f.mtime_nsec()), (1_100_000_000, 999_999_999));
        assert_eq!(f.atime(), 1_100_000_000);

        let d = std::fs::metadata(&dir).unwrap();
        keep(&f, &dir).unwrap();
        assert_ne!(d.mtime(), 1_100_000_000);
        assert_eq!(std::fs::metadata(&dir).unwrap().mtime(), 1_100_000_000);

        for (sec, nsec) in [(1, -1), (1, 1_000_000_000)] {
            assert!(set(&file, sec, nsec).is_err(), "{sec} {nsec}");
        }
        assert!(set(&dir.join("gone"), 1, 0).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
