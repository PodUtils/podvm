//! TODO/probe.md T-1911: the user namespace of a run with the subordinate
//! range of the caller (`/etc/subuid`, `/etc/subgid`), so that `-u UID`
//! drops for real and the ids of an image map, as under rootless podman.
//!
//! The setuid helpers `newuidmap` and `newgidmap` write the maps. They run
//! OUTSIDE the namespace: inside it, host root is not mapped, so the kernel
//! does not give the setuid helper root. podvm forks a helper before it
//! calls `unshare`; the helper waits on a pipe, then runs both tools on the
//! pid of podvm.
//!
//! ⛔ Opt-in (`PODVM_USERNS_RANGE=1`) until each verb that touches a tree
//! keeps the store rule (the Decision of T-1911).

use std::path::Path;
use std::process::Command;

/// The variable that asks for the range.
pub const VAR: &str = "PODVM_USERNS_RANGE";

/// One range: its first host id and its count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub start: u64,
    pub count: u64,
}

/// The range of the user `uid` (named `name`) in the text of a file of the
/// form of `/etc/subuid`: the first line whose owner is the name or the
/// number.
pub fn range_in(text: &str, uid: u32, name: Option<&str>) -> Option<Range> {
    let id = uid.to_string();
    text.lines().find_map(|l| {
        let mut f = l.trim().split(':');
        let (who, start, count) = (f.next()?, f.next()?, f.next()?);
        if who != id && Some(who) != name {
            return None;
        }
        let r = Range {
            start: start.parse().ok()?,
            count: count.parse().ok()?,
        };
        (r.count > 0).then_some(r)
    })
}

/// The name of `uid` in `/etc/passwd`.
fn user_name(uid: u32) -> Option<String> {
    let text = std::fs::read_to_string("/etc/passwd").ok()?;
    text.lines().find_map(|l| {
        let f: Vec<&str> = l.split(':').collect();
        (f.len() > 2 && f[2] == uid.to_string()).then(|| f[0].to_string())
    })
}

/// The tool on PATH, as an absolute path.
fn tool(name: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// The plan of the maps: the two tools and the two ranges. None where the
/// caller does not ask, or the host lacks a part.
pub struct Plan {
    uidmap: std::path::PathBuf,
    gidmap: std::path::PathBuf,
    uids: Range,
    gids: Range,
}

/// The plan for `uid` and `gid` where the caller asks for the range.
pub fn plan(want: Option<&str>) -> Option<Plan> {
    if want != Some("1") {
        return None;
    }
    host_plan()
}

/// True where the host has the tools and the ranges of the caller.
pub fn available() -> bool {
    host_plan().is_some()
}

/// The plan where the host has the tools and the ranges of the caller.
fn host_plan() -> Option<Plan> {
    // SAFETY: getuid cannot fail.
    let uid = unsafe { super::getuid() };
    let name = user_name(uid);
    let read = |p: &str| std::fs::read_to_string(p).ok();
    Some(Plan {
        uidmap: tool("newuidmap")?,
        gidmap: tool("newgidmap")?,
        uids: range_in(&read("/etc/subuid")?, uid, name.as_deref())?,
        gids: range_in(&read("/etc/subgid")?, uid, name.as_deref())?,
    })
}

impl Plan {
    /// The highest id that the maps hold (0 is the caller).
    pub fn top(&self) -> u64 {
        self.uids.count.min(self.gids.count)
    }
}

/// The arguments of `newuidmap PID ...`: the caller on 0, the range from 1.
pub fn map_args(pid: i32, own: u32, r: Range) -> Vec<String> {
    let max = r.count.min(u64::from(u32::MAX) - 1);
    vec![
        pid.to_string(),
        "0".into(),
        own.to_string(),
        "1".into(),
        "1".into(),
        r.start.to_string(),
        max.to_string(),
    ]
}

/// Run both tools on `pid`; true where both wrote their map.
pub fn write_maps(p: &Plan, pid: i32, uid: u32, gid: u32) -> bool {
    let run = |tool: &Path, args: Vec<String>| {
        Command::new(tool)
            .args(args)
            .stdin(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    };
    run(&p.uidmap, map_args(pid, uid, p.uids)) && run(&p.gidmap, map_args(pid, gid, p.gids))
}

/// The internal verb: remove the tree DIR in a user namespace with the
/// range (the hook `RANGE_REMOVER` of podvm-extract).
pub const RM_ARG: &str = "--podvm-range-rm";

/// The hook: run podvm again with [`RM_ARG`]; true where the tree is gone.
pub fn remover(dir: &Path) -> bool {
    if host_plan().is_none() {
        return false;
    }
    let Ok(me) = std::env::current_exe() else {
        return false;
    };
    let st = Command::new(me)
        .arg(RM_ARG)
        .arg(dir)
        .stdin(std::process::Stdio::null())
        .status();
    st.is_ok_and(|s| s.success()) && !dir.exists()
}

/// [`RM_ARG`], first in `main` with one thread: a user namespace with the
/// range, where the caller (0) holds each subordinate id, then the
/// removal. Only a tree of a store (`.../trees/ID` or a trash entry).
pub fn serve_rm(argv: &[String]) {
    if argv.get(1).map(String::as_str) == Some(AGAIN_ARG) {
        serve_again(argv);
    }
    if argv.get(1).map(String::as_str) != Some(RM_ARG) {
        return;
    }
    let dir = Path::new(argv.get(2).map(String::as_str).unwrap_or(""));
    let in_trees = dir
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|n| n == "trees");
    let ok = in_trees
        && host_plan().is_some_and(|p| {
            // SAFETY: getuid and getgid cannot fail.
            let (uid, gid) = unsafe { (super::getuid(), super::getgid()) };
            super::with_range(&p, uid, gid, super::CLONE_NEWUSER)
        })
        && podvm_extract::live::remove_plain(dir).is_ok();
    std::process::exit(if ok { 0 } else { 1 });
}

/// The internal verb: `podvm VERB ARGS` (`cp` or `exec`) in a user
/// namespace with the range.
pub const AGAIN_ARG: &str = "--podvm-range-again";

/// `podvm cp` or `podvm exec` of a container that is not running, where
/// the range is asked for: the payload of a run could leave files of a
/// subordinate id with no right of the caller, so the verb runs again in a
/// user namespace with the range, where the caller (0) holds each of those
/// ids. A file that it writes on the host belongs to the caller. A running
/// container keeps its own path (`cp` and `exec` join it, T-1775, T-1910).
/// None: go on here.
pub fn again_in_range(verb: &str, running: bool, args: &[String]) -> Option<i32> {
    if running || super::owners::TOP.load(std::sync::atomic::Ordering::Relaxed) > 0 {
        return None;
    }
    plan(std::env::var(VAR).ok().as_deref())?;
    let me = std::env::current_exe().ok()?;
    let st = Command::new(me)
        .arg(AGAIN_ARG)
        .arg(verb)
        .args(args)
        .status()
        .ok()?;
    Some(st.code().unwrap_or(125))
}

/// The `cp` form of [`again_in_range`]: the container of either side.
pub fn cp_in_range(s: &podvm_image::Store, a: &str, b: &str, args: &[String]) -> Option<i32> {
    let name = [a, b]
        .into_iter()
        .find_map(|x| crate::lifecycle::split(x).map(|(n, _)| n))?;
    let c = podvm_supervise::get(s, &name).ok()?;
    again_in_range(
        "cp",
        c.state == podvm_supervise::table::State::Running,
        args,
    )
}

/// Open a user namespace with the subordinate range for this process, and
/// no other namespace: the caller is 0 in it and holds each id of the
/// range. True where the maps hold the range.
///
/// ⛔ With one thread only (`unshare(CLONE_NEWUSER)`).
pub fn enter_alone() -> bool {
    // SAFETY: getuid and getgid cannot fail.
    let (uid, gid) = unsafe { (super::getuid(), super::getgid()) };
    host_plan().is_some_and(|p| super::with_range(&p, uid, gid, super::CLONE_NEWUSER))
        && super::owners::TOP.load(std::sync::atomic::Ordering::Relaxed) > 0
}

/// [`AGAIN_ARG`], first in `main` with one thread: the namespace with the
/// range, then the verb. With no range, podvm refuses (125).
fn serve_again(argv: &[String]) {
    let entered = enter_alone();
    let verb = argv.get(2).map(String::as_str).unwrap_or("");
    if !entered {
        eprintln!("podvm {verb}: the user namespace with the subordinate range did not open");
        std::process::exit(125);
    }
    let rest = &argv[3..];
    std::process::exit(match verb {
        "cp" => crate::lifecycle::cp(rest),
        "exec" => crate::exec::exec(rest),
        _ => 125,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_range_is_found_by_name_or_by_number() {
        let t = "root:1000000:1000000000\nubuntu:100000:65536\n2000:300000:10\n";
        assert_eq!(
            range_in(t, 1000, Some("ubuntu")),
            Some(Range {
                start: 100000,
                count: 65536
            })
        );
        assert_eq!(
            range_in(t, 2000, None),
            Some(Range {
                start: 300000,
                count: 10
            })
        );
        assert_eq!(range_in(t, 3000, Some("nobody")), None);
        assert_eq!(range_in("x:1:0\n", 1, Some("x")), None);
    }

    #[test]
    fn the_caller_is_0_and_the_range_starts_at_1() {
        let a = map_args(
            42,
            1000,
            Range {
                start: 100000,
                count: 65536,
            },
        );
        assert_eq!(a, ["42", "0", "1000", "1", "1", "100000", "65536"]);
    }
}
