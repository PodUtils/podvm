//! The parts of the emulator line that a run changes, and the notice of
//! the tier.

use podvm_windows::plan::Accel;

use super::Request;

/// `--podvm-net none`: the guest has no NIC.
pub(super) fn no_nic(argv: &mut [String]) {
    if let Some(nic) = argv
        .iter_mut()
        .find(|a| a.starts_with("user,model=virtio-net-pci"))
    {
        *nic = "none".to_string();
    }
}

/// TODO/cli.md T-1654 (`limits.cpus`). With `acpi=off` the kernel reads
/// the MP table, which names one CPU, so a guest of more CPUs keeps ACPI
/// (measured on 2026-11-11: `nproc` 1 with `acpi=off`, 2 without it).
pub(super) fn with_cpus(argv: &mut [String], cpus: Option<u32>) {
    if let Some(n) = cpus.filter(|n| *n > 1) {
        if let Some(i) = argv.iter().position(|a| a == "-smp") {
            argv[i + 1] = n.to_string();
        }
        if let Some(m) = argv.iter_mut().find(|a| *a == "pc,acpi=off") {
            *m = "pc".to_string();
        }
    }
}

/// The notice of the machine tier, or `None` where the banner is off
/// (TODO/cli.md T-1824: `-q`, and the default, keep stderr silent).
pub fn notice(r: &Request, accel: Accel, source: &str) -> Option<String> {
    if !r.loud {
        return None;
    }
    Some(format!(
        "podvm {}: machine tier: a guest under {} ({}), {} (TODO/machine.md T-1656)",
        r.verb,
        match accel {
            Accel::Kvm => "KVM",
            Accel::Tcg => "TCG",
        },
        match accel {
            Accel::Kvm => "hardware virtualization",
            Accel::Tcg => "the emulator process is the boundary, not hardware isolation",
        },
        source
    ))
}
