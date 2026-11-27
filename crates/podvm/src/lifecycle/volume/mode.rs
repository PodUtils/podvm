//! The mode of one `-v HOST:GUEST[:OPTIONS]`: `rw`, `ro`, `overlay`
//! (TODO/enter.md T-2101) and `mapped` (TODO/machine.md T-2103).

/// The mode of a volume (`-v HOST:GUEST[:OPTIONS]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Rw,
    Ro,
    /// HOST is the read-only lower layer of an overlay (TODO/enter.md T-2101).
    Overlay,
    /// A 9p share with `security_model=mapped-xattr` on the machine tier,
    /// writable or read-only (TODO/machine.md T-2103).
    Mapped {
        ro: bool,
    },
}

impl Mode {
    /// The modes that take a directory only.
    pub fn dir_only(self) -> bool {
        matches!(self, Mode::Overlay | Mode::Mapped { .. })
    }
}

/// The mode of the options of `-v`. `ro` with `rw` is a pair that
/// conflicts (TODO/cli.md T-2108, OR-B42-08: it was `ro`, silently; docker
/// refuses it), and so is `overlay` with `ro`, `rw` or `mapped`.
pub fn mode_of(opts: &str) -> Result<Mode, String> {
    let (mut ro, mut rw, mut overlay, mut mapped) = (false, false, false, false);
    for o in opts.split(',') {
        match o {
            "" => {}
            "rw" => rw = true,
            // TODO/cli.md T-2061: `z` and `Z` label a source for SELinux.
            // podvm labels nothing, and docker too does nothing on a host
            // with no SELinux, so the option is accepted as a no-op.
            "z" | "Z" => {}
            "ro" => ro = true,
            "overlay" => overlay = true,
            "mapped" => mapped = true,
            other => {
                return Err(format!(
                    "the option {other:?} has nothing to act on: podvm takes rw, ro, overlay, \
                     mapped, z and Z (TODO/cli.md T-1655, T-2061, TODO/enter.md T-2101, \
                     TODO/machine.md T-2103)"
                ))
            }
        }
    }
    if ro && rw {
        return Err(format!(
            "the options {opts:?} conflict: ro makes the volume read-only and rw makes it \
             writable; name one (TODO/cli.md T-2108)"
        ));
    }
    match (overlay, ro || rw || mapped) {
        (true, true) => Err(format!(
            "the options {opts:?} conflict: overlay keeps HOST read-only and makes GUEST \
             writable, so it takes neither ro, rw nor mapped (TODO/enter.md T-2101)"
        )),
        (true, false) => Ok(Mode::Overlay),
        _ if mapped => Ok(Mode::Mapped { ro }),
        _ if ro => Ok(Mode::Ro),
        _ => Ok(Mode::Rw),
    }
}
