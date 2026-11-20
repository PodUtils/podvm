//! The calls that act on a link itself where the caller asks for it
//! (`AT_SYMLINK_NOFOLLOW`), PodUtils/podrunner#178 (TODO/interpose.md
//! T-1915).

use super::*;

/// `AT_EMPTY_PATH`: an empty path names the descriptor itself.
const AT_EMPTY_PATH: c_int = 0x1000;

/// [`path_at_int!`] for a call with `AT_SYMLINK_NOFOLLOW` in `$flags`: the
/// last link stays a link (T-1725, T-1915).
macro_rules! path_at_int_flags {
    ($name:ident, $real:ident, ( $($arg:ident : $ty:ty),* ), $dirfd:ident, $path:ident, $flags:ident) => {
        /// # Safety
        /// The payload's own contract for this entry point.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name($($arg: $ty),*) -> c_int {
            let Some(f) = $real() else { set_errno(EINVAL); return -1; };
            let name = stringify!($name).as_bytes();
            // TODO/enter.md T-1985: a null path (`utimensat`), or an empty
            // one with `AT_EMPTY_PATH`, is the descriptor's own file.
            if $path.is_null() || ($flags & AT_EMPTY_PATH != 0 && unsafe { *$path } == 0) {
                if !unsafe { crate::map::meta_fd_allowed(name, $dirfd) } {
                    return -1;
                }
                return unsafe { f($($arg),*) };
            }
            let _pin = crate::map::Scope::new(); // TODO/enter.md T-1972
            let mut buf = [0u8; crate::map::OUT];
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

// TODO/enter.md T-1985: the descriptor forms of a change of mode or times,
// which the confine layer judges on the file of the descriptor.
crate::real!(pub fn next_fchmod = "fchmod"(c_int, c_uint) -> c_int);
crate::real!(pub fn next_futimens = "futimens"(c_int, *const c_void) -> c_int);
crate::real!(pub fn next_futimes = "futimes"(c_int, *const c_void) -> c_int);

macro_rules! fd_meta {
    ($name:ident, $real:ident, ( $($arg:ident : $ty:ty),* ), $fd:ident) => {
        /// # Safety
        /// The payload's own contract for this entry point.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name($($arg: $ty),*) -> c_int {
            let Some(f) = $real() else { set_errno(EINVAL); return -1; };
            if !unsafe { crate::map::meta_fd_allowed(stringify!($name).as_bytes(), $fd) } {
                return -1;
            }
            unsafe { f($($arg),*) }
        }
    };
}

fd_meta!(fchmod, next_fchmod, (fd: c_int, mode: c_uint), fd);
fd_meta!(futimens, next_futimens, (fd: c_int, times: *const c_void), fd);
fd_meta!(futimes, next_futimes, (fd: c_int, times: *const c_void), fd);
