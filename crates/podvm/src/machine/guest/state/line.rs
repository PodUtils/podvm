//! TODO/machine.md T-2008: the emulator line of a guest of a state, and
//! of its root disk with systemd as PID 1 (T-1994).

use std::path::{Path, PathBuf};

use podvm_windows::plan::Accel;

use super::super::{arch, channel, vol, Request};
use super::SLOTS;

/// The root disk of a guest of a state with systemd as PID 1
/// (TODO/machine.md T-1994): the disk, the modules that `guest-root`
/// loads, the bound of the boot of the init, and the nonce of the state.
#[derive(Clone)]
pub(crate) struct RootLine<'a> {
    pub disk: &'a Path,
    pub load: &'a [String],
    pub bound: u64,
    pub nonce: &'a str,
}

/// The paths of one emulator line.
pub(super) struct Paths<'a> {
    pub initrd: &'a Path,
    pub err: &'a Path,
    pub qmp: Option<&'a Path>,
    /// The directory of each slot, and whether it is read-only.
    pub slots: Vec<(PathBuf, bool)>,
    pub incoming: Option<String>,
    /// The root disk of a guest with systemd as PID 1 (TODO/machine.md
    /// T-1994), or none for the root in the initramfs.
    pub root: Option<RootLine<'a>>,
}

impl<'a> Paths<'a> {
    /// The line with a fixed name for each path of a run, for the
    /// fingerprint; `root` keeps its modules and its bound, and takes a
    /// fixed disk and nonce.
    pub(super) fn fixed(root: Option<&RootLine<'a>>) -> Self {
        Self {
            initrd: Path::new("INITRD"),
            err: Path::new("ERR"),
            qmp: None,
            slots: (0..SLOTS)
                .map(|i| (PathBuf::from(format!("SLOT{i}")), true))
                .collect(),
            incoming: None,
            root: root.map(|r| RootLine {
                disk: Path::new("ROOTDISK"),
                load: r.load,
                bound: r.bound,
                nonce: "NONCE",
            }),
        }
    }
}

/// The emulator line of a guest of a state: the line of each guest, with
/// [`SLOTS`] 9p devices.
pub(super) fn line(r: &Request, accel: Accel, mem: u64, kernel: &Path, p: &Paths) -> Vec<String> {
    let mut extra = super::super::stderr::argv(p.err);
    // The device has the same name at save and load. The fingerprint uses
    // the fixed stderr name to derive this socket name as well.
    if r.channel.is_some() {
        extra.extend(channel::argv(&channel::socket_path(p.err)));
    }
    for (i, (dir, ro)) in p.slots.iter().enumerate() {
        extra.push("-virtfs".to_string());
        extra.push(vol::virtfs(
            dir,
            &vol::Vol {
                tag: format!("podvm{i}"),
                guest: String::new(),
                ro: *ro,
            },
        ));
    }
    extra.extend(r.qemu_args.iter().cloned());
    if let Some(q) = p.qmp {
        extra.push("-qmp".to_string());
        extra.push(format!("unix:{},server=on,wait=off", q.display()));
    }
    if let Some(root) = &p.root {
        extra.extend(super::super::disk::root::argv(root.disk));
    }
    if let Some(uri) = &p.incoming {
        extra.push("-incoming".to_string());
        extra.push(uri.clone());
    }
    let mut argv = super::super::qemu_argv(accel, mem, kernel, p.initrd, &extra);
    arch::adapt(&mut argv, r.arch);
    if let Some(root) = &p.root {
        super::super::disk::root::with_init(
            &mut argv,
            root.load,
            false,
            Some((root.bound, true)),
            root.nonce,
        );
    }
    arch::err_port(&mut argv, r.arch);
    if r.net_none {
        super::super::no_nic(&mut argv);
    }
    super::super::with_cpus(&mut argv, r.cpus);
    argv
}
