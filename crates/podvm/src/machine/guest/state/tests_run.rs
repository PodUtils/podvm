use super::*;

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
    let line_of = |r: &Request| line(r, Accel::Tcg, 0, Path::new("KERNEL"), &Paths::fixed());
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
        let frame = job(&r, "RNONCE", &[]).unwrap();
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
