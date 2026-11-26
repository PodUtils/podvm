//! TODO/cli.md T-2087 (OR-B40-16, OR-B41-04): the container side of `cp`
//! resolves in the root of the container, in both directions.

use super::*;
use crate::lifecycle::cp_root::in_root;

/// What a row of the table expects.
enum Want {
    /// The copy lands at this path under the base, with these bytes.
    Lands(&'static str, &'static str),
    /// The copy is refused with exit 1; the line holds these words, and
    /// nothing is at this path under the base.
    Refused(&'static str, &'static str),
}

/// One rule for each row: a link of the container (absolute or relative)
/// and a `..` resolve in the root of the container, as docker resolves
/// them; a walk that goes above the root, a loop and a link to a path that
/// the container does not have are refused by name, and the host path of
/// the same name is never read or written.
#[test]
fn a_container_path_resolves_in_the_root_of_the_container() {
    let base = std::env::temp_dir().join(format!("podvm-cp-root-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let root = base.join("rootfs");
    for d in ["run", "var", "root", "etc", "tmp", "loop", "out"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    std::fs::create_dir_all(base.join("outside")).unwrap();
    std::fs::create_dir_all(base.join("got")).unwrap();
    std::fs::write(root.join("run/in"), "container run").unwrap();
    std::fs::write(root.join("etc/hostname"), "ctr").unwrap();
    std::fs::write(base.join("outside/secret"), "host").unwrap();
    for (n, text) in [("a", "aa"), ("b", "bb"), ("c", "cc")] {
        std::fs::write(base.join(n), text).unwrap();
    }
    let link =
        |to: &std::path::Path, at: &str| std::os::unix::fs::symlink(to, root.join(at)).unwrap();
    link("/run".as_ref(), "var/run");
    link("/".as_ref(), "root/hostroot");
    link("../../..".as_ref(), "tmp/up");
    link("/loop/b".as_ref(), "loop/a");
    link("/loop/a".as_ref(), "loop/b");
    // An absolute host path that exists on the host, not in the container.
    link(&base.join("outside"), "tmp/host");

    // (container path, out of the container, host side, want)
    let rows: [(&str, bool, &str, Want); 11] = [
        (
            "/var/run/in",
            true,
            "got/1",
            Want::Lands("got/1", "container run"),
        ),
        (
            "/root/hostroot/etc/hostname",
            true,
            "got/2",
            Want::Lands("got/2", "ctr"),
        ),
        (
            "/etc/../etc/hostname",
            true,
            "got/3",
            Want::Lands("got/3", "ctr"),
        ),
        (
            "/var/run/../etc/hostname",
            true,
            "got/4",
            Want::Lands("got/4", "ctr"),
        ),
        ("/var/run/a", false, "a", Want::Lands("rootfs/run/a", "aa")),
        ("/var/run", false, "b", Want::Lands("rootfs/run/b", "bb")),
        (
            "/etc/../../../../etc/hostname",
            true,
            "got/5",
            Want::Refused("above the root", "got/5"),
        ),
        (
            "/tmp/up/etc/hostname",
            true,
            "got/6",
            Want::Refused("above the root", "got/6"),
        ),
        (
            "/loop/a/x",
            true,
            "got/7",
            Want::Refused("40 links", "got/7"),
        ),
        (
            "/tmp/host/secret",
            true,
            "got/8",
            Want::Refused("is a link to", "got/8"),
        ),
        (
            "/tmp/host/x",
            false,
            "c",
            Want::Refused("is a link to", "outside/x"),
        ),
    ];
    for (inside, out_of, host, want) in rows {
        let code = copy_one(&root, inside, &base.join(host), out_of, false);
        match want {
            Want::Lands(at, text) => {
                assert_eq!(code, 0, "{inside}");
                assert_eq!(
                    std::fs::read_to_string(base.join(at)).unwrap(),
                    text,
                    "{inside}"
                );
            }
            Want::Refused(words, absent) => {
                assert_eq!(code, EXIT_CLI_ERROR, "{inside}");
                let e = in_root(&root, inside, !out_of).unwrap_err();
                assert!(e.contains(words), "{inside}: {e}");
                assert!(!base.join(absent).exists(), "{inside} made {absent}");
            }
        }
    }
    assert_eq!(
        std::fs::read_to_string(base.join("outside/secret")).unwrap(),
        "host"
    );
    let _ = std::fs::remove_dir_all(&base);
}
