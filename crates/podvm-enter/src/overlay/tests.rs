use super::*;

/// T-2101: one rule for each guest path of an overlay volume: not `/`, not
/// at or under a path that the rung mounts after the volumes, and no other
/// volume at, over or under it.
#[test]
fn an_overlay_volume_takes_a_guest_path_of_its_own() {
    let s = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let cases: &[(&str, &[&str], Option<&str>)] = &[
        ("/c", &["/c"], None),
        ("/c", &["/c", "/data"], None),
        ("/c", &["/c", "/cache"], None),
        ("/", &["/"], Some("cannot replace the root")),
        ("/tmp", &["/tmp"], Some("mounts its own /tmp")),
        ("/tmp/c", &["/tmp/c"], Some("mounts its own /tmp")),
        ("/dev/x", &["/dev/x"], Some("mounts its own /dev")),
        ("/proc", &["/proc"], Some("mounts its own /proc")),
        ("/sys/fs", &["/sys/fs"], Some("mounts its own /sys")),
        (
            "/c",
            &["/c", "/c/sub"],
            Some("shares a path with the volume at /c/sub"),
        ),
        (
            "/c/sub",
            &["/c", "/c/sub"],
            Some("shares a path with the volume at /c"),
        ),
        ("/tmpx", &["/tmpx"], None),
    ];
    for (g, all, want) in cases {
        let got = check_guests(&s(&[g]), &s(all));
        match want {
            None => assert_eq!(got, Ok(()), "{g} {all:?}"),
            Some(w) => {
                let e = got.unwrap_err();
                assert!(e.contains(w), "{g} {all:?}: {e}");
            }
        }
    }
}

/// T-2101: the layers are beside the rootfs, in the tree of the container,
/// one directory for each overlay volume.
#[test]
fn the_layers_are_in_the_tree_of_the_container() {
    let r = Path::new("/s/trees/ab12/rootfs");
    assert_eq!(
        layer_dir(r, 0),
        Some(PathBuf::from("/s/trees/ab12/overlay/0"))
    );
    assert_eq!(
        layer_dir(r, 3),
        Some(PathBuf::from("/s/trees/ab12/overlay/3"))
    );
    assert_eq!(layer_dir(Path::new("/"), 0), None);
}
