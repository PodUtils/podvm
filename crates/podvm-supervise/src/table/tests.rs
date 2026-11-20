use super::*;

/// ⭐ T-0710: the memo lives beside the container record, on the host, and
/// the environment names the constant descriptor number for it.
#[test]
fn the_memo_lives_beside_the_container_record() {
    let d = std::env::temp_dir().join(format!("podvm-memo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let s = Store::open(&d).unwrap();
    let p = memo_path(&s, "abc123");
    assert!(p.ends_with("containers/abc123/ownership.memo"), "{p:?}");
    assert_eq!(memo_fd_env(), "PODVM_MEMO_FD=17");
    assert_eq!(MEMO_CHILD_FD, 17);
    // Creating twice keeps what is there: a second entry must not truncate
    // the first process's ownership memory.
    ensure_memo_file(&p).unwrap();
    std::fs::write(&p, b"first").unwrap();
    ensure_memo_file(&p).unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), b"first");
    let _ = std::fs::remove_dir_all(&d);
}

/// TODO/enter.md T-1411. A table written before the userland drive
/// still parses, with an empty drive: old records enter through
/// `spawn_selected` exactly as before.
#[test]
fn a_table_without_the_userland_drive_still_parses() {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "podvm_containers": TABLE_VERSION,
        "containers": [{
            "id": "abc",
            "name": "one",
            "image": "img",
            "manifest_digest": "sha256:0",
            "rootfs": "/tmp",
            "argv": ["true"],
            "env": [],
            "working_dir": "/",
            "created_at": "now",
            "started_at": null,
            "finished_at": null,
            "state": "created",
            "pid": null,
            "launcher_pid": null,
            "exit_code": null,
            "noticed": null,
            "rung": "chroot"
        }]
    }))
    .unwrap();
    let t: Table =
        pod_supervise::table::decode(&crate::settings_at(std::path::Path::new("/store")), &bytes)
            .unwrap();
    assert!(t.containers[0].userland_exec.is_empty());
}

fn tally_rec(dev: u64, ino: u64, uid: u32, gid: u32, set: u32) -> Vec<u8> {
    let mut r = Vec::with_capacity(32);
    r.extend(dev.to_le_bytes());
    r.extend(ino.to_le_bytes());
    r.extend(uid.to_le_bytes());
    r.extend(gid.to_le_bytes());
    r.extend(set.to_le_bytes());
    r.extend(0u32.to_le_bytes());
    r
}

fn tally_cont(path: &[u8; 16]) -> Vec<u8> {
    let mut r = Vec::with_capacity(32);
    r.extend(u64::MAX.to_le_bytes());
    r.extend(0u64.to_le_bytes());
    r.extend(path);
    r
}

/// T-0708: the tally scan counts complete records and drops torn chains.
#[test]
fn the_tally_scan_counts_complete_records_only() {
    let d = std::env::temp_dir().join(format!("podvm-tally-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("memo");
    let mut b = Vec::new();
    // An ownership record passes through untouched.
    b.extend(tally_rec(7, 9, 0, 0, 0));
    // Two mknods, one unshare, one clone.
    b.extend(tally_rec(u64::MAX, TALLY_MKNOD, 0, 0, 0));
    b.extend(tally_rec(u64::MAX, TALLY_MKNOD, 0o644, 0, 0));
    b.extend(tally_rec(u64::MAX, TALLY_UNSHARE, 0x20000, 0, 0));
    b.extend(tally_rec(u64::MAX, TALLY_CLONE, 0x20000, 0, 0));
    // A mount with a one-record chain each side.
    b.extend(tally_rec(u64::MAX, TALLY_MOUNT, 0, 1, 1));
    b.extend(tally_cont(b"/src\0\0\0\0\0\0\0\0\0\0\0\0"));
    b.extend(tally_cont(b"/tgt\0\0\0\0\0\0\0\0\0\0\0\0"));
    // A header broken by an ownership record: a torn chain, dropped.
    b.extend(tally_rec(u64::MAX, TALLY_MOUNT, 0, 1, 0));
    b.extend(tally_rec(7, 9, 0, 0, 0));
    // A header at EOF with its chain missing: dropped too.
    b.extend(tally_rec(u64::MAX, TALLY_MOUNT, 0, 1, 0));
    // An unknown kind: skipped.
    b.extend(tally_rec(u64::MAX, 9, 0, 0, 0));
    // T-0413: two served /proc answers with their detail words.
    b.extend(tally_rec(u64::MAX, TALLY_PROC, 1, 0, 0));
    b.extend(tally_rec(u64::MAX, TALLY_PROC, 5, 0, 0));
    std::fs::write(&p, &b).unwrap();
    assert_eq!(
        emulated_counts(&p),
        EmulatedCounts {
            mknod: 2,
            mount: 1,
            unshare: 1,
            clone: 1,
            procfs: 2,
        }
    );
    // A missing file counts nothing rather than failing.
    assert_eq!(
        emulated_counts(&d.join("absent")),
        EmulatedCounts::default()
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// PodUtils/podrunner#304 (TODO/enter.md T-1982): the confine layer of a
/// family of the userland rung lets the payload's process run podvm itself
/// and the emulator; the launcher of `run -d` takes the same spec, and the
/// exec of the syscall tier was denied without it.
#[test]
fn the_userland_layer_runs_podvm_itself_and_the_emulator() {
    use podvm_enter::confine::Access;
    let d = std::env::temp_dir().join(format!("podvm-t1982-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("tmp")).unwrap();
    let root = d.to_string_lossy().to_string();
    let maps = podvm_enter::userland::Maps::new(&podvm_enter::NAMES, &root, &[]).unwrap();
    let ask = podvm_enter::confine::parse("read=/;write=/tmp").unwrap();
    let rx = Access::READ | Access::EXECUTE;
    let me = std::env::current_exe()
        .unwrap()
        .to_string_lossy()
        .to_string();
    for (emu, want_emu) in [
        (None, None),
        (Some("62:/opt/qemu-x86_64"), Some("/opt/qemu-x86_64")),
    ] {
        let spec = userland_host_spec(&ask, &maps, emu).unwrap();
        let has = |p: &str| {
            spec.paths
                .iter()
                .any(|r| r.path == p && r.access.contains(rx))
        };
        assert!(has(&me), "podvm itself: {:?}", spec.paths);
        if let Some(e) = want_emu {
            assert!(has(e), "the emulator: {:?}", spec.paths);
        }
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// TODO/enter.md T-1985: the write table of a spec holds the resolved host
/// path of each rule with write, once, and no other; a caller's own value
/// never reaches the payload; a path that the table cannot spell refuses.
#[test]
fn the_write_table_holds_each_rule_with_write() {
    use podvm_enter::confine::{Access, Spec};
    let d = std::env::temp_dir().join(format!("podvm-t1985-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("w")).unwrap();
    std::os::unix::fs::symlink(d.join("w"), d.join("link")).unwrap();
    let real = std::fs::canonicalize(d.join("w")).unwrap();
    let w = real.to_string_lossy().to_string();
    let link = d.join("link").to_string_lossy().to_string();
    let spec = Spec::new()
        .allow(w.clone(), Access::READ | Access::WRITE)
        .allow(link, Access::WRITE)
        .allow("/usr".to_string(), Access::READ | Access::EXECUTE)
        .allow("/dev/null".to_string(), Access::WRITE);
    let mut env = vec!["A=1".to_string(), format!("{WRITE_TABLE_VAR}=/")];
    set_write_table(&mut env, Some(&spec)).unwrap();
    assert_eq!(
        env,
        [
            "A=1".to_string(),
            format!("{WRITE_TABLE_VAR}={w}:/dev/null")
        ]
    );
    set_write_table(&mut env, None).unwrap();
    assert_eq!(env, ["A=1"], "no spec: no table, and the old one goes");
    let bad = Spec::new().allow("/a:b".to_string(), Access::WRITE);
    assert!(set_write_table(&mut env, Some(&bad)).is_err());
    let _ = std::fs::remove_dir_all(&d);
}
