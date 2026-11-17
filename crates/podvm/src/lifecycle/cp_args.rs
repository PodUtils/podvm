//! The usage and the arguments of `podvm cp` (TODO/cli.md T-1323), out
//! of `wait_rm_cp.rs` for the limit of 500 lines of a source file.

use super::*;

/// `podvm cp <src> <dest>`, where one side is `<name>:<path>` and the
/// name is a container or, failing that, an image.
pub(super) const CP_USAGE: &str = "\
usage: podvm cp [-r|--recursive] <container|image>:<path> <dest>
       podvm cp [-r|--recursive] <src> <container>:<path>

  -r, --recursive  copy a directory tree. A file copies with or without
                   it; a directory without it is refused naming the flag.

  The name before the colon is a container first (as before), then an
  image, whose rootfs is extracted if needed. An image is a source only:
  a copy into it would change the shared extraction, and is refused. A directory copies its
  contents into DEST: created where missing, and it must be a directory
  where present. Symlinks are replicated as symlinks, never followed,
  and every rootfs-side path passes the same containment gate as a
  single file; special files are refused by name rather than recreated
  (TODO/cli.md T-1323).

  A detached full Windows machine (run -d windows:2025) copies one file
  either way through its QEMU Guest Agent: NAME:C:\\path. A guest path
  that ends with \\ is a directory (TODO/machine.md T-1868).
";

/// What `cp` was asked for.
pub(super) struct CpArgs {
    pub(super) recursive: bool,
    pub(super) a: String,
    pub(super) b: String,
}

pub(super) fn parse_cp(args: &[String]) -> std::result::Result<CpArgs, i32> {
    let mut recursive = false;
    let mut positionals: Vec<String> = Vec::new();
    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{CP_USAGE}");
                return Err(0);
            }
            "-r" | "--recursive" => recursive = true,
            other if other.starts_with('-') => {
                crate::parity::admit("cp", other, CP_USAGE)?;
                return Err(crate::parity::no_arm("cp", other));
            }
            other => positionals.push(other.to_string()),
        }
    }
    if positionals.len() != 2 {
        eprintln!("podvm cp: takes exactly two paths, one of them <name>:<path>");
        return Err(EXIT_CLI_ERROR);
    }
    let mut it = positionals.into_iter();
    Ok(CpArgs {
        recursive,
        a: it.next().expect("checked"),
        b: it.next().expect("checked"),
    })
}
