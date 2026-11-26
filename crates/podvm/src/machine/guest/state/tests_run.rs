use super::*;
use crate::machine::guest::arch;

fn request() -> Request<'static> {
    Request {
        verb: "run",
        keeper: None,
        pids: None,
        disk: None,
        arch: arch::Arch::Amd64,
        loud: false,
        store_root: Path::new("/store"),
        rootfs: Path::new("/rootfs"),
        digest: "sha256:aa",
        argv: vec!["true".into()],
        env: vec![crate::initns::MARK.into()],
        workdir: "/".into(),
        mem: None,
        cpus: None,
        broker: None,
        timeout: None,
        qemu_args: Vec::new(),
        copies: Vec::new(),
        volumes: Vec::new(),
        stdin: None,
        tty: false,
        net_none: true,
        channel: None,
        sealed: None,
        engine: false,
    }
}

#[test]
fn the_channel_device_changes_the_state_shape_and_its_settings_belong_to_the_job() {
    let mut r = request();
    let line_of = |r: &Request| line(r, Accel::Tcg, 0, Path::new("KERNEL"), &Paths::fixed(None));
    let without = line_of(&r);
    let template = init::template("NONCE", !r.net_none);
    assert!(!template.contains("guest-channel"));
    assert!(!template.contains("pids.max"));
    let mut with = None;
    for (target, port, pids) in [("/first.sock", 8080, "64"), ("/second.sock", 8081, "96")] {
        r.channel = Some((PathBuf::from(target), port));
        r.pids = Some(pids.into());
        assert_eq!(
            refusal(&Asks {
                native: true,
                ..Asks::of(&r, true, true)
            }),
            None
        );
        let args = line_of(&r);
        assert_ne!(without, args);
        assert!(args.iter().any(|a| a.contains("path=ERR.chd/s")));
        assert!(!args.iter().any(|a| a.contains(target)));
        if let Some(first) = &with {
            assert_eq!(*first, args);
        } else {
            with = Some(args);
        }
        let frame = job(&r, "RNONCE", &[], init::Held::Ns).unwrap();
        let script = Job::read(&mut frame.as_slice()).unwrap().script;
        let limit = script
            .find(&format!(
                "echo {pids} > /sys/fs/cgroup/podvm-payload/pids.max"
            ))
            .unwrap();
        let listener = script
            .find(&format!("/podvm/podvm guest-channel {port} &"))
            .unwrap();
        let command = script.find("guest-init --attach").unwrap();
        assert!(limit < command && listener < command);
        assert!(!script.contains(target));
    }
}

/// TODO/machine.md T-1994: the guest of a state with systemd as PID 1 has
/// the root disk read-only in its line, with a fixed name and nonce in the
/// fingerprint, and `guest-root --held`; its template takes the job from
/// the serial line in the unit of the run; the job runs the command in
/// that unit (no `guest-init`), with the channel, the process limit and no
/// variable of podvm's own entry but the mark of the run.
#[test]
fn the_state_of_systemd_as_pid1_has_the_root_disk_and_its_job_runs_in_the_unit() {
    let mut r = request();
    r.channel = Some((PathBuf::from("/s.sock"), 8080));
    r.pids = Some("64".into());
    r.env = ["PODVM_INIT=image", "PODVM_INIT_LOG=1", "A=b"]
        .map(String::from)
        .to_vec();
    let load = vec!["ext4".to_string()];
    let root = RootLine {
        disk: Path::new("/store/root.img"),
        load: &load,
        bound: 180,
        nonce: "abc",
    };
    let kernel = Path::new("KERNEL");
    let fixed = line(&r, Accel::Tcg, 0, kernel, &Paths::fixed(Some(&root)));
    assert!(
        fixed
            .iter()
            .any(|a| a.contains("file=ROOTDISK,readonly=on")),
        "{fixed:?}"
    );
    assert!(
        fixed
            .iter()
            .any(|a| a.ends_with("--load ext4 --init=180 --held NONCE")),
        "{fixed:?}"
    );
    assert!(!fixed
        .iter()
        .any(|a| a.contains("/store/root.img") || a.contains("abc")));
    assert_ne!(fixed, line(&r, Accel::Tcg, 0, kernel, &Paths::fixed(None)));
    let frame = job(&r, "RNONCE", &[], init::Held::Pid1).unwrap();
    let script = Job::read(&mut frame.as_slice()).unwrap().script;
    for want in [
        "echo 64 > /sys/fs/cgroup/podvm-payload/pids.max",
        "/podvm/podvm guest-channel 8080 &",
        "--podvm-channel: the guest channel did not start",
        "export A='b'\n",
        "export PODVM_INIT='image'\n",
    ] {
        assert!(script.contains(want), "{want:?} is not in\n{script}");
    }
    assert!(
        !script.contains("guest-init") && !script.contains("PODVM_INIT_LOG"),
        "{script}"
    );
    let t = init::template_pid1("N", true);
    for want in [
        "/podvm/podvm guest-net\n",
        "/podvm/podvm guest-9p\n",
        "/podvm/podvm guest-job N 0 /podvm/job.sh </dev/ttyS0 ||",
        ". /podvm/job.sh\n",
    ] {
        assert!(t.contains(want), "{want:?} is not in\n{t}");
    }
    assert!(!t.contains("guest-init"), "{t}");
}
