//! TODO/machine.md T-1994, steps 2 and 3: the init of the image as PID 1
//! of the guest.

use super::*;

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "podvm-t1994-pid1-{name}-{}",
        crate::machine::guest::init::nonce().unwrap()
    ));
    std::fs::create_dir(&d).unwrap();
    d
}

/// One table of images and the init that each one has: systemd through
/// each kind of link (relative, absolute, through a linked directory of
/// a merged /usr), and each init that is not systemd or not there.
#[test]
fn the_init_of_an_image_is_systemd_through_each_link() {
    use std::os::unix::fs::symlink;
    // (the name, the links and files of the tree, systemd)
    type Tree = &'static [(&'static str, Option<&'static str>)];
    let cases: [(&str, Tree, bool); 7] = [
        (
            "arch",
            &[
                ("usr/lib/systemd/systemd", None),
                ("usr/bin/init", Some("../lib/systemd/systemd")),
                ("sbin", Some("usr/bin")),
            ],
            true,
        ),
        (
            "debian",
            &[
                ("lib/systemd/systemd", None),
                ("sbin/init", Some("/lib/systemd/systemd")),
            ],
            true,
        ),
        (
            "absolute-dir",
            &[
                ("usr/lib/systemd/systemd", None),
                ("usr/sbin/init", Some("/usr/lib/systemd/systemd")),
                ("sbin", Some("/usr/sbin")),
            ],
            true,
        ),
        (
            "busybox",
            &[("bin/busybox", None), ("sbin/init", Some("/bin/busybox"))],
            false,
        ),
        ("none", &[("bin/sh", None)], false),
        (
            "dangling",
            &[("sbin/init", Some("/lib/systemd/systemd"))],
            false,
        ),
        (
            "loop",
            &[
                ("sbin/init", Some("/sbin/init2")),
                ("sbin/init2", Some("init")),
            ],
            false,
        ),
    ];
    for (name, tree, want) in cases {
        let d = scratch(name);
        for (path, link) in tree {
            let p = d.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            match link {
                Some(t) => symlink(t, &p).unwrap(),
                None => std::fs::write(&p, b"ELF").unwrap(),
            }
        }
        assert_eq!(systemd_init(&d), want, "{name}");
        let _ = std::fs::remove_dir_all(&d);
    }
    // A target above the root stays in the root.
    let d = scratch("escape");
    std::fs::create_dir_all(d.join("lib/systemd")).unwrap();
    std::fs::write(d.join("lib/systemd/systemd"), b"ELF").unwrap();
    std::fs::create_dir_all(d.join("sbin")).unwrap();
    symlink("../../../../lib/systemd/systemd", d.join("sbin/init")).unwrap();
    assert_eq!(
        resolve(&d, "/sbin/init"),
        Some(d.join("lib/systemd/systemd"))
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// The files that `guest-root` writes before the exec of the init: the
/// unit runs `guest-ready` with the nonce on the serial line of the
/// markers, a target wants it, and each console unit is masked.
#[test]
fn the_unit_of_the_run_and_its_masks() {
    let e = entries("n0");
    let u = unit("n0");
    assert!(e.contains(&Entry::File(format!("{ETC}/{UNIT}"), u.clone())));
    for want in [
        "ExecStart=/podvm/podvm guest-ready n0 /podvm-init\n",
        "StandardOutput=tty\n",
        "TTYPath=/dev/ttyS0\n",
        "Type=exec\n",
    ] {
        assert!(u.contains(want), "{want:?} is not in\n{u}");
    }
    assert!(
        !u.contains("DefaultDependencies=no"),
        "the unit runs after basic.target"
    );
    assert!(e.contains(&Entry::Link(
        format!("{ETC}/multi-user.target.wants/{UNIT}"),
        format!("../{UNIT}")
    )));
    for m in MASKS {
        assert!(
            e.contains(&Entry::Link(format!("{ETC}/{m}"), "/dev/null".into())),
            "{m}"
        );
    }
    let d = scratch("prepare-files");
    for x in &e {
        let (p, made) = match x {
            Entry::File(p, t) => (p, write_at(&d, p, |at| std::fs::write(at, t))),
            Entry::Link(p, t) => (p, write_at(&d, p, |at| std::os::unix::fs::symlink(t, at))),
        };
        made.unwrap_or_else(|err| panic!("{p}: {err}"));
    }
    assert_eq!(
        std::fs::read_link(d.join("etc/systemd/system/console-getty.service")).unwrap(),
        Path::new("/dev/null")
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// The deadline of the boot: its text gives back its two values, and a
/// text that does not parse is none.
#[test]
fn the_deadline_of_the_boot_reads_back() {
    assert_eq!(read_deadline(&deadline_text(12.5, 180)), Some((12.5, 180)));
    for bad in ["", "12.5", "x 180", "12.5 -1"] {
        assert_eq!(read_deadline(bad), None, "{bad:?}");
    }
    assert!(line("why").starts_with("podvm: machine tier: --podvm-init=image: why"));
}

/// The command gets the environment of the run with no variable of
/// podvm's own entry but the mark of the run.
#[test]
fn the_command_gets_no_variable_of_podvm() {
    let env: Vec<String> = [
        "PATH=/usr/bin",
        "PODVM_INIT=image",
        "PODVM_INIT_LOG=1",
        "PODVM_INIT_DISCARD=1",
        "LD_PRELOAD=/x.so",
        "HOME=/root",
    ]
    .map(String::from)
    .to_vec();
    assert_eq!(
        image_env(&env),
        ["PATH=/usr/bin", "PODVM_INIT=image", "HOME=/root"].map(String::from)
    );
}

/// The arguments of `guest-root`, and the kernel line that carries them.
#[test]
fn the_arguments_of_guest_root() {
    use crate::machine::guest::disk::root_args;
    let s = |v: &[&str]| v.iter().map(|a| a.to_string()).collect::<Vec<_>>();
    let ok = root_args(&s(&["--load", "a,b", "--disk", "--init=180", "ab12"])).unwrap();
    assert_eq!(
        (ok.load, ok.disk, ok.init, ok.nonce.as_str()),
        (
            vec!["a".to_string(), "b".to_string()],
            true,
            Some(180),
            "ab12"
        )
    );
    let plain = root_args(&s(&["--load", "a", "ab12"])).unwrap();
    assert_eq!((plain.disk, plain.init), (false, None));
    for bad in [
        &["--load", "a"][..],
        &["--load", "a", "--init=0", "ab"],
        &["--load", "a", "--init=x", "ab"],
        &["--load", "a", "--other", "ab"],
        &["--load"],
        &[],
    ] {
        assert!(root_args(&s(bad)).is_err(), "{bad:?}");
    }
    let mut argv = s(&["-append", "console=ttyS0 rdinit=/podvm-init"]);
    crate::machine::guest::disk::root::with_init(&mut argv, &s(&["ext4"]), false, Some(60), "ab");
    assert_eq!(
        argv[1],
        "console=ttyS0 rdinit=/podvm/podvm -- guest-root --load ext4 --init=60 ab"
    );
}
