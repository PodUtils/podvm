//! A static test payload of the native-static rung. Each verb prints one
//! answer; the tests of `podvm-dispatch` read it.

use std::io::Read;
use std::os::unix::process::CommandExt;
use std::sync::atomic::{AtomicBool, Ordering};

static GOT: AtomicBool = AtomicBool::new(false);

extern "C" fn on_usr1(_: i32) {
    GOT.store(true, Ordering::SeqCst);
}

fn cat(p: &str) -> String {
    let mut s = String::new();
    match std::fs::File::open(p) {
        Ok(mut f) => {
            let _ = f.read_to_string(&mut s);
            s.trim_end().to_string()
        }
        Err(e) => format!("ERR {e}"),
    }
}

unsafe extern "C" {
    fn syscall(n: i64, ...) -> i64;
    fn sigaltstack(new: *const [i64; 3], old: *mut [i64; 3]) -> i32;
    fn fork() -> i32;
    fn waitpid(pid: i32, status: *mut i32, opt: i32) -> i32;
    fn signal(sig: i32, h: extern "C" fn(i32)) -> usize;
    fn raise(sig: i32) -> i32;
    fn _exit(c: i32) -> !;
    fn uname(buf: *mut u8) -> i32;
}

fn main() {
    // T-1976: each verb on an alternate signal stack of this many bytes.
    if let Some(size) = std::env::var("PODVM_PROBE_ALT")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
    {
        let mem: &'static mut [u8] = Box::leak(vec![0u8; size].into_boxed_slice());
        let st = [mem.as_mut_ptr() as i64, 0, size as i64];
        // SAFETY: a stack_t of a leaked buffer, which lives to the end.
        if unsafe { sigaltstack(&st, std::ptr::null_mut()) } != 0 {
            println!("ERR sigaltstack");
            return;
        }
    }
    let args: Vec<String> = std::env::args().collect();
    let verb = args.get(1).map(String::as_str).unwrap_or("");
    let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
    match verb {
        "cat" => println!("{}", cat(&arg(2))),
        // PodUtils/podrunner#303: openat as glibc calls it, with AT_FDCWD
        // in the low 32 bits of the register and zeros above.
        "cat-zx" => {
            let path = std::ffi::CString::new(arg(2)).unwrap_or_default();
            // SAFETY: openat(2) with a NUL-ended path that lives for the call.
            let fd = unsafe { syscall(257, 0xFFFF_FF9Ci64, path.as_ptr(), 0i64, 0i64) };
            if fd < 0 {
                println!("ERR {}", std::io::Error::last_os_error());
            } else {
                use std::os::fd::FromRawFd;
                let mut s = String::new();
                // SAFETY: fd is a new descriptor that this File owns.
                let _ = unsafe { std::fs::File::from_raw_fd(fd as i32) }.read_to_string(&mut s);
                println!("{}", s.trim_end());
            }
        }
        "cwd" => println!(
            "{}",
            std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|e| format!("ERR {e}"))
        ),
        "cd" => match std::env::set_current_dir(arg(2)) {
            Ok(()) => println!(
                "{} {}",
                std::env::current_dir().unwrap().display(),
                cat(&arg(3))
            ),
            Err(e) => println!("ERR {e}"),
        },
        "exe" => println!(
            "{}",
            std::fs::read_link("/proc/self/exe")
                .map(|p| p.display().to_string())
                .unwrap_or_else(|e| format!("ERR {e}"))
        ),
        // T-1988: the kind of PATH through statx(2) as coreutils' `ls -d`
        // and `stat` call it: AT_FDCWD as glibc fills the register, with
        // AT_SYMLINK_NOFOLLOW | AT_NO_AUTOMOUNT.
        "kind" => {
            let path = std::ffi::CString::new(arg(2)).unwrap_or_default();
            let mut st = [0u8; 256];
            // SAFETY: statx(2) writes one struct statx of 256 bytes.
            let r = unsafe {
                syscall(
                    332,
                    0xFFFF_FF9Ci64,
                    path.as_ptr(),
                    0x900i64,
                    0x7ffi64,
                    st.as_mut_ptr(),
                )
            };
            let mode = u16::from_ne_bytes([st[28], st[29]]);
            println!(
                "{}",
                if r < 0 {
                    format!("ERR {}", std::io::Error::last_os_error())
                } else if mode & 0o170000 == 0o040000 {
                    "dir".to_string()
                } else {
                    "other".to_string()
                }
            );
        }
        "ls" => {
            let mut v: Vec<String> = std::fs::read_dir(arg(2))
                .map(|d| {
                    d.filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect()
                })
                .unwrap_or_default();
            v.sort();
            println!("{}", v.join(" "));
        }
        "write" => {
            let r = std::fs::write(arg(2), arg(3)).and_then(|()| std::fs::rename(arg(2), arg(4)));
            println!(
                "{}",
                match r {
                    Ok(()) => cat(&arg(4)),
                    Err(e) => format!("ERR {e}"),
                }
            );
        }
        "fork-cat" => {
            // SAFETY: the child prints and exits with no other thread.
            let pid = unsafe { fork() };
            if pid == 0 {
                println!("child {}", cat(&arg(2)));
                // SAFETY: exit the child.
                unsafe { _exit(7) };
            }
            let mut st = 0;
            // SAFETY: st lives for the call.
            unsafe { waitpid(pid, &mut st, 0) };
            println!("parent status {}", (st >> 8) & 0xff);
        }
        "thread-cat" => {
            let p = arg(2);
            let got = std::thread::spawn(move || cat(&p)).join().unwrap();
            println!("thread {got}");
        }
        "signal" => {
            // SAFETY: a handler that stores to an atomic.
            unsafe {
                signal(10, on_usr1);
                raise(10);
            }
            println!("usr1 {}", GOT.load(Ordering::SeqCst));
        }
        // exec on an alternate signal stack of SIZE bytes (gate 44312445769).
        "exec-alt" => {
            let size: usize = arg(2).parse().unwrap_or(8192);
            let mem: &'static mut [u8] = Box::leak(vec![0u8; size].into_boxed_slice());
            let st = [mem.as_mut_ptr() as i64, 0, size as i64];
            // SAFETY: a stack_t of a leaked buffer, which lives to the exec.
            if unsafe { sigaltstack(&st, std::ptr::null_mut()) } != 0 {
                println!("ERR sigaltstack");
                return;
            }
            let e = std::process::Command::new(arg(3)).args(&args[4..]).exec();
            println!("ERR {e}");
        }
        "exec" => {
            let e = std::process::Command::new(arg(2)).args(&args[3..]).exec();
            println!("ERR {e}");
        }
        // TODO/enter.md T-1985: one raw call of the family of a change of
        // mode, owner or times on PATH, with no change (the mode 0644 of
        // the test files, the owner -1, the time now): "meta ok", or
        // "meta ERR" and the errno.
        "meta" => {
            let c = std::ffi::CString::new(arg(3)).unwrap_or_default();
            let p = c.as_ptr() as i64;
            let at = 0xFFFF_FF9Ci64;
            let e = c"".as_ptr() as i64;
            // SAFETY: each call reads the path and the probe's own memory.
            let r = unsafe {
                let fd = || syscall(2, p, 0i64, 0i64);
                match arg(2).as_str() {
                    "chmod" => syscall(90, p, 0o644i64),
                    "fchmod" => syscall(91, fd(), 0o644i64),
                    "fchmodat" => syscall(268, at, p, 0o644i64),
                    "fchmodat2" => syscall(452, at, p, 0o644i64, 0i64),
                    "chown" => syscall(92, p, -1i64, -1i64),
                    "fchown" => syscall(93, fd(), -1i64, -1i64),
                    "lchown" => syscall(94, p, -1i64, -1i64),
                    "fchownat" => syscall(260, at, p, -1i64, -1i64, 0i64),
                    "fchownat-empty" => syscall(260, fd(), e, -1i64, -1i64, 0x1000i64),
                    "utime" => syscall(132, p, 0i64),
                    "utimes" => syscall(235, p, 0i64),
                    "utimensat" => syscall(280, at, p, 0i64, 0i64),
                    "utimensat-fd" => syscall(280, fd(), 0i64, 0i64, 0i64),
                    "futimesat" => syscall(261, at, p, 0i64),
                    _ => -2,
                }
            };
            match r {
                0 => println!("meta ok"),
                -2 => println!("meta no call {:?}", arg(2)),
                _ => println!(
                    "meta ERR {}",
                    std::io::Error::last_os_error().raw_os_error().unwrap_or(-1)
                ),
            }
        }
        "spawn" => {
            let o = std::process::Command::new(arg(2)).args(&args[3..]).output();
            match o {
                Ok(o) => println!(
                    "spawn {} {}",
                    o.status.code().unwrap_or(-1),
                    String::from_utf8_lossy(&o.stdout).trim_end()
                ),
                Err(e) => println!("ERR {e}"),
            }
        }
        // T-1652, found by T-1666: a detached thread ends (musl unmaps its
        // stack and exits), and the process goes on.
        "detached" => {
            drop(std::thread::spawn(|| 7));
            std::thread::sleep(std::time::Duration::from_millis(300));
            println!("detached ok");
        }
        // T-1665: a TCP connect; ok, or the errno.
        "connect" => match std::net::TcpStream::connect(arg(2)) {
            Ok(_) => println!("connect ok"),
            Err(e) => println!("connect {}", e.raw_os_error().unwrap_or(-1)),
        },
        // T-1657: the release of uname(2).
        "uname" => {
            let mut u = [0u8; 390];
            // SAFETY: uname writes 390 bytes.
            let r = unsafe { uname(u.as_mut_ptr()) };
            let rel = &u[130..195];
            let end = rel.iter().position(|&c| c == 0).unwrap_or(rel.len());
            println!("{r} {}", String::from_utf8_lossy(&rel[..end]));
        }
        // T-1657: the cap of /tmp, in one process tree: a file of N bytes,
        // a second of M bytes from a child, the first removed, the second
        // again.
        "tmpcap" => {
            let n: usize = arg(2).parse().unwrap_or(0);
            let m: usize = arg(3).parse().unwrap_or(0);
            let put = |p: &str, k: usize| match std::fs::write(p, vec![b'x'; k]) {
                Ok(()) => "ok".to_string(),
                Err(e) => format!("{}", e.raw_os_error().unwrap_or(-1)),
            };
            let a = put("/tmp/a", n);
            // SAFETY: the child writes and exits with no other thread.
            let pid = unsafe { fork() };
            if pid == 0 {
                let b = put("/tmp/b", m);
                // SAFETY: exit the child.
                unsafe { _exit(if b == "ok" { 0 } else { b.parse().unwrap_or(1) }) };
            }
            let mut st = 0;
            // SAFETY: st lives for the call.
            unsafe { waitpid(pid, &mut st, 0) };
            let b = (st >> 8) & 0xff;
            let rm = std::fs::remove_file("/tmp/a").is_ok();
            let b2 = put("/tmp/b", m);
            println!("a={a} b={b} rm={rm} b2={b2}");
        }
        "alloc" => {
            let v: Vec<u8> = vec![7; 64 << 20];
            println!("alloc {}", v.iter().map(|&b| b as u64).sum::<u64>());
        }
        // T-1995 (PodUtils/podrunner#315): a signal that stops a call of
        // the payload runs its handler on the payload's alternate stack,
        // as Go's runtime needs, not on a stack of the SIGSYS handler.
        "sig-in-call" => sig_in_call(),
        // T-2001 (OP-03): the strings of argv lie in one block in rising
        // order, just below those of the environment, as libuv reads them.
        "layout" => {
            unsafe extern "C" {
                static environ: *const *const u8;
            }
            let want: Vec<u8> = std::env::args_os()
                .flat_map(|a| {
                    let mut v = std::os::unix::ffi::OsStringExt::into_vec(a);
                    v.push(0);
                    v
                })
                .collect();
            // SAFETY: environ[0] is the first string of the environment; the
            // `want.len()` bytes below it are on the stack of this process
            // where the layout holds, and the test passes an environment.
            let got = unsafe {
                let e0 = *environ;
                std::slice::from_raw_parts(e0.sub(want.len()), want.len())
            };
            println!(
                "layout {}",
                if got == want.as_slice() { "ok" } else { "bad" }
            );
        }
        _ => {
            eprintln!("podvm-dispatch-probe: no verb {verb:?}");
            std::process::exit(2);
        }
    }
}

#[cfg(target_arch = "x86_64")]
static ALT: [std::sync::atomic::AtomicUsize; 2] =
    [const { std::sync::atomic::AtomicUsize::new(0) }; 2];
#[cfg(target_arch = "x86_64")]
static ON: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(target_arch = "x86_64")]
static OFF: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[cfg(target_arch = "x86_64")]
extern "C" fn on_urg(_: i32, _: *mut u8, _: *mut u8) {
    let x = 0u8;
    let sp = &x as *const u8 as usize;
    let (lo, n) = (ALT[0].load(Ordering::SeqCst), ALT[1].load(Ordering::SeqCst));
    let c = if sp >= lo && sp < lo + n { &ON } else { &OFF };
    c.fetch_add(1, Ordering::SeqCst);
}

// The restorer of the kernel's x86_64 frame: Syscall User Dispatch, and so
// this verb, is of x86_64 (nightly 44458458095 built the probe for aarch64).
#[cfg(target_arch = "x86_64")]
core::arch::global_asm!(
    ".globl probe_restorer",
    "probe_restorer:",
    "mov rax, 15",
    "syscall"
);
#[cfg(target_arch = "x86_64")]
unsafe extern "C" {
    fn probe_restorer();
}

#[cfg(not(target_arch = "x86_64"))]
fn sig_in_call() {
    println!("ERR the verb is of x86_64");
}

/// SIGURG by `tgkill`, as Go sends it for preemption, while this thread
/// makes calls: each handler counts whether it runs on the alternate stack.
#[cfg(target_arch = "x86_64")]
fn sig_in_call() {
    let size = 64 * 1024;
    let mem: &'static mut [u8] = Box::leak(vec![0u8; size].into_boxed_slice());
    ALT[0].store(mem.as_ptr() as usize, Ordering::SeqCst);
    ALT[1].store(size, Ordering::SeqCst);
    let st = [mem.as_mut_ptr() as i64, 0, size as i64];
    // `struct kernel_sigaction`: SA_SIGINFO | SA_ONSTACK | SA_RESTART | SA_RESTORER.
    let act: [u64; 4] = [
        on_urg as *const () as usize as u64,
        0x4 | 0x0800_0000 | 0x1000_0000 | 0x0400_0000,
        probe_restorer as *const () as usize as u64,
        0,
    ];
    // SAFETY: a leaked stack, and an action whose handler and restorer live.
    unsafe {
        if sigaltstack(&st, std::ptr::null_mut()) != 0
            || syscall(13, 23i64, act.as_ptr(), 0i64, 8i64) != 0
        {
            println!("ERR setup");
            return;
        }
    }
    // SAFETY: plain calls.
    let (pid, tid) = unsafe { (syscall(39), syscall(186)) };
    let done = std::sync::Arc::new(AtomicBool::new(false));
    let d = done.clone();
    let sender = std::thread::spawn(move || {
        for _ in 0..3000 {
            // SAFETY: SIGURG to the first thread of this probe.
            unsafe { syscall(234, pid, tid, 23i64) };
            std::thread::yield_now();
        }
        d.store(true, Ordering::SeqCst);
    });
    let ts = [0i64, 1000];
    while !done.load(Ordering::SeqCst) {
        // SAFETY: getppid, and a sleep of 1 us on a timespec that lives.
        unsafe {
            syscall(110);
            syscall(35, ts.as_ptr(), 0i64);
        }
    }
    let _ = sender.join();
    let on = ON.load(Ordering::SeqCst);
    println!(
        "sig {} off {}",
        if on > 0 { "on-alt" } else { "none" },
        OFF.load(Ordering::SeqCst)
    );
}
