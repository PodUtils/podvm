use super::*;

/// (flags, the mounts of the namespace rung, the directories that stay)
type Held<'a> = (&'a [&'a str], &'a [(&'a str, u64)], &'a [&'a str]);
/// (rung, userland, forced, init_image, the words of the refusal)
type Entry<'a> = (&'a str, bool, Option<&'a str>, bool, Option<&'a str>);

fn parsed(args: &[&str]) -> Result<Opts, i32> {
    let mut a: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    a.extend(["alpine".to_string(), "true".to_string()]);
    super::super::parse("run", &a)
}

/// TODO/machine.md T-1968: one rule for each spelling of `--tmpfs`: a
/// `size=` over 0 is a tmpfs with that limit, which leaves the directories
/// of T-1655; `size=0` and no `size=` stay a directory; a bad size, a
/// comma, `ro` and a volume at, under or over the path refuse by name.
#[test]
fn each_tmpfs_with_a_size_is_a_mount_with_its_limit() {
    let held: &[Held] = &[
        (&["--tmpfs", "/w:size=64m"], &[("/w", 64 << 20)], &[]),
        (&["--tmpfs=/w/:size=4096,mode=1777"], &[("/w", 4096)], &[]),
        (&["--tmpfs", "/tmp:size=1g"], &[("/tmp", 1 << 30)], &[]),
        (&["--tmpfs", "/w:size=0"], &[], &["/w"]),
        (&["--tmpfs", "/w"], &[], &["/w"]),
        (
            &["--tmpfs", "/w:size=2m", "--tmpfs", "/x"],
            &[("/w", 2 << 20)],
            &["/x"],
        ),
        (
            &["--tmpfs", "/w:size=2m", "-v", "/tmp:/data"],
            &[("/w", 2 << 20)],
            &[],
        ),
    ];
    for (flags, mounts, dirs) in held {
        let mut o = parsed(flags).unwrap_or_else(|c| panic!("{flags:?}: {c}"));
        let got = take("run", false, &mut o).unwrap_or_else(|c| panic!("{flags:?}: {c}"));
        let got: Vec<(&str, u64)> = got
            .iter()
            .map(|m| (m.guest.as_str(), m.bytes.get()))
            .collect();
        assert_eq!(&got, mounts, "{flags:?}");
        assert_eq!(&o.tmpfs, dirs, "{flags:?}");
    }
    let refused: &[&[&str]] = &[
        &["--tmpfs", "/w:size=lots"],
        &["--tmpfs", "/w:size=64m,ro"],
        &["--tmpfs", "/w:size=64m", "-v", "/tmp:/w"],
        &["--tmpfs", "/w:size=64m", "-v", "/tmp:/w/sub"],
        &["--tmpfs", "/w/sub:size=64m", "-v", "/tmp:/w"],
    ];
    for flags in refused {
        let code = parsed(flags).and_then(|mut o| take("run", false, &mut o).map(|_| 0));
        assert_eq!(code, Err(125), "{flags:?}");
    }
    // The syscall tier: /tmp stays the cap of its handler; each other path
    // has no mount namespace there.
    let mut o = parsed(&["--tmpfs", "/tmp:size=8m"]).unwrap();
    assert!(take("run", true, &mut o).unwrap().is_empty());
    assert_eq!(
        (o.tmp_size, o.tmpfs.clone()),
        (Some(8 << 20), vec!["/tmp".to_string()])
    );
    let mut o = parsed(&["--tmpfs", "/w:size=8m"]).unwrap();
    assert_eq!(take("run", true, &mut o), Err(125));
}

/// TODO/machine.md T-1968: only the namespace rung of pod-enter holds a
/// tmpfs with a limit; each other entry gives one line that names the
/// mount and the cause, and never a directory with no limit.
#[test]
fn each_entry_but_the_namespace_rung_refuses_a_tmpfs_by_name() {
    let m = vec![TmpfsMount {
        guest: "/w".into(),
        bytes: NonZeroU64::new(64 << 20).unwrap(),
    }];
    let rows: &[Entry] = &[
        ("namespace", false, None, false, None),
        (
            "chroot",
            false,
            None,
            false,
            Some("this host gives the chroot rung"),
        ),
        (
            "userland",
            true,
            None,
            false,
            Some("this host gives the userland rung"),
        ),
        ("namespace", true, None, false, Some("no mount namespace")),
        (
            "namespace",
            false,
            Some("tmpfs"),
            false,
            Some("PODVM_MODE forces the tmpfs rung"),
        ),
        ("namespace", false, None, true, Some("--podvm-init=image")),
    ];
    for (rung, userland, forced, init, want) in rows {
        let got = wrong_entry(&m, rung, *userland, *forced, *init);
        match want {
            None => assert_eq!(got, None, "{rung}"),
            Some(w) => {
                let line = got.unwrap_or_else(|| panic!("{rung}: no refusal"));
                assert!(
                    line.contains(w) && line.contains("/w:size=67108864"),
                    "{line}"
                );
                assert!(
                    line.contains("no limit in its place") && !line.contains('\n'),
                    "{line}"
                );
            }
        }
    }
    assert_eq!(wrong_entry(&[], "chroot", true, Some("x"), true), None);
    // The record and the words of `inspect` and `--report`.
    let (mut env, mut banner) = (vec!["A=1".to_string()], String::new());
    push(&mut env, &m, &mut banner);
    assert_eq!(words(&env), vec!["/w:size=67108864 (memory)".to_string()]);
    assert!(
        banner.contains("in memory") && banner.contains("ENOSPC"),
        "{banner}"
    );
}
