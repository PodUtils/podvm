use super::*;

/// T-2108: one rule for each mount of the plan: a point for each volume
/// and each `--tmpfs` out of `/tmp`, a file for a volume of a file, and
/// none for a source that cannot be read.
#[test]
fn read_only_points_of_each_mount() {
    let vol = |host: &str, guest: &str| VolumeBind {
        host: host.to_string(),
        guest: guest.to_string(),
        read_only: false,
        host_base: None,
    };
    let tmp = |guest: &str| TmpfsMount {
        guest: guest.to_string(),
        bytes: std::num::NonZeroU64::new(1 << 20).unwrap(),
    };
    let is_dir = |h: &str| match h {
        "/d" => Some(true),
        "/f" => Some(false),
        _ => None,
    };
    let d = |g: &str| Point::Dir(g.to_string());
    let f = |g: &str| Point::File(g.to_string());
    let rows: Vec<(Vec<VolumeBind>, Vec<TmpfsMount>, Vec<Point>)> = vec![
        (vec![], vec![], vec![]),
        (vec![vol("/d", "/data")], vec![], vec![d("/data")]),
        (vec![vol("/f", "/etc/conf")], vec![], vec![f("/etc/conf")]),
        (vec![vol("/gone", "/x")], vec![], vec![]),
        (vec![vol("/d", "/tmp/v"), vol("/d", "/tmp")], vec![], vec![]),
        (vec![vol("/d", "/tmpx")], vec![], vec![d("/tmpx")]),
        (
            vec![],
            vec![tmp("/w"), tmp("/tmp"), tmp("/tmp/s/")],
            vec![d("/w")],
        ),
        (
            vec![vol("/f", "/a")],
            vec![tmp("/run")],
            vec![f("/a"), d("/run")],
        ),
    ];
    for (v, t, want) in rows {
        assert_eq!(points(&v, &t, is_dir), want, "{v:?} {t:?}");
    }
}

/// T-2108: the variable of the record asks for the read-only root with
/// the value 1 only, and leaves the environment of the payload.
#[test]
fn read_only_variable_of_the_record() {
    let rows: [(&[&str], bool); 5] = [
        (&["PODVM_RO_ROOT=1", "A=1"], true),
        (&["PODVM_RO_ROOT=0", "A=1"], false),
        (&["PODVM_RO_ROOTX=1"], false),
        (&["A=PODVM_RO_ROOT=1"], false),
        (&[], false),
    ];
    for (env, want) in rows {
        let mut e: Vec<String> = env.iter().map(|s| s.to_string()).collect();
        assert_eq!(take(&mut e), want, "{env:?}");
        assert!(!e.iter().any(|x| x.starts_with("PODVM_RO_ROOT=")), "{e:?}");
    }
}
