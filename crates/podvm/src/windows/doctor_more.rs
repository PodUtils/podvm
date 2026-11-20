//! The checks of `podvm windows doctor` beyond the profile (TODO/machine.md
//! T-1871): the emulator and its version, the firmware pair, the free disk
//! and the file-size ceiling of the store of the catalog, a host that is
//! itself a virtual machine, the state of each name of the catalog, and the
//! next command. The doctor exits 1 where `run` would refuse.

use super::plan;

/// The QEMU that a warm start of a Windows guest needs (T-1874): the
/// first that saves an NVMe disk, the root disk of the plan. The commit
/// `9d378cab537d` "hw/nvme: add basic live migration support" is in
/// `v11.1.0` and in no 11.0.x release (podcrates@T-1035, call C866; before
/// it QEMU marks the device `unmigratable`).
const QEMU_WARM: (u32, u32) = (11, 1);
/// The free disk that a setup ISO and its disk take: an estimate.
const FULL_INSTALL_BYTES: u64 = 40 << 30;

/// Print the checks; 0 where a `windows:dos` run can start, 1 where not.
pub(super) fn report() -> i32 {
    let mut code = 0;
    let emulator = plan::emulator();
    let name = emulator.to_string_lossy().to_string();
    match version(&name) {
        Some((v, release)) => {
            println!("qemu: {v}");
            if release < QEMU_WARM {
                println!(
                    "note: QEMU {}.{} or later keeps the state of a disk for a warm start \
                     (TODO/machine.md T-1874); this one runs each guest from a cold boot",
                    QEMU_WARM.0, QEMU_WARM.1
                );
            }
        }
        None => {
            println!("fail: {name} is not on this machine");
            println!(
                "next: install QEMU: `apt install qemu-system-x86 qemu-utils`, \
                 `dnf install qemu-kvm qemu-img` or `apk add qemu-system-x86_64 qemu-img`"
            );
            code = 1;
        }
    }
    let (fw_code, fw_vars) = plan::firmware();
    let pair = fw_code.is_file() && fw_vars.is_file() && fw_code.parent() == fw_vars.parent();
    println!(
        "firmware pair: {}",
        if pair {
            "present, one directory (a full Windows guest boots UEFI)"
        } else {
            "absent or split; a full Windows guest needs the OVMF code and vars of one \
             build (`apt install ovmf`, `dnf install edk2-ovmf`)"
        }
    );
    let store = podvm_windows::catalog::store_dir();
    let probe_at = store
        .ancestors()
        .find(|p| p.is_dir())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".into());
    match podvm_image::space::read(&probe_at) {
        Ok(h) => {
            println!(
                "free disk at {probe_at}: {}",
                podvm_image::space::mib(h.free.bytes)
            );
            if h.free.bytes < FULL_INSTALL_BYTES {
                println!(
                    "note: a full Windows guest takes about {} (an estimate); `windows:dos` \
                     takes 50 MiB",
                    podvm_image::space::mib(FULL_INSTALL_BYTES)
                );
            }
        }
        Err(e) => println!("free disk at {probe_at}: unknown ({e})"),
    }
    match podvm_image::space::file_ceiling() {
        Some(c) => {
            println!("file-size ceiling (RLIMIT_FSIZE): {c} bytes; a larger download is refused")
        }
        None => println!("file-size ceiling (RLIMIT_FSIZE): none"),
    }
    if in_a_vm() {
        println!(
            "note: this host is itself a virtual machine; KVM in it needs nested \
             virtualization, and TCG runs without it"
        );
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    for e in podvm_windows::catalog::CATALOG {
        let have = store.join(e.file).is_file();
        println!(
            "windows:{:<13} {}",
            e.name,
            if have { "downloaded" } else { "not downloaded" }
        );
        // TODO/machine.md T-1871: the days left of the evaluation.
        if let Some((at, days)) = std::fs::read_to_string(super::install::meta_path(e))
            .ok()
            .as_deref()
            .and_then(podvm_windows::install::meta_of)
        {
            println!(
                "note: {}",
                podvm_windows::install::eval_line(e.name, at, days, now)
            );
        }
    }
    if code == 0 {
        println!("next: `podvm run windows:dos ver` boots FreeDOS and prints its version");
    }
    code
}

/// The version line of `emulator`, and its major number.
fn version(emulator: &str) -> Option<(String, (u32, u32))> {
    let out = std::process::Command::new(emulator)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    let line = String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()?
        .to_string();
    let release = release_of(&line)?;
    Some((line, release))
}

/// The major and the minor version of `QEMU emulator version X.Y.Z ...`;
/// a minor that is absent is 0. None where the line names no version.
fn release_of(line: &str) -> Option<(u32, u32)> {
    let text = line.split("version ").nth(1)?;
    let mut part = text
        .split(|c: char| !c.is_ascii_digit())
        .take_while(|p| !p.is_empty());
    let major = part.next()?.parse().ok()?;
    let minor = part.next().and_then(|m| m.parse().ok()).unwrap_or(0);
    Some((major, minor))
}

/// True where the CPU says that a hypervisor runs this host.
fn in_a_vm() -> bool {
    std::fs::read_to_string("/proc/cpuinfo")
        .map(|c| {
            c.lines()
                .any(|l| l.starts_with("flags") && l.contains(" hypervisor"))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_doctor_reads_the_version_of_an_emulator() {
        assert_eq!(version("/nonexistent-qemu-system"), None);
        if let Some((line, release)) = version("qemu-system-x86_64") {
            assert!(line.contains("version"), "{line}");
            assert!(release >= (2, 0), "{line}");
        }
    }

    /// T-1874, podcrates@T-1035: a warm start from QEMU 11.1, not from 11.0
    /// (the static QEMU 11.0.0 of the store runs each guest from a cold boot).
    #[test]
    fn a_warm_start_needs_qemu_11_1() {
        for (line, warm) in [
            (
                "QEMU emulator version 8.2.2 (Debian 1:8.2.2+ds-0ubuntu1.18)",
                Some(false),
            ),
            ("QEMU emulator version 10.2.1", Some(false)),
            ("QEMU emulator version 11.0.0", Some(false)),
            ("QEMU emulator version 11.0.5", Some(false)),
            ("QEMU emulator version 11.1.0", Some(true)),
            ("QEMU emulator version 11.1.3 (v11.1.3)", Some(true)),
            ("QEMU emulator version 12", Some(true)),
            ("QEMU emulator version 12.0.0", Some(true)),
            ("qemu-system-x86_64: no version", None),
            ("", None),
        ] {
            assert_eq!(release_of(line).map(|r| r >= QEMU_WARM), warm, "{line}");
        }
    }
}
