//! The bus of the root disk, and the size, of a Windows install
//! (TODO/machine.md T-1867).

/// TODO/machine.md T-1867: `PODVM_WINDOWS_ROOT_BUS=sata` puts the root disk
/// of the install on the SATA (AHCI) bus of q35, not on NVMe, to tell a
/// stall of the emulated NVMe disk from one of the setup. Run 43726557269
/// stalled in the apply of the WIM at 5%, with no error in Panther.
/// ⛔ A diagnosis only: Windows installed on SATA does not boot from NVMe.
pub(super) fn root_bus(argv: Vec<String>) -> Vec<String> {
    let argv = rebooting(argv);
    let argv = match std::env::var("PODVM_WINDOWS_ISO_BUS").as_deref() {
        Ok("usb") => iso_on_usb(argv),
        _ => argv,
    };
    if std::env::var("PODVM_WINDOWS_ROOT_BUS").as_deref() != Ok("sata") {
        return argv;
    }
    on_sata(argv)
}

/// TODO/machine.md T-1867: the install reboots several times, so its
/// emulator keeps running across a reboot. `pod-guest` gives `-no-reboot`
/// to a run that ends on a power-off of the agent; with it, run 44017901200
/// ended at the first reboot, after Setup had applied the image ("successful
/// finalize", then "Requesting reboot of system"). The install ends on the
/// power-off of its answer file, and its deadline bounds a reboot loop.
pub(super) fn rebooting(argv: Vec<String>) -> Vec<String> {
    argv.into_iter().filter(|a| a != "-no-reboot").collect()
}

/// TODO/machine.md T-1867: `PODVM_WINDOWS_ISO_BUS=usb` puts the setup ISO on
/// a USB CD (`usb-bot` and `scsi-cd` on the xHCI bus), not on `ide-cd`.
/// Run 43740331439: with the root on NVMe and on SATA alike, the apply of
/// `install.wim` from the ISO stopped with 1.53 GiB on the disk, as in each
/// run before; the source is the common part.
fn iso_on_usb(argv: Vec<String>) -> Vec<String> {
    let mut out = Vec::with_capacity(argv.len() + 2);
    for a in argv {
        match a.strip_prefix("ide-cd,drive=cd0,bus=ide.0") {
            Some(rest) => {
                out.push("usb-bot,bus=xhci.0,id=isobot".to_string());
                out.push("-device".to_string());
                out.push(format!("scsi-cd,bus=isobot.0,drive=cd0{rest}"));
            }
            None => out.push(a),
        }
    }
    out
}

/// The argv with the NVMe device of the root on port 2 of the AHCI bus.
fn on_sata(argv: Vec<String>) -> Vec<String> {
    argv.into_iter()
        .map(|a| match a.strip_prefix("nvme,drive=root,") {
            Some(rest) => {
                let keep: Vec<&str> = rest
                    .split(',')
                    .filter(|o| !o.starts_with("serial="))
                    .collect();
                let mut d = "ide-hd,drive=root,bus=ide.2".to_string();
                for o in keep {
                    d.push(',');
                    d.push_str(o);
                }
                d
            }
            None => a,
        })
        .collect()
}

/// TODO/machine.md T-1867: the memory and the CPUs of an install from
/// `PODVM_WINDOWS_INSTALL_MEM` (as `--podvm-mem`) and
/// `PODVM_WINDOWS_INSTALL_CPUS`; else the defaults (4096 MiB). Run
/// 43767666467: the emulator went idle at 2.14 GB read of the ISO, with the
/// guest's CPU near 7%, so the size of the guest is the next suspect.
pub(super) fn install_args() -> Result<super::args::Args, i32> {
    // TODO/gate.md T-1947: a value out of its rule is a refusal, not the
    // default.
    let mem = crate::number::env("windows", &crate::number::INSTALL_MEM)?;
    let cpus = crate::number::env("windows", &crate::number::INSTALL_CPUS)?.map(|n| n as u32);
    Ok(super::args::Args {
        mem,
        cpus,
        ..Default::default()
    })
}

#[cfg(test)]
#[test]
fn the_iso_moves_to_a_usb_cd_with_its_boot_index() {
    let a = vec![
        "-device".to_string(),
        "ide-cd,drive=cd0,bus=ide.0,bootindex=0".to_string(),
    ];
    let b = iso_on_usb(a);
    assert_eq!(
        b,
        [
            "-device",
            "usb-bot,bus=xhci.0,id=isobot",
            "-device",
            "scsi-cd,bus=isobot.0,drive=cd0,bootindex=0"
        ]
    );
}

#[cfg(test)]
#[test]
fn the_root_moves_to_sata_with_its_boot_index() {
    let a = vec![
        "-device".to_string(),
        "nvme,drive=root,serial=wqroot,bootindex=1".to_string(),
    ];
    assert_eq!(on_sata(a)[1], "ide-hd,drive=root,bus=ide.2,bootindex=1");
    let b = vec!["nvme,drive=mailbox,serial=x".to_string()];
    assert_eq!(on_sata(b.clone()), b);
}

#[cfg(test)]
#[test]
fn the_install_keeps_running_across_a_reboot() {
    let a: Vec<String> = ["-rtc", "base=localtime", "-no-reboot", "-nic", "user"]
        .map(String::from)
        .to_vec();
    assert_eq!(rebooting(a), ["-rtc", "base=localtime", "-nic", "user"]);
}
