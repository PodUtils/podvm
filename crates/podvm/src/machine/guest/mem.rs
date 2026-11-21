//! TODO/machine.md T-1922 (PodUtils/podrunner#199): the memory of a guest
//! for the size of its initrd.
//!
//! The guest holds the initrd and its unpacked copy in RAM at the same
//! time. Measured with experiment 463 (`kvm.yml`, run 43876504275): an
//! initrd of 1416 MiB resets the guest in its early boot at `-m 4G`, with
//! no line on the console, and boots at 5G, 6G and 8G. Experiment 470
//! (T-2047, `kvm.yml` run 44854125431) measured the curve under KVM: the
//! smallest memory in steps of 256 MiB that boots an initrd of 263, 519,
//! 1031 and 2055 MiB is 1024, 1792, 3328 and 6656 MiB. The need grows
//! faster than three times the initrd (from 1031 to 2055 MiB: 3.25 times
//! the growth), so the rule below is 13/4 of the initrd and 512 MiB,
//! rounded up to 256 MiB: an estimate that each point holds.

use std::path::Path;

/// Below this size of initrd the default memory stays as it is.
const LARGE: u64 = 100 << 20;

/// The estimate of the memory that an initrd of `bytes` needs, rounded up
/// to 256 MiB.
pub(super) fn estimate(bytes: u64) -> u64 {
    let need = (bytes / 4).saturating_mul(13).saturating_add(512 << 20);
    need.div_ceil(256 << 20) * (256 << 20)
}

/// The memory of the guest: `asked`, or the default raised for a large
/// initrd. `warn` prints one line where `asked` is below the estimate.
pub(super) fn size(verb: &str, asked: Option<u64>, initrd: &Path, warn: bool) -> u64 {
    let bytes = std::fs::metadata(initrd).map(|m| m.len()).unwrap_or(0);
    let need = estimate(bytes);
    match asked {
        None if bytes > LARGE => need.max(super::DEFAULT_MEM),
        None => super::DEFAULT_MEM,
        Some(m) => {
            if warn && bytes > LARGE && m < need {
                eprintln!(
                    "podvm {verb}: machine tier: the initrd of {} MiB likely needs about {} MiB \
                     of memory, and --podvm-mem gives {} MiB: the guest can reset before the \
                     command (an estimate, TODO/machine.md T-1922)",
                    bytes.div_ceil(1 << 20),
                    need >> 20,
                    m >> 20
                );
            }
            m
        }
    }
}

#[cfg(test)]
mod tests {
    use super::estimate;

    /// Each measured point: (the initrd in MiB, the largest memory that
    /// did not boot, the smallest that booted), from experiment 463 (1416)
    /// and experiment 470 under KVM (the others). The estimate is above
    /// each failure and at most one step of 256 MiB over the boot plus the
    /// margin of the rule; it is never below a boot.
    #[test]
    fn the_estimate_holds_each_measured_point() {
        for (initrd, failed, booted) in [
            (263u64, 768u64, 1024u64),
            (519, 1536, 1792),
            (1031, 3072, 3328),
            (1416, 4096, 5120),
            (2055, 6400, 6656),
        ] {
            let e = estimate(initrd << 20) >> 20;
            assert!(e > failed && e >= booted, "{initrd} MiB: {e} MiB");
            assert!(
                e <= booted + 1024,
                "{initrd} MiB: {e} MiB is far over {booted}"
            );
        }
        assert_eq!(estimate(0), 512 << 20);
    }
}
