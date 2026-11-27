//! The files that podvm adds to the root disk of an incus VM
//! (TODO/machine.md T-1998). The exec and file calls of incusd reach a VM
//! through `incus-agent` in the guest. An image of incus starts the agent
//! itself; an OCI image does not. incusd shares the agent and its
//! certificates with each VM as the 9p drive `config`, and it gives the
//! guest the virtio port `org.linuxcontainers.incus`. podvm adds:
//!
//! * [`RULE`]: udev starts [`UNIT`] when the port comes;
//! * [`UNIT`]: the setup, then the agent, as a `Type=notify` service;
//! * [`SETUP`]: it mounts the drive, copies it to a tmpfs at
//!   `/run/incus_agent`, and releases the drive;
//! * `/.podvm/podvm`, which runs the command of the instance;
//! * an empty `/etc/machine-id`, so that systemd makes a new one in each
//!   VM of the same image (T-1996).

use std::path::Path;

use crate::machine::guest::disk::tree::{Kind, Tree};

/// The path of the setup script in the guest.
pub const SETUP_PATH: &str = "/.podvm/incus-agent-setup";

/// The setup of the agent.
pub const SETUP: &str = r#"#!/bin/sh
# podvm (TODO/machine.md T-1998): incusd shares incus-agent with this VM
# as the 9p drive "config". Copy it to a tmpfs, then release the drive.
set -eu
run=/run/incus_agent
umount -l "$run" 2>/dev/null || true
mkdir -p "$run" "$run.mnt"
modprobe 9pnet_virtio 2>/dev/null || true
mount -t 9p config "$run.mnt" -o ro,access=0,trans=virtio,size=1048576
mount -t tmpfs tmpfs "$run" -o mode=0700,size=50M
cp -Ra "$run.mnt/." "$run/"
umount "$run.mnt"
rmdir "$run.mnt"
chown -R 0:0 "$run"
"#;

/// The unit of the agent.
pub const UNIT: &str = "[Unit]
Description=incus-agent of a VM of podvm (TODO/machine.md T-1998)
Before=multi-user.target
DefaultDependencies=no

[Service]
Type=notify
WorkingDirectory=-/run/incus_agent
ExecStartPre=/.podvm/incus-agent-setup
ExecStart=/run/incus_agent/incus-agent
Restart=on-failure
RestartSec=5s
StartLimitInterval=60
StartLimitBurst=10
";

/// The udev rule that starts the agent.
pub const RULE: &str = "SYMLINK==\"virtio-ports/org.linuxcontainers.incus\", TAG+=\"systemd\", \
ENV{SYSTEMD_WANTS}+=\"incus-agent.service\"\n";

/// The files of [`add`]: the path in the guest, the text (`None`: podvm
/// itself) and the mode.
pub const FILES: [(&str, Option<&str>, u32); 5] = [
    ("/.podvm/podvm", None, 0o755),
    (SETUP_PATH, Some(SETUP), 0o755),
    ("/etc/systemd/system/incus-agent.service", Some(UNIT), 0o644),
    ("/etc/udev/rules.d/99-incus-agent.rules", Some(RULE), 0o644),
    ("/etc/machine-id", Some(""), 0o444),
];

/// Add the files of the agent and `podvm` (a file of the host) to `t`.
/// The texts go into `stage`, a directory of this build, which must stay
/// until the disk is written.
pub fn add(t: &mut Tree, podvm: &Path, stage: &Path) -> Result<(), String> {
    for (i, (path, text, mode)) in FILES.into_iter().enumerate() {
        let src = match text {
            None => podvm.to_path_buf(),
            Some(text) => {
                let p = stage.join(format!("file-{i}"));
                std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))?;
                p
            }
        };
        let size = std::fs::metadata(&src)
            .map_err(|e| format!("{}: {e}", src.display()))?
            .len();
        t.put(path, Kind::Reg { src, size }, mode)?;
    }
    Ok(())
}
