//! The calls of an extended attribute (OT-05 of the fourth outside pass,
//! TODO/enter.md T-1987): each maps its path, then makes the system call
//! itself, with no "next" symbol. Split out of `stat_calls` (the limit of
//! 500 lines, TODO/gate.md T-1674).

use core::ffi::{c_char, c_int, c_void};

macro_rules! path_syscall {
    ($name:ident, $nr:ident, ( $($arg:ident : $ty:ty),* ), $path:ident, $ret:ty) => {
        /// # Safety
        /// The payload's own contract for this entry point.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name($($arg: $ty),*) -> $ret {
            let _pin = crate::map::Scope::new(); // TODO/enter.md T-1972
            let mut buf = [0u8; crate::map::OUT];
            let p = unsafe {
                crate::map::prepare(
                    stringify!($name).as_bytes(),
                    crate::map::AT_FDCWD,
                    $path,
                    &mut buf,
                )
            };
            if p.is_null() {
                return -1;
            }
            let $path = p;
            let mut a = [0 as core::ffi::c_long; 6];
            let mut i = 0usize;
            $( a[i] = $arg as usize as core::ffi::c_long; i += 1; )*
            let _ = i;
            // SAFETY: the kernel's contract for this number, with the
            // payload's own arguments and the mapped path.
            unsafe { crate::raw_calls::forward(crate::raw_calls::$nr, a) as $ret }
        }
    };
}

// OT-05 (TODO/enter.md T-1987): the reads and writes of an attribute make the
// system call themselves. AlmaLinux 9's `libattr.so.1` exports its own
// `lgetxattr`, which calls the symbol `lgetxattr` again: the "next" symbol
// of these wrappers was that one, it bound back to this object, and `ls -l`
// recursed until its stack ended (SIGSEGV).
path_syscall!(getxattr, SYS_GETXATTR, (path: *const c_char, name: *const c_char, value: *mut c_void, size: usize), path, isize);
path_syscall!(lgetxattr, SYS_LGETXATTR, (path: *const c_char, name: *const c_char, value: *mut c_void, size: usize), path, isize);
path_syscall!(listxattr, SYS_LISTXATTR, (path: *const c_char, list: *mut c_char, size: usize), path, isize);
path_syscall!(llistxattr, SYS_LLISTXATTR, (path: *const c_char, list: *mut c_char, size: usize), path, isize);
path_syscall!(setxattr, SYS_SETXATTR, (path: *const c_char, name: *const c_char, value: *const c_void, size: usize, flags: c_int), path, c_int);
path_syscall!(lsetxattr, SYS_LSETXATTR, (path: *const c_char, name: *const c_char, value: *const c_void, size: usize, flags: c_int), path, c_int);
path_syscall!(removexattr, SYS_REMOVEXATTR, (path: *const c_char, name: *const c_char), path, c_int);
path_syscall!(lremovexattr, SYS_LREMOVEXATTR, (path: *const c_char, name: *const c_char), path, c_int);
