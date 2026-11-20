//! TODO/enter.md T-2002: `exec` into a running container whose PID 1 is the
//! init of its image, on the namespace rung.
//!
//! `exec` starts again in the user namespace of the supervisor of the run
//! ([`crate::userns::rejoin`], T-1910), where the caller holds each right.
//! Here it reads the state that the supervisor wrote, takes the PID
//! namespace of the init for its child, and runs the command in the mount,
//! cgroup, IPC and UTS namespace of the init, as the foreground run does
//! for its own command ([`super::join`]). The init stays.

use std::path::{Path, PathBuf};

use super::boot::{self, Spec};
use super::hold::Held;
use super::{cgroup, join};

/// What `exec` asks for.
pub struct Ask<'a> {
    pub command: &'a [String],
    /// The environment of the command (`KEY=VALUE`): the one of the
    /// container, then each `-e` of `exec`.
    pub env: &'a [String],
    pub workdir: &'a str,
    /// The ids of `--user`, else the ids that the run recorded.
    pub ids: Option<(u32, u32)>,
    /// The tree of the container, for the groups of the user.
    pub rootfs: &'a str,
    pub tty: bool,
}

/// The parent of the process `pid`, from the text of its `/proc/PID/stat`:
/// the second field after the name, which ends at the last `)`.
pub fn parent_of(stat: &str) -> Option<i32> {
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(1)?.parse().ok()
}

/// The parent of the process `pid` of this `/proc`.
fn parent(pid: i32) -> Option<i32> {
    parent_of(&std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
}

/// The init of the state file `state` and its supervisor, where the
/// parent of that supervisor is the process `outer` of the record: a pid
/// of a file can name another process after the end of the run.
fn init_of(state: &Path, outer: i32) -> Result<(i32, i32, bool), String> {
    let (pid, cgroup) = match Held::read(state) {
        Ok(Held::Up { pid, cgroup, .. }) => (pid, cgroup),
        Ok(Held::Failed(why)) => return Err(format!("the boot of the init failed: {why}")),
        Err(e) => return Err(format!("the state of the init: {e}")),
    };
    let supervisor =
        parent(pid).ok_or(format!("the init of the state file (pid {pid}) is gone"))?;
    if parent(supervisor) != Some(outer) {
        return Err(format!(
            "the process {pid} of the state file is not the init of this run (the process \
             {outer} of the record is not the parent of its supervisor)"
        ));
    }
    Ok((pid, supervisor, cgroup))
}

/// Run the command of `ask` in the init of the run whose record names the
/// process `outer` (a pid of this `/proc`): the exit code of the command,
/// or 125 with one line.
pub fn exec(state: &Path, outer: i32, ask: &Ask<'_>) -> i32 {
    let (init, supervisor, own_cgroup) = match init_of(state, outer) {
        Ok(found) => found,
        Err(why) => return boot::refuse(&why),
    };
    // The cgroup of the command of the run, where the host lets this
    // process move: a refusal leaves the command in the cgroup of the
    // caller, and the PID namespace still holds it.
    if let Some(leaf) = leaf_of(supervisor) {
        let _ = cgroup::enter(&leaf);
    }
    if let Err(e) = join::enter_pid_namespace(init) {
        return boot::refuse(&format!("the PID namespace of the init {init}: {e}"));
    }
    let ns = match join::Namespaces::of(init, own_cgroup) {
        Ok(ns) => ns,
        Err(e) => return boot::refuse(&format!("the namespaces of the init {init}: {e}")),
    };
    // The namespaces are open: the same check again, so that the init of
    // the descriptors is the init of this run.
    if let Err(why) = init_of(state, outer) {
        return boot::refuse(&why);
    }
    let ids = ask.ids.filter(|i| *i != (0, 0));
    let spec = Spec {
        cwd: ask.workdir.to_string(),
        ids,
        groups: ids.map_or(Vec::new(), |i| super::image_groups(ask.rootfs, i)),
        tty: ask.tty,
        argv: ask.command.to_vec(),
        ..Default::default()
    };
    join::run_command(&ns, &spec, init, Some(ask.env))
}

/// The cgroup2 directory of the process `pid`: the leaf of the supervisor
/// and of the command of the run.
fn leaf_of(pid: i32) -> Option<PathBuf> {
    let info = std::fs::read_to_string("/proc/self/mountinfo").ok()?;
    let root = cgroup::mount_point(&info)?;
    let membership = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    let own = membership.lines().find_map(|l| l.strip_prefix("0::"))?;
    let clean = own.starts_with('/') && own.split('/').all(|c| c != "." && c != "..");
    clean.then(|| Path::new(&root).join(own.trim_start_matches('/')))
}

#[cfg(test)]
mod tests {
    use super::parent_of;

    /// The name of a process can hold spaces and parentheses: the parent
    /// is read after the last `)`.
    #[test]
    fn the_parent_of_a_process_is_read_after_its_name() {
        for (stat, want) in [
            ("42 (systemd) S 7 42 42 0 -1", Some(7)),
            ("42 (a b) c) R 1234 1 1", Some(1234)),
            ("42 (x) S", None),
            ("42 (x) S seven 1", None),
            ("42 x S 7", None),
            ("", None),
        ] {
            assert_eq!(parent_of(stat), want, "{stat:?}");
        }
    }
}
