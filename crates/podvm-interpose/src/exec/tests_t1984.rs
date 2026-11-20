//! TODO/enter.md T-1984: an exec of the dynamic loader takes the exec rule
//! of the program that the loader would run.

use super::ldso::{each_program, is_loader};
use super::tests::{elf, put, tree};
use super::*;
use std::ffi::CString;
use std::os::unix::fs::symlink;

/// The arguments that a check sees, and the answer, for each reading of
/// the loaders' options: one table, one rule (`ldso::each_program`).
#[test]
fn the_program_of_the_loader_is_each_argument_that_a_loader_can_run() {
    let names: [(&[u8], bool); 12] = [
        (b"/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2", true),
        (b"/lib/ld-linux-aarch64.so.1", true),
        (b"/lib/x86_64-linux-gnu/ld-2.28.so", true),
        (b"/lib/ld-musl-x86_64.so.1", true),
        (b"/lib64/ld64.so.2", true),
        (b"/usr/lib/ld.so.1", true),
        (b"/usr/bin/ld", false),
        (b"/usr/bin/ldd", false),
        // musl's install makes `ld-musl-ARCH.so.1` a link to its libc,
        // so the resolved file of the loader is the libc (podcrates'
        // COORD-149); glibc's libc.so.6 runs no program, so its rule only
        // adds a check.
        (b"/lib/libc.musl-x86_64.so.1", true),
        (b"/usr/lib/musl/lib/libc.so", true),
        (b"/lib/x86_64-linux-gnu/libc.so.6", true),
        (b"/lib/x86_64-linux-gnu/libcrypt.so.1", false),
    ];
    for (n, want) in names {
        assert_eq!(is_loader(n), want, "{}", String::from_utf8_lossy(n));
    }
    // (arguments, the names that the check sees, the answer where the check
    // lets each one run)
    let rows: [(&[&str], &[&str], bool); 12] = [
        (&["/tmp/p", "a"], &["/tmp/p"], true),
        (&["--library-path", "/tmp", "/tmp/p"], &["/tmp/p"], true),
        (&["--inhibit-cache", "--argv0", "x", "./p"], &["./p"], true),
        (&["--list", "/tmp/p"], &["/tmp/p"], true),
        (
            &["--library-path=/l", "/tmp/p"],
            &["--library-path=/l", "/tmp/p"],
            true,
        ),
        (&["--help"], &[], true),
        (&[], &[], true),
        // A name with no `/`: glibc's loader searches it, so it is refused.
        (&["--library-path", "/tmp", "p"], &[], false),
        (&["--", "/tmp/p"], &[], false),
        (&["--unknown", "/tmp/p"], &[], false),
        (&["--argv0=x", "/tmp/p"], &[], false),
        (&["-x"], &[], false),
    ];
    for (args, seen, want) in rows {
        let mut got: Vec<&str> = Vec::new();
        let ok = each_program(args.iter().map(|a| a.as_bytes()), |p| {
            got.push(core::str::from_utf8(p).unwrap());
            true
        });
        assert_eq!((ok, got.as_slice()), (want, seen), "{args:?}");
    }
    // The first program that the check refuses ends the walk.
    assert!(!each_program(
        [b"--library-path=/l".as_slice(), b"/tmp/p"],
        |p| p != b"/tmp/p"
    ));
}

fn launch(path: &str, args: &[&str]) -> Result<String, c_int> {
    let p = CString::new(path).unwrap();
    let a: Vec<CString> = args.iter().map(|s| CString::new(*s).unwrap()).collect();
    let mut av: Vec<*const c_char> = a.iter().map(|s| s.as_ptr()).collect();
    av.push(core::ptr::null());
    let ev = [core::ptr::null()];
    let (mut s, mut buf) = (Box::new(Scratch::new()), [0u8; OUT]);
    match unsafe {
        route(
            b"execve",
            AT_FDCWD,
            p.as_ptr(),
            av.as_ptr(),
            ev.as_ptr(),
            false,
            &mut s,
            &mut buf,
        )
    } {
        Route::Launch(l) => Ok(unsafe { std::ffi::CStr::from_ptr(l.path) }
            .to_string_lossy()
            .into_owned()),
        Route::Pass(_) => Ok("pass".to_string()),
        Route::Fail => Err(crate::errno()),
    }
}

/// Each route of an exec through the loader, by each of its names: a
/// program out of `exec=` gets `EACCES`, one in it runs, through podvm's
/// handler (guest names) and through the kernel (host names); with no
/// exec table each one runs.
#[test]
fn an_exec_of_the_loader_takes_the_exec_rule_of_its_program() {
    let d = tree("t1984");
    let root = d.to_string_lossy().to_string();
    for sub in ["usr/lib/x86_64-linux-gnu", "tmp"] {
        std::fs::create_dir_all(d.join(sub)).unwrap();
    }
    let ld = "/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2";
    put(&d.join(&ld[1..]), &elf(1, b""));
    put(&d.join("lib/ld-2.28.so"), &elf(1, b""));
    symlink(ld, d.join("lib64/ld-linux-x86-64.so.2")).unwrap();
    put(&d.join("usr/bin/ok"), &elf(1, b""));
    put(&d.join("tmp/prog"), &elf(1, b""));
    put(
        &d.join("usr/bin/ldscr"),
        b"#!/lib64/ld-linux-x86-64.so.2 /tmp/prog\n",
    );
    put(&d.join("usr/bin/ldok"), b"#!/lib64/ld-linux-x86-64.so.2\n");
    let table = format!("{root}/usr:{root}/lib:{root}/lib64:{root}/bin");
    // SAFETY: the interposer tests run with one thread (gate.yml).
    unsafe {
        std::env::set_var("PODVM_MAPS", format!("/:{root}"));
        std::env::set_var("PODVM_DISPATCH_SELF", "/opt/podvm");
        std::env::set_var("PODVM_EXEC_HOST", &table);
    }
    let host_prog = format!("{root}/tmp/prog");
    let host_ok = format!("{root}/usr/bin/ok");
    let rows: [(&str, &[&str], Result<&str, c_int>); 10] = [
        (ld, &["ld.so", "/tmp/prog"], Err(EACCES)),
        (
            ld,
            &["ld.so", "--library-path", "/tmp", "/tmp/prog"],
            Err(EACCES),
        ),
        (
            ld,
            &["ld.so", "--library-path", "/tmp", "prog"],
            Err(EACCES),
        ),
        (
            "/lib64/ld-linux-x86-64.so.2",
            &["ld.so", "/tmp/prog"],
            Err(EACCES),
        ),
        ("/lib/ld-2.28.so", &["ld.so", "/tmp/prog"], Err(EACCES)),
        // A `#!` line whose interpreter is the loader, and its argument.
        ("/usr/bin/ldscr", &["ldscr"], Err(EACCES)),
        ("/usr/bin/ldok", &["ldok"], Ok("/opt/podvm")),
        (ld, &["ld.so", "/usr/bin/ok"], Ok("/opt/podvm")),
        (ld, &["ld.so", "/tmp/absent"], Ok("/opt/podvm")),
        (ld, &["ld.so", "--version"], Ok("/opt/podvm")),
    ];
    for (path, args, want) in rows {
        assert_eq!(
            launch(path, args).as_deref().map_err(|e| *e),
            want,
            "{path} {args:?}"
        );
    }
    // Through the kernel (podvm names no handler), the loader opens a host
    // name: the host file of the program takes the rule.
    unsafe { std::env::remove_var("PODVM_DISPATCH_SELF") };
    let host_ld = format!("{root}{ld}");
    assert_eq!(launch(ld, &["ld.so", &host_prog]), Err(EACCES));
    assert_eq!(launch(ld, &["ld.so", &host_ok]), Ok(host_ld.clone()));
    // With no exec table, each route runs.
    unsafe { std::env::remove_var("PODVM_EXEC_HOST") };
    assert_eq!(launch(ld, &["ld.so", &host_prog]), Ok(host_ld));
    unsafe { std::env::remove_var("PODVM_MAPS") };
    let _ = std::fs::remove_dir_all(&d);
}
