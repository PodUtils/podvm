//! The program that a dynamic loader runs (TODO/enter.md T-1984).
//!
//! Landlock checks `EXECUTE` on the file that `execve` names. The dynamic
//! loader of an image (`ld-linux-x86-64.so.2`, `ld-musl-x86_64.so.1`) must
//! be executable, and it runs the program of its first argument: it only
//! reads and maps that program, so `ld.so /tmp/prog` ran a program that
//! `exec=` of `--podvm-confine` denies. Where podvm sees the exec (the
//! interposer of the loader family, the handler of the syscall tier), the
//! exec of a loader takes the exec rule of each argument that the loader
//! can take as its program.
//!
//! Pure over bytes: no libc call, no allocation, no crate item. The
//! interposer and `podvm-dispatch` include this file as it is.

/// True where the base name of a resolved path names a dynamic loader:
/// `ld-linux-x86-64.so.2`, `ld-linux-aarch64.so.1`, `ld-2.28.so` (the file
/// of an older glibc), `ld-musl-x86_64.so.1`, `ld64.so.2`, `ld.so.1`. The
/// caller resolves each link first, so another name of the same file (a
/// link, `/lib64/...` or `/usr/lib/...`) gives the name of the file.
///
/// musl's install makes `ld-musl-ARCH.so.1` a link to its libc
/// (`libc.musl-ARCH.so.1`, `libc.so`), so a resolved name of a libc counts
/// too; glibc's `libc.so.6` runs no program, so it only adds a check.
pub fn is_loader(path: &[u8]) -> bool {
    let base = path.rsplit(|&b| b == b'/').next().unwrap_or(path);
    let named = base.starts_with(b"ld-") || base.starts_with(b"ld.") || base.starts_with(b"ld64.");
    let libc = base.starts_with(b"libc.so") || base.starts_with(b"libc.musl-");
    (named && base.windows(3).any(|w| w == b".so")) || libc
}

/// The options of glibc's and musl's loaders that take the next argument
/// as their value.
const VALUED: [&[u8]; 7] = [
    b"--library-path",
    b"--inhibit-rpath",
    b"--audit",
    b"--preload",
    b"--argv0",
    b"--glibc-hwcaps-prefix",
    b"--glibc-hwcaps-mask",
];

/// The options of the loaders that take no value.
const FLAGS: [&[u8]; 7] = [
    b"--list",
    b"--verify",
    b"--inhibit-cache",
    b"--list-tunables",
    b"--list-diagnostics",
    b"--help",
    b"--version",
];

/// Give `check` each argument that a loader can take as its program, from
/// the arguments after its own name, and answer false at the first one
/// that `check` refuses. True where the loader runs no program (`--help`)
/// or `check` lets each one run.
///
/// The loaders do not read the options the same way, so each reading
/// counts (fail safe):
///
/// - a valued option (`--library-path X`) skips its value;
/// - `--name=value` is an option for musl and a program for a glibc that
///   does not know the form: both;
/// - `--` ends the options for musl and a newer glibc, and is a program
///   for an older glibc: both, and the next argument;
/// - another `--name` is a program for an older glibc, and a refusal of
///   the others;
/// - each other argument is the program.
///
/// A program with no `/` gets false with no call of `check`: glibc's
/// loader searches such a name as a library (`--library-path`,
/// `LD_LIBRARY_PATH`, its cache), which the check cannot follow.
pub fn each_program<'a, I, F>(args: I, mut check: F) -> bool
where
    I: IntoIterator<Item = &'a [u8]>,
    F: FnMut(&'a [u8]) -> bool,
{
    let mut one = |p: &'a [u8]| p.contains(&b'/') && check(p);
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if VALUED.contains(&a) {
            it.next();
            continue;
        }
        if FLAGS.contains(&a) {
            continue;
        }
        if a == b"--" {
            return one(a) && it.next().is_none_or(&mut one);
        }
        let valued = a.starts_with(b"--")
            && a.iter()
                .position(|&b| b == b'=')
                .is_some_and(|eq| VALUED.contains(&&a[..eq]));
        if valued {
            if !one(a) {
                return false;
            }
            continue;
        }
        return one(a);
    }
    true
}
