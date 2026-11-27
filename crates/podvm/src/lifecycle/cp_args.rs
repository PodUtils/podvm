//! The usage and the arguments of `podvm cp` (TODO/cli.md T-1323), out
//! of `wait_rm_cp.rs` for the limit of 500 lines of a source file.

use super::*;

/// `podvm cp <src> <dest>`, where one side is `<name>:<path>` and the
/// name is a container or, failing that, an image.
pub(super) const CP_USAGE: &str = "\
usage: podvm cp [-r|--recursive] <container|image>:<path> <dest>
       podvm cp [-r|--recursive] <src> <container>:<path>
       podvm cp <container|image>:<path> -
       podvm cp - <container>:<path>

  -r, --recursive  copy a directory tree. A file copies with or without
                   it; a directory without it is refused naming the flag.

  - as the destination writes a tar of the path to stdout; - as the
  source unpacks a tar from stdin into the directory <path> of a container
  (docker's two forms; a file of the name - is ./-). The entries of the
  stream going out are named by the last name of the path (or ./ where it
  ends with /.) and are owned by 0:0. A tar going in keeps the mode and
  the modification time of each entry; it takes files, directories and
  links, refuses a name above the destination (..), a link on the way and
  a special file by name, and needs no -r (TODO/cli.md T-2108). The
  refusals exit 1, as docker's.

  cp keeps the modification time of each file, directory and link, in
  both directions (docker sets the access time to it as well).

  The name before the colon is a container first (as before), then an
  image, whose rootfs is extracted if needed. An image is a source only:
  a copy into it would change the shared extraction, and is refused.

  Where the copy lands, as docker cp (TODO/cli.md T-2079):
    SRC a directory, DEST missing      DEST is made with the contents of SRC
    SRC a directory, DEST a directory  DEST/basename(SRC)
    SRC ends with /.                   the contents of SRC go into DEST
    SRC a file, DEST a directory       DEST/basename(SRC)
    SRC a file, DEST a file or missing written as DEST; DEST/ must exist
  A missing DEST needs its parent directory: podvm makes no parent. A
  directory onto a file is refused. The container path resolves in the
  root of the container, as docker resolves it: a link on the way, also
  one with an absolute target, and a `..` stay in that root; a `..` above
  the root is refused (TODO/cli.md T-2087). The last name of a source
  that is a link copies as a link; a destination that is a link to a
  directory takes that directory. Symlinks in a tree are replicated as
  symlinks, never followed, and every rootfs-side path passes the same
  containment gate as a single file; special files are refused by name
  rather than recreated (TODO/cli.md T-1323).

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
            // TODO/cli.md T-2108: `-` is the tar stream of stdin or stdout.
            "-" => positionals.push("-".to_string()),
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
