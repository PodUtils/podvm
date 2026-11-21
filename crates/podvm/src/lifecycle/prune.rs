//! TODO/supervise.md T-1848: the `container` group, and `container prune`,
//! which removes each exited or dead record and its tree in one verb. A
//! foreground run keeps its record (T-1421), so a host that runs podvm in
//! each job of a CI grows until something removes them.

use super::*;
use std::os::unix::fs::MetadataExt;

pub(super) const PRUNE_USAGE: &str = "\
usage: podvm container prune [-f|--force]

  -f, --force  accepted for parity: podvm never prompts.

  Remove each exited or dead container and its rootfs tree, and print
  the name of each and the bytes the trees held. A created or running
  container stays. `podvm image prune` removes images.
";

const GROUP_USAGE: &str = "\
usage: podvm container ls | ps | rm | prune | start | stop | restart | kill | wait | logs | inspect | exec | create | run | cp

  Each subverb is the top-level verb of the same name (`ls` is `ps`).
";

/// `podvm container SUBVERB ...`: each subverb is the top-level verb.
pub fn container_group(args: &[String]) -> i32 {
    let rest = if args.len() > 1 { &args[1..] } else { &[] };
    match args.first().map(String::as_str) {
        Some("prune") => prune(rest),
        Some("ls" | "list" | "ps") => ps(rest),
        Some("rm" | "remove") => rm(rest),
        Some("start") => start(rest),
        Some("stop") => stop(rest),
        Some("restart") => restart(rest),
        Some("kill") => kill(rest),
        Some("wait") => wait(rest),
        Some("logs") => logs(rest),
        Some("create") => create(rest),
        Some("cp") => cp(rest),
        Some("inspect") => crate::images::inspect("inspect", rest),
        Some("exec") => crate::exec::exec(rest),
        Some("run") => crate::run::run(rest),
        Some("-h" | "--help") | None => {
            print!("{GROUP_USAGE}");
            0
        }
        Some(other) => {
            eprintln!("podvm container: {other:?} is not a subverb of the group");
            eprint!("{GROUP_USAGE}");
            EXIT_CLI_ERROR
        }
    }
}

/// `podvm container prune`.
pub fn prune(args: &[String]) -> i32 {
    const VERB: &str = "container prune";
    let expanded = match crate::parity::expand(VERB, args) {
        Ok(a) => a,
        Err(member) => return crate::parity::refuse_member(VERB, &member, PRUNE_USAGE),
    };
    if let Some(c) = crate::parity::admit_all(VERB, &expanded, PRUNE_USAGE) {
        return c;
    }
    for a in &expanded {
        match a.as_str() {
            "-h" | "--help" => {
                print!("{PRUNE_USAGE}");
                return 0;
            }
            "-f" | "--force" => {}
            other if other.starts_with('-') => return crate::parity::no_arm(VERB, other),
            other => {
                eprintln!("podvm {VERB}: takes no argument, and {other:?} is one");
                eprint!("{PRUNE_USAGE}");
                return EXIT_CLI_ERROR;
            }
        }
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    // TODO/cli.md T-2063 (OQ-12): the memo of a run that a SIGKILL ended
    // stays in `staging/`. A memo that no lock holds belongs to no run.
    let _ = s.sweep_staging_aged(std::time::Duration::ZERO);
    let all = match podvm_supervise::list(&s, true) {
        Ok(l) => l,
        Err(e) => return fail(VERB, e),
    };
    use podvm_supervise::table::State;
    let mut freed = 0u64;
    let mut code = 0;
    let mut first = true;
    for c in all
        .iter()
        .filter(|c| matches!(c.state, State::Exited | State::Dead))
    {
        let bytes = tree_bytes(std::path::Path::new(&c.rootfs));
        match podvm_supervise::remove(&s, &c.id, false) {
            Ok(_) => {
                if first {
                    println!("Deleted Containers:");
                    first = false;
                }
                println!("{}", c.id);
                freed += bytes;
            }
            Err(e) => {
                eprintln!("podvm {VERB}: {}: {e}", c.name);
                code = EXIT_RUNTIME_ERROR;
            }
        }
    }
    println!(
        "\nTotal reclaimed space: {}",
        podvm_image::space::mib(freed)
    );
    code
}

/// The bytes on disk of the files under `root`, each inode once, with no
/// link followed.
pub(super) fn tree_bytes(root: &std::path::Path) -> u64 {
    let mut seen = std::collections::HashSet::new();
    let mut total = 0u64;
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        let Ok(m) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        if seen.insert((m.dev(), m.ino())) {
            total += m.blocks() * 512;
        }
        if m.is_dir()
            && let Ok(rd) = std::fs::read_dir(&p)
        {
            stack.extend(rd.flatten().map(|e| e.path()));
        }
    }
    total
}
