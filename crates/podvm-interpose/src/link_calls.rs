//! The calls that act on a link itself where the caller asks for it
//! (`AT_SYMLINK_NOFOLLOW`), PodUtils/podrunner#178 (TODO/interpose.md
//! T-1915).

use super::*;

/// [`path_at_int!`] for a call with `AT_SYMLINK_NOFOLLOW` in `$flags`: the
/// last link stays a link (T-1725, T-1915).
macro_rules! path_at_int_flags {
    ($name:ident, $real:ident, ( $($arg:ident : $ty:ty),* ), $dirfd:ident, $path:ident, $flags:ident) => {
        /// # Safety
        /// The payload's own contract for this entry point.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name($($arg: $ty),*) -> c_int {
            let Some(f) = $real() else { set_errno(EINVAL); return -1; };
            let _pin = crate::map::Scope::new(); // TODO/enter.md T-1972
            let mut buf = [0u8; crate::map::OUT];
            let name = stringify!($name).as_bytes();
            let follow = $flags & crate::owner_calls::AT_SYMLINK_NOFOLLOW == 0;
            let p = unsafe {
                crate::map::prepare_follow(
                    name,
                    $dirfd,
                    $path,
                    crate::map::is_write_call(name),
                    follow,
                    &mut buf,
                )
            };
            if p.is_null() {
                return -1;
            }
            let $path = p;
            unsafe { f($($arg),*) }
        }
    };
}

// PodUtils/podrunner#178 (TODO/interpose.md T-1915): AT_SYMLINK_NOFOLLOW
// acts on the link itself. GNU tar sets the time and the mode of an
// extracted link that way; the mapping followed the link, and a link
// before its target (or a dangling one) failed with ENOENT.
path_at_int_flags!(fchmodat, next_fchmodat, (dirfd: c_int, path: *const c_char, mode: c_uint, flags: c_int), dirfd, path, flags);
path_at_int_flags!(utimensat, next_utimensat, (dirfd: c_int, path: *const c_char, times: *const c_void, flags: c_int), dirfd, path, flags);
