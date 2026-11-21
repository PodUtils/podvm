//! OT-04 of the fourth outside pass (TODO/enter.md T-1986): the home of the
//! user of the run is writable on the userland rung.
//!
//! On a rung with no user namespace the payload runs as the host user, the
//! owner of the tree, not as uid 0, so a directory of mode 0550 denies it
//! the write that uid 0 of the image has. Fedora 42's `/root` is such a
//! directory, and `dnf` writes `$HOME/.local/state`. The home gets the
//! owner's write bit, in the container's own copy of the tree (T-1709);
//! each other mode stays the image's.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// The guest path of the home of the run: `HOME` of `env`, else `/root`,
/// as `pod-enter` sets it.
pub(super) fn home_of(env: &[String]) -> String {
    env.iter()
        .rev()
        .find_map(|e| e.strip_prefix("HOME="))
        .filter(|h| h.starts_with('/'))
        .unwrap_or("/root")
        .to_string()
}

/// TODO/cli.md T-2061: docker gives `-t` the word `xterm` for `TERM` where
/// neither the image nor `-e` sets one.
pub(super) fn tty_term(tty: bool, env: &mut Vec<String>) {
    if tty && !env.iter().any(|e| e == "TERM" || e.starts_with("TERM=")) {
        env.push("TERM=xterm".to_string());
    }
}

/// On the userland rung, give the home of the run the owner's write bit
/// in the tree `rootfs`. A home that is no directory, a link, or a path
/// with `..` stays as it is: nothing out of the tree changes.
pub(super) fn writable(entered: podvm_probe::select::Rung, rootfs: &str, env: &[String]) {
    if entered == podvm_probe::select::Rung::Userland {
        give(rootfs, env);
    }
}

fn give(rootfs: &str, env: &[String]) {
    let home = home_of(env);
    if home.split('/').any(|c| c == "..") {
        return;
    }
    let p = Path::new(rootfs).join(home.trim_start_matches('/'));
    // A link at any name of the path could leave the tree.
    let mut at = Path::new(rootfs).to_path_buf();
    for c in Path::new(home.trim_start_matches('/')).components() {
        at.push(c);
        match std::fs::symlink_metadata(&at) {
            Ok(m) if !m.file_type().is_symlink() => {}
            _ => return,
        }
    }
    let Ok(m) = std::fs::symlink_metadata(&p) else {
        return;
    };
    let mode = m.permissions().mode();
    if m.is_dir() && mode & 0o200 == 0 {
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode | 0o200));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_home_of_the_run_gets_the_owners_write_bit_and_nothing_else() {
        let d = std::env::temp_dir().join(format!("podvm-t1986-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        for sub in ["root", "home/u", "etc"] {
            std::fs::create_dir_all(d.join(sub)).unwrap();
        }
        std::os::unix::fs::symlink("/etc", d.join("home/l")).unwrap();
        let set = |p: &str, m: u32| {
            std::fs::set_permissions(d.join(p), std::fs::Permissions::from_mode(m)).unwrap()
        };
        let mode = |p: &str| std::fs::metadata(d.join(p)).unwrap().permissions().mode() & 0o777;
        set("root", 0o550);
        set("home/u", 0o500);
        set("etc", 0o555);
        let r = d.to_string_lossy().to_string();
        give(&r, &[]);
        assert_eq!(mode("root"), 0o750, "the default home, /root");
        give(&r, &["HOME=/home/u".into()]);
        assert_eq!(mode("home/u"), 0o700);
        // A link, a path with .., and a missing home change nothing.
        give(&r, &["HOME=/home/l".into()]);
        give(&r, &["HOME=/home/u/../../etc".into()]);
        give(&r, &["HOME=/none".into()]);
        assert_eq!(mode("etc"), 0o555);
        assert_eq!(home_of(&["HOME=rel".into()]), "/root");
        for p in ["root", "home/u", "etc"] {
            set(p, 0o755);
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    /// TODO/cli.md T-2061: `-t` sets `TERM=xterm` where nothing sets it.
    #[test]
    fn a_tty_gets_term_xterm_unless_the_caller_set_one() {
        // (tty, the environment before, the environment after)
        let rows: [(bool, &[&str], &[&str]); 5] = [
            (true, &[], &["TERM=xterm"]),
            (true, &["A=1"], &["A=1", "TERM=xterm"]),
            (true, &["TERM=vt100"], &["TERM=vt100"]),
            (true, &["TERM"], &["TERM"]),
            (false, &["A=1"], &["A=1"]),
        ];
        for (tty, before, after) in rows {
            let mut env: Vec<String> = before.iter().map(|s| s.to_string()).collect();
            tty_term(tty, &mut env);
            assert_eq!(env, after, "{tty} {before:?}");
        }
    }
}
