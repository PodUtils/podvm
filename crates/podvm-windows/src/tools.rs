//! TODO/machine.md T-1875: the emulator, `qemu-img` and the firmware that
//! podvm fetches into its store when the host has none.
//!
//! The store copy is the static QEMU 11.1.3 of the release `qemu-11.1.3`
//! of PodUtils/static-pkgs (the service `static-pkgs-qemu` of
//! `pod-services`, podcrates `v0.81.0`, T-1014): musl, static-pie, no
//! `PT_INTERP`. It holds the QEMU commits `2865bf1c` (the fix of the
//! use-after-free of `address_space_translate_for_iotlb`, a SIGSEGV at the
//! boot of a guest with four vCPUs under TCG, PodUtils/podvm#618) and
//! `9d378cab` (the migration of an NVMe device, so a warm start of a
//! Windows guest saves its root disk, T-1874). Each archive keeps the
//! installed layout (`./bin/NAME`, `./share/qemu/...`).
//! podvm checks the size and the sha256 of each archive before it reads a
//! byte of it, then the size and the sha256 of each member that it takes.
//!
//! The store keeps the layout of a relocatable install: `bin/` for the two
//! programs, `share/qemu/` for the data, so the emulator finds its firmware
//! with no `-L` (`bin/../share/qemu`).
//!
//! ⛔ The host QEMU wins where it is present: the store copy is the
//! fallback of a host with none (the Decision of T-1875).

use std::path::{Path, PathBuf};

/// The version of QEMU in the store.
pub const QEMU_VERSION: &str = "11.1.3";

/// The release of PodUtils/static-pkgs that holds [`QEMU_VERSION`].
pub const RELEASE: &str = "qemu-11.1.3";

/// Where one file of the store goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dest {
    /// `bin/NAME`, mode 0755.
    Bin,
    /// `share/qemu/NAME`.
    Share,
}

/// One download: the noVNC package (T-1899).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tool {
    /// The name of the download.
    pub name: &'static str,
    pub url: &'static str,
    /// The size of the download in bytes.
    pub size: u64,
    /// The sha256 of the download, lower-case hex.
    pub sha256: &'static str,
    pub dest: Dest,
}

/// One file of the store: a member of an archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Member {
    /// The path of the member in the archive, as `tar` lists it.
    pub path: &'static str,
    /// The size of the member in bytes.
    pub size: u64,
    /// The sha256 of the member, lower-case hex.
    pub sha256: &'static str,
    pub dest: Dest,
}

/// One archive of the release, and the members that the store takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Archive {
    /// The name of the asset of the release.
    pub name: &'static str,
    pub url: &'static str,
    /// The size of the archive in bytes.
    pub size: u64,
    /// The sha256 of the archive, lower-case hex.
    pub sha256: &'static str,
    pub members: &'static [Member],
}

#[cfg(test)]
const RELEASES: &str = "https://github.com/PodUtils/static-pkgs/releases/download/qemu-11.1.3/";

macro_rules! archive {
    ($name:literal, $size:literal, $sha:literal, $members:expr) => {
        Archive {
            name: $name,
            url: concat!(
                "https://github.com/PodUtils/static-pkgs/releases/download/qemu-11.1.3/",
                $name
            ),
            size: $size,
            sha256: $sha,
            members: $members,
        }
    };
}

macro_rules! share {
    ($name:literal, $size:literal, $sha:literal) => {
        Member {
            path: concat!("./share/qemu/", $name),
            size: $size,
            sha256: $sha,
            dest: Dest::Share,
        }
    };
}

/// The archives of the store, pinned 2026-11-21. The size and the sha256
/// of each archive, and the member, the size and the sha256 of each
/// program, are the facts of the service `static-pkgs-qemu` of
/// `pod-services` (a test of podvm compares them). The sha256 of each file
/// of the data were measured on the members of the data archive (the same
/// files as in the data of QEMU 11.0.0).
pub const ARCHIVES: &[Archive] = &[
    archive!(
        "qemu-system-x86_64-11.1.3-linux-amd64.tar.gz",
        6_814_799,
        "368da02758b8b474448a7a37fed601256ec9cd7e11c09656dbb85276e778c888",
        &[Member {
            path: "./bin/qemu-system-x86_64",
            size: 33_571_760,
            sha256: "5b5c3a2c181ae3381d1881d615ab923ec1ae35188d0274246ef14863b71c14d4",
            dest: Dest::Bin,
        }]
    ),
    archive!(
        "qemu-img-11.1.3-linux-amd64.tar.gz",
        1_430_921,
        "cffa14e62afc745f5487beb70b5caa6a4814853f5bba92a03e3eeddab53c43a5",
        &[Member {
            path: "./bin/qemu-img",
            size: 3_209_680,
            sha256: "e88a84357dd7f86d2f6dd7159deff4a929c6d9f030b691f378d9a59c0b7cc0ea",
            dest: Dest::Bin,
        }]
    ),
    archive!(
        "qemu-data-11.1.3-linux-amd64.tar.gz",
        18_171_081,
        "b11bff496ff7e345c9e4931306ccc3074e55d5eeb62d4a25f9efb70a3500f66e",
        &[
            share!(
                "bios-256k.bin",
                262_144,
                "ae6f6aa973aaccc143f57aa960fb035fd9de4daee4ad0cd713322f8c259e7650"
            ),
            share!(
                "bios-microvm.bin",
                131_072,
                "d226177d125aaf1b91e507738f4e9a0ade9474169b211751f78be08008cb394c"
            ),
            share!(
                "qboot.rom",
                65_536,
                "9b9dfc6c25740d6225625570d71cab6805cc9216e68c8932e343266daaeb8c4b"
            ),
            share!(
                "kvmvapic.bin",
                9_216,
                "cdf057a71b07e3b52b19cbe210bdefa59250d01a9810b960f7fe1f98eed95a27"
            ),
            share!(
                "linuxboot_dma.bin",
                1_536,
                "9c49e255340c78fc12e54ed043462bca02fb7fca29b7cfab62ff88a5344b6950"
            ),
            share!(
                "multiboot_dma.bin",
                1_024,
                "46def003b1c413cc4a5880d858ca54456ad6978d5fdb6e1b7d086b2905a66df0"
            ),
            share!(
                "pvh.bin",
                1_536,
                "918ff03a118615c2f4611a5df2d9496aabe099b5cfaaf69e544ebf973305d59d"
            ),
            share!(
                "vgabios-stdvga.bin",
                39_424,
                "e8fc9e55790dbe3cb31f019a3deb57206ba6c54f5e581adb2ab2677a9d391472"
            ),
            share!(
                "vgabios-virtio.bin",
                39_424,
                "02853140c262001ecb7245b151ffe47a0f0c79767a6fb2a9eb60f5f1938c5cee"
            ),
            share!(
                "vgabios-cirrus.bin",
                39_424,
                "10ecbac600620eb589f0c6d98f3f32a073d745c8a7f390b1704155d46a6b59d7"
            ),
            share!(
                "vgabios-bochs-display.bin",
                28_672,
                "da07e26661279edb7935284a99f8ddfa77dbf04ddb939e4a8c7bc7e36e40cdc7"
            ),
            share!(
                "vgabios-ramfb.bin",
                28_672,
                "24f6b98b2c263f3f562b1180df23e949f7dd27f194fa26aeba27f36bbf6265c6"
            ),
            share!(
                "efi-e1000e.rom",
                159_232,
                "102c20348be2d0ebe45a77b076b31848182c2f7b4364775b6bc2ece6e97b4024"
            ),
            share!(
                "efi-e1000.rom",
                159_232,
                "f37222fcdf0481bfbaf7a16d9aaf740aeb41d3d24c187801eb6e9a628cec0fc8"
            ),
            share!(
                "efi-virtio.rom",
                160_768,
                "26be36901db7f8181c306cc62bd74891d8646528965a78e40cceadba5dd7c8e7"
            ),
            share!(
                "edk2-x86_64-code.fd",
                3_653_632,
                "33090cc07675baa5190d9f1e84bf5176b33bcbfa9bacac522961150cdb6dbb2a"
            ),
            share!(
                "edk2-i386-vars.fd",
                540_672,
                "5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e"
            ),
        ]
    ),
];

/// TODO/machine.md T-1899: the RFB client of the screen viewer, noVNC
/// 1.7.0 as npm publishes it (`@novnc/novnc`: `core/` and `vendor/pako`,
/// ES modules, MPL-2.0 and MIT). `podvm windows view` fetches it into
/// `tools/novnc-1.7.0` of the store and serves it on the loopback.
pub const NOVNC: Tool = Tool {
    name: "novnc-1.7.0.tgz",
    url: "https://registry.npmjs.org/@novnc/novnc/-/novnc-1.7.0.tgz",
    size: 155_185,
    sha256: "32689f18d6abe96bc6530828a6bd0b9ae33bda07c083a6575ed255b5a8f2e903",
    dest: Dest::Share,
};

/// The directory of the noVNC copy, beside the QEMU copy.
pub fn novnc_root() -> PathBuf {
    root()
        .parent()
        .unwrap_or(Path::new("."))
        .join("novnc-1.7.0")
}

/// The directory of the store copy: `PODVM_TOOLS_DIR` where it is set,
/// else `tools/qemu-VERSION` beside the Windows catalog.
pub fn root() -> PathBuf {
    if let Some(p) = std::env::var_os("PODVM_TOOLS_DIR")
        && !p.is_empty()
    {
        return PathBuf::from(p);
    }
    let base = crate::base_cache();
    let windows = base.parent().unwrap_or(Path::new("."));
    let data = windows.parent().unwrap_or(Path::new("."));
    data.join("tools").join(format!("qemu-{QEMU_VERSION}"))
}

impl Member {
    /// The name of the file: the last component of its path.
    pub fn name(&self) -> &'static str {
        self.path.rsplit('/').next().unwrap_or(self.path)
    }

    /// The file that the member becomes under `root`.
    pub fn path_in(&self, root: &Path) -> PathBuf {
        match self.dest {
            Dest::Bin => bin(root, self.name()),
            Dest::Share => share(root).join(self.name()),
        }
    }
}

/// Each member of [`ARCHIVES`].
pub fn members() -> impl Iterator<Item = &'static Member> {
    ARCHIVES.iter().flat_map(|a| a.members.iter())
}

/// The program `name` of the store under `root`.
pub fn bin(root: &Path, name: &str) -> PathBuf {
    root.join("bin").join(name)
}

/// The data directory of the store under `root`.
pub fn share(root: &Path) -> PathBuf {
    root.join("share/qemu")
}

/// The UEFI pair of the store under `root`: `(code, vars)`.
pub fn firmware(root: &Path) -> (PathBuf, PathBuf) {
    let s = share(root);
    (s.join("edk2-x86_64-code.fd"), s.join("edk2-i386-vars.fd"))
}

/// True when each member of the table is in the store under `root`.
pub fn complete(root: &Path) -> bool {
    members().all(|m| m.path_in(root).is_file())
}

/// The bytes of all downloads.
pub fn total_bytes() -> u64 {
    ARCHIVES.iter().map(|a| a.size).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex64(h: &str) -> bool {
        h.len() == 64
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }

    /// Each pin is a size and 64 hex digits, each name is unique, each
    /// URL is an asset of the one release, and the store layout gives the
    /// emulator, `qemu-img`, the data and the UEFI pair.
    #[test]
    fn tools_pins_are_complete_and_the_layout_is_relocatable() {
        let mut names = std::collections::BTreeSet::new();
        for a in ARCHIVES {
            assert!(a.size > 0 && hex64(a.sha256), "{}", a.name);
            assert!(a.url.starts_with(RELEASES), "{}", a.url);
            assert!(a.url.ends_with(a.name) && a.name.ends_with(".tar.gz"));
            assert!(a.name.contains(QEMU_VERSION), "{}", a.name);
            assert!(!a.members.is_empty(), "{}", a.name);
            for m in a.members {
                assert!(names.insert(m.name()), "{} twice", m.name());
                assert!(m.size > 0 && hex64(m.sha256), "{}", m.path);
                let dir = match m.dest {
                    Dest::Bin => "./bin/",
                    Dest::Share => "./share/qemu/",
                };
                assert_eq!(m.path, format!("{dir}{}", m.name()), "{}", m.path);
            }
        }
        let root = Path::new("/store/tools/qemu-11.1.3");
        let paths: Vec<PathBuf> = members().map(|m| m.path_in(root)).collect();
        for want in [
            bin(root, "qemu-system-x86_64"),
            bin(root, "qemu-img"),
            share(root).join("bios-256k.bin"),
            firmware(root).0,
            firmware(root).1,
        ] {
            assert!(
                paths.contains(&want),
                "{} is not in the store",
                want.display()
            );
        }
        assert_eq!(share(root).parent().unwrap(), root.join("share"));
        assert!(total_bytes() < 64 << 20, "{}", total_bytes());
        assert!(!complete(Path::new("/nonexistent-podvm-tools")));
    }
}
