//! The environment variables that a user sets to change what podvm does
//! (TODO/cli.md T-2001 OP-01). `podvm man` prints [`USER`] as its ENVIRONMENT
//! section, and the same table gives the facts of `man --json`, `--roff` and
//! `--llms`.
//!
//! ⛔ Each `PODVM_*` name that a source reads has exactly one home: [`USER`]
//! (a user sets it), [`INTERNAL`] (podvm sets it for its own processes) or
//! [`DEV`] (a test, a measure, a build or a developer tool uses it). The unit
//! test scans the sources, so a new name with no home fails it.

/// One variable that a user sets.
pub struct Var {
    pub name: &'static str,
    /// The values that the code takes, or a dash where any value works.
    pub values: &'static str,
    pub effect: &'static str,
}

/// A group of [`USER`] rows.
pub struct Group {
    pub name: &'static str,
    pub vars: &'static [Var],
}

const fn v(name: &'static str, values: &'static str, effect: &'static str) -> Var {
    Var {
        name,
        values,
        effect,
    }
}

/// Each variable that a user sets, in groups.
pub const USER: &[Group] = &[
    Group {
        name: "Store, images and registries",
        vars: &[
            v(
                "PODVM_STORE",
                "a directory",
                "Names the store of images and containers. With no value, podvm uses the XDG data \
                 directory, then $HOME/.local/share/podvm. The flag --store sets it too.",
            ),
            v(
                "PODVM_CONFIG",
                "a file",
                "Names the file of insecure registries, one host for each line, with # comments. \
                 With no value, podvm reads $XDG_CONFIG_HOME/podvm/registries.conf.",
            ),
            v(
                "PODVM_INSECURE_REGISTRIES",
                "hosts",
                "Lists registries that podvm reaches without TLS verification, as docker's \
                 --insecure-registry does.",
            ),
            v(
                "PODVM_DEFAULT_PLATFORM",
                "os/arch[/variant]",
                "Sets the platform of an image when no --platform flag is present. \
                 DOCKER_DEFAULT_PLATFORM has the same effect, and this variable wins.",
            ),
            v(
                "PODVM_SPARE_TREE",
                "0",
                "0 turns off the spare tree and the deferred removal of a container tree. Any \
                 other value, or no value, leaves them on.",
            ),
            v(
                "PODVM_PULL_PHASES",
                "any value",
                "Makes podvm pull print the time of its phases on stderr.",
            ),
        ],
    },
    Group {
        name: "Launch of a foreground run",
        vars: &[
            v(
                "PODVM_USERNS",
                "0",
                "0 stops a foreground run of a caller that is not uid 0 from opening the user, \
                 PID and UTS namespace of podvm (call C21 of docs/decisions.md). A run that has a \
                 volume (-v, --tmpfs or --read-only) then gets the userland loader family. A run \
                 with no volume gets the namespace rung of pod-enter, which opens a user and a \
                 mount namespace itself. Any other value leaves the entry on. The --verbose \
                 banner then shows mode=namespace beside probe rows of the host.",
            ),
            v(
                "PODVM_USERNS_RANGE",
                "1",
                "1 asks for the subordinate id range of the caller (/etc/subuid and /etc/subgid) \
                 in the user namespace, so that -u UID drops for real. It needs newuidmap and \
                 newgidmap, and a range for the caller. Any other value leaves it off.",
            ),
            v(
                "PODVM_MODE",
                "memfd, fuse, tmpfs, rundir, cache",
                "Forces the launch rung of a foreground run. A refusal names the rung when the \
                 host does not allow it. Detached run, create, exec and the machine tier refuse \
                 a value.",
            ),
            v(
                "PODVM_CACHE",
                "1, true",
                "Lets a foreground run use the cache rung. Any other value leaves it out.",
            ),
            v(
                "PODVM_RUNSC",
                "a program",
                "Names the runsc program of the gVisor tier. With no value, podvm uses runsc on \
                 PATH.",
            ),
            v(
                "PODVM_RUNSC_ARGS",
                "flags with spaces between them",
                "Adds flags of runsc after the flags of the tier and before the verb. Each word \
                 must be a flag (-name or --name). A flag of the same name wins over the flag of \
                 the tier.",
            ),
        ],
    },
    Group {
        name: "Diagnosis",
        vars: &[
            v(
                "PODVM_DEBUG",
                "any value",
                "Makes run say on stderr whether its probe ran now or came from the probe cache.",
            ),
            v(
                "PODVM_INTERPOSE_DEBUG",
                "1, or any value but 0",
                "Turns on the per-call notes of the interposer in a payload. Set it with -e. A \
                 refusal of the interposer prints without it.",
            ),
            v(
                "PODVM_EXCLUDE_PATH",
                "path prefixes with : between them",
                "Adds prefixes that the interposer does not rewrite to the built-in list. Set it \
                 with -e.",
            ),
        ],
    },
    Group {
        name: "Machine tier and emulator",
        vars: &[
            v(
                "PODVM_MACHINE_ACCEL",
                "kvm, tcg",
                "Selects the accelerator of a guest (call C16). With no value, a guest runs under \
                 KVM where /dev/kvm opens and under TCG elsewhere. kvm with a closed /dev/kvm \
                 exits 125 and boots nothing. For a Windows install, only tcg allows TCG.",
            ),
            v(
                "PODVM_MACHINE_KERNEL",
                "a file",
                "Names a kernel for the guest in place of the pinned kernel. The first use is \
                 recorded, and a change of the file is refused. --podvm-engine refuses it.",
            ),
            v(
                "PODVM_MACHINE_KERNEL_ARM64",
                "a file",
                "Names the kernel of a linux/arm64 guest.",
            ),
            v(
                "PODVM_MACHINE_SSH_LOG",
                "an ssh LogLevel, such as DEBUG1",
                "Puts the diagnosis of the ssh client of a machine on stderr.",
            ),
            v(
                "PODVM_QEMU",
                "a program",
                "Names the qemu-system-x86_64 program. With no value, podvm uses PATH, then the \
                 copy in the store.",
            ),
            v(
                "PODVM_QEMU_IMG",
                "a program",
                "Names the qemu-img program, with the same search as PODVM_QEMU.",
            ),
            v(
                "PODVM_QEMU_SHARE",
                "a directory",
                "Names the directory of the firmware and option ROMs of the emulator.",
            ),
            v(
                "PODVM_OVMF_CODE",
                "a file",
                "Names the UEFI code image of a Windows guest.",
            ),
            v(
                "PODVM_OVMF_VARS",
                "a file",
                "Names the UEFI variable store of a Windows guest.",
            ),
            v(
                "PODVM_INCUS_SOCKET",
                "a socket path",
                "Names the socket of incusd for the incus tier. With no value, podvm uses \
                 $INCUS_DIR/unix.socket, then the socket of the system daemon.",
            ),
        ],
    },
    Group {
        name: "Windows guest",
        vars: &[
            v(
                "PODVM_WINDOWS_BASE",
                "a directory",
                "Names the cache of the base image of a Windows guest. With no value, podvm uses \
                 a directory under $HOME.",
            ),
            v(
                "PODVM_DOS_BASE",
                "a directory",
                "Names the cache of the base image of a DOS guest, apart from the Windows cache.",
            ),
            v(
                "PODVM_MACHINES_DIR",
                "a directory",
                "Names the directory of all machines. With no value, it is machines beside the \
                 catalog.",
            ),
            v(
                "PODVM_TOOLS_DIR",
                "a directory",
                "Names the directory of the copy of the emulator in the store.",
            ),
            v(
                "PODVM_WINDOWS_KEEP",
                "any value",
                "Keeps the per-run directory of a Windows run or install, which podvm removes \
                 otherwise.",
            ),
            v(
                "PODVM_WINDOWS_KEEP_FAILED",
                "a directory",
                "Copies the logs of WinPE to this directory when an install fails.",
            ),
            v(
                "PODVM_WINDOWS_SHOTS",
                "a directory",
                "Writes a screen dump of the guest to this directory during an install.",
            ),
            v(
                "PODVM_WINDOWS_HYPERV",
                "0",
                "0 drops the Hyper-V flags of an install, to tell a stall of nested KVM from a \
                 stall of the emulated disk.",
            ),
            v(
                "PODVM_WINDOWS_INSTALL_DEADLINE",
                "seconds",
                "Bounds the time of an install.",
            ),
            v(
                "PODVM_WINDOWS_INSTALL_MEM",
                "a size, such as 8G",
                "Sets the memory of the guest of an install.",
            ),
            v(
                "PODVM_WINDOWS_INSTALL_CPUS",
                "a number",
                "Sets the CPU count of the guest of an install.",
            ),
            v(
                "PODVM_WINDOWS_ISO_BUS",
                "usb",
                "usb puts the install ISO on a USB bus. It is a diagnosis only.",
            ),
            v(
                "PODVM_WINDOWS_ROOT_BUS",
                "sata",
                "sata puts the root disk of an install on SATA. It is a diagnosis only: Windows \
                 installed on SATA does not boot from NVMe.",
            ),
            v(
                "PODVM_WINDOWS_MAILBOX",
                "any value",
                "Sends the command of a named Windows machine through the mailbox, not through \
                 the guest agent.",
            ),
        ],
    },
];

/// The variables that podvm sets or reads for its own processes: the hand-off
/// between the entry, the supervisor, the interposer and the payload. A user
/// does not set them, and a value of a user can break a run.
#[cfg_attr(not(test), allow(dead_code))] // read by the unit test only
pub const INTERNAL: &[&str] = &[
    "PODVM_ACTIVE_MODE",
    "PODVM_BINDS",
    "PODVM_BINDS_BASE",
    "PODVM_DEVICE_FDS",
    "PODVM_DEVICE_SPEC",
    "PODVM_DISPATCH_BROKER",
    "PODVM_DISPATCH_EMU",
    "PODVM_DISPATCH_EXE",
    "PODVM_DISPATCH_ROOT",
    "PODVM_DISPATCH_SELF",
    "PODVM_DISPATCH_TIER",
    "PODVM_EXEC_HOST",
    "PODVM_GUEST_EXE",
    "PODVM_GUEST_EXE_FORCE",
    "PODVM_HOSTNAME",
    "PODVM_IDENTITY",
    "PODVM_IDENTITY_SAID",
    "PODVM_INIT",
    "PODVM_INIT_DISCARD",
    "PODVM_INIT_LOG",
    "PODVM_LIB_PATH",
    "PODVM_MACHINE_KEEPER",
    "PODVM_MAPS",
    "PODVM_MEMO_FD",
    "PODVM_MEMO_ID",
    "PODVM_MEMO_PATH",
    "PODVM_RO",
    "PODVM_SANDPROBE_HOST",
    "PODVM_USERNS_IDS",
    "PODVM_USERNS_JOIN",
    // `--podvm-volume-base` for the handler and the interposer (T-1972).
    "PODVM_VOLUME_BASE",
    // The write table of `--podvm-confine` (TODO/enter.md T-1985).
    "PODVM_WRITE_HOST",
];

/// The variables of tests, measures, builds and developer tools. No run of a
/// released binary reads them for a user.
#[cfg_attr(not(test), allow(dead_code))] // read by the unit test only
pub const DEV: &[&str] = &[
    "PODVM_154_CURL_TIMEOUT",
    "PODVM_154_TIMEOUT",
    "PODVM_ARTIFACTS",
    "PODVM_BIN",
    "PODVM_BLESS",
    "PODVM_BUILD_COMMIT",
    "PODVM_BUILD_CRT_STATIC",
    "PODVM_BUILD_RUSTC",
    "PODVM_BUILD_TARGET",
    "PODVM_CHECK_STEP_TIMEOUT",
    "PODVM_DEV_LOG",
    "PODVM_DEV_ROOT",
    "PODVM_DEV_TARGET",
    "PODVM_DEV_WAIT",
    "PODVM_DL_STUB",
    "PODVM_INTERPOSE_GNU",
    "PODVM_INTERPOSE_GNU234",
    "PODVM_INTERPOSE_GNU_MACHINE",
    "PODVM_INTERPOSE_MUSL",
    "PODVM_INTERPOSE_MUSL_MACHINE",
    "PODVM_LICENSES",
    "PODVM_PLANT_TRANSCRIPT",
    "PODVM_PROBE_ALT",
    "PODVM_PROVE_RUNS",
    "PODVM_SWEEP",
    "PODVM_T0215_CAPTURE",
    "PODVM_T1756",
    "PODVM_TARGET",
    "PODVM_TEST_MODLOOP",
    "PODVM_WORKLOAD_MIRROR",
];

/// The ENVIRONMENT section of the text manual.
pub fn section() -> String {
    let mut out = String::from(
        "ENVIRONMENT\n\
         A variable changes a run only when it is in the environment of podvm. \
         podvm sets other PODVM_* variables for its own processes; do not set them.\n\n",
    );
    for g in USER {
        out.push_str(&format!("  {}\n", g.name));
        for var in g.vars {
            out.push_str(&format!(
                "    {} ({})\n      {}\n",
                var.name, var.values, var.effect
            ));
        }
        out.push('\n');
    }
    out
}

/// The same table as the typed facts of `pod-manual`.
pub fn facts() -> pod_manual::facts::Facts {
    use pod_manual::facts::{Facts, Var as Fact, VarGroup};
    Facts {
        variables: USER
            .iter()
            .map(|g| VarGroup {
                name: g.name.to_string(),
                vars: g
                    .vars
                    .iter()
                    .map(|var| {
                        Fact::reads(
                            &[var.name],
                            &format!("Values: {}. {}", var.values, var.effect),
                        )
                    })
                    .collect(),
            })
            .collect(),
        ..Facts::default()
    }
}

#[cfg(test)]
mod tests;
