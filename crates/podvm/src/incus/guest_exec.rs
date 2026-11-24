//! `podvm guest-exec --cwd DIR -- PROG ARGS...`: the start of the command of
//! a foreground run of the incus tier, in the instance (TODO/machine.md
//! T-2080, OR-B39-01, OR-B39-09 and OR-B39-16 of the eighth outside pass).
//!
//! incusd 6.0 gives the same answer for a program that is not there and for
//! a payload that ends with 127 (and for 126): an operation that failed with
//! "Command not found" ("Command not executable") and `return` 127 (126),
//! with no other word (measured on 2026-11-24). podvm in the image starts
//! the command, so the cause of a start that fails is a line of its own:
//!
//! * the program is not on `PATH`: docker's words, exit 127;
//! * the file is there and its interpreter or loader is not: exit 127;
//! * no permission: exit 126;
//! * an ELF of another architecture (`--platform`): the two architectures,
//!   exit 126.
//!
//! The working directory of `-w` is made where it is missing, as docker
//! makes it; incusd ran the command in `/` with no word.
//!
//! Then it replaces itself with the command (`execve`): the pid, the
//! streams and the signals are those of the command.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// The verb.
pub const VERB: &str = "guest-exec";
/// podvm in the image of each instance of podvm (T-1722).
pub const PODVM: &str = "/.podvm/podvm";
/// The `PATH` where the environment has none (docker's default).
pub const DEFAULT_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

/// The argv of a foreground run: `argv` in `cwd`, through podvm in the image.
pub fn wrap(argv: &[String], cwd: &str) -> Vec<String> {
    let mut v = vec![
        PODVM.to_string(),
        VERB.to_string(),
        "--cwd".to_string(),
        cwd.to_string(),
        "--".to_string(),
    ];
    v.extend(argv.iter().cloned());
    v
}

/// The working directory and the command of the arguments of [`VERB`].
pub fn parse(args: &[String]) -> Result<(String, &[String]), String> {
    match args {
        [flag, cwd, sep, rest @ ..] if flag == "--cwd" && sep == "--" => Ok((cwd.clone(), rest)),
        [sep, rest @ ..] if sep == "--" => Ok((String::new(), rest)),
        _ => Err("usage: podvm guest-exec --cwd DIR -- PROG [ARG...]".to_string()),
    }
}

/// The file that `prog` names: a path as it is, else the first file of
/// that name in a directory of `path` (an empty entry is the working
/// directory) that has an execute bit, else the first such file with none
/// (its start then fails with EACCES, exit 126). `None`: no file.
pub fn resolve(prog: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    if prog.is_empty() {
        return None;
    }
    if prog.contains('/') {
        let p = PathBuf::from(prog);
        return std::fs::metadata(&p).is_ok().then_some(p);
    }
    let path = path.unwrap_or(OsStr::new(DEFAULT_PATH));
    let mut no_bit = None;
    for dir in std::env::split_paths(path) {
        let dir = if dir.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            dir
        };
        let cand = dir.join(prog);
        match std::fs::metadata(&cand) {
            Ok(m) if m.is_file() && m.permissions().mode() & 0o111 != 0 => return Some(cand),
            Ok(m) if m.is_file() && no_bit.is_none() => no_bit = Some(cand),
            _ => {}
        }
    }
    no_bit
}

/// The architecture of an ELF header `head` (its first 20 bytes or more),
/// in the words of `std::env::consts::ARCH`; `None` for another file.
pub fn elf_arch(head: &[u8]) -> Option<&'static str> {
    if head.len() < 20 || !head.starts_with(b"\x7fELF") {
        return None;
    }
    let m = match head[5] {
        2 => u16::from_be_bytes([head[18], head[19]]),
        _ => u16::from_le_bytes([head[18], head[19]]),
    };
    Some(match m {
        3 => "x86",
        8 => "mips",
        20 => "powerpc",
        21 => "powerpc64",
        22 => "s390x",
        40 => "arm",
        62 => "x86_64",
        183 => "aarch64",
        243 => "riscv64",
        258 => "loongarch64",
        _ => "an architecture that podvm does not name",
    })
}

/// The exit code and the line of a start of `prog` (the file `file`) that
/// failed with `e`. `head` is the start of the file, for an ELF of another
/// architecture; `host` the architecture of this machine.
pub fn explain(
    prog: &str,
    file: &Path,
    e: &std::io::Error,
    head: &[u8],
    host: &str,
) -> (i32, String) {
    let f = file.display();
    match e.raw_os_error() {
        Some(2) => (
            127,
            format!(
                "{f}: no such file or directory (the file is there: its interpreter or its \
                 loader is not)"
            ),
        ),
        Some(13) => (126, format!("{prog}: permission denied")),
        Some(8) => match elf_arch(head) {
            Some(a) if a != host => (
                126,
                format!(
                    "{f}: exec format error: the file is an ELF for {a}, and this machine runs \
                     {host}; an image of another platform needs an emulator of {a} that the \
                     host registers in binfmt_misc"
                ),
            ),
            _ => (126, format!("{f}: exec format error")),
        },
        _ => (126, format!("{f}: {e}")),
    }
}

/// The line and the exit code of a program that is not there.
pub fn not_found(prog: &str) -> (i32, String) {
    if prog.contains('/') {
        (127, format!("{prog}: no such file or directory"))
    } else {
        (
            127,
            format!("\"{prog}\": executable file not found in $PATH"),
        )
    }
}

/// [`VERB`]: start the command, or end with the line of its cause.
pub fn main(args: &[String]) -> i32 {
    use std::os::unix::process::CommandExt;
    let say = |(code, line): (i32, String)| {
        eprintln!("podvm: exec: {line}");
        code
    };
    let (cwd, argv) = match parse(args) {
        Ok(p) => p,
        Err(e) => return say((126, e)),
    };
    if !cwd.is_empty() {
        if let Err(e) = std::fs::create_dir_all(&cwd) {
            return say((126, format!("the working directory {cwd}: {e}")));
        }
        if let Err(e) = std::env::set_current_dir(&cwd) {
            return say((126, format!("the working directory {cwd}: {e}")));
        }
    }
    let Some((prog, rest)) = argv.split_first() else {
        return say((126, "the command is empty".to_string()));
    };
    let path = std::env::var_os("PATH");
    let Some(file) = resolve(prog, path.as_deref()) else {
        return say(not_found(prog));
    };
    let e = std::process::Command::new(&file)
        .arg0(prog)
        .args(rest)
        .exec();
    let mut head = [0u8; 20];
    let n = std::fs::File::open(&file)
        .and_then(|mut f| std::io::Read::read(&mut f, &mut head))
        .unwrap_or(0);
    say(explain(prog, &file, &e, &head[..n], std::env::consts::ARCH))
}
