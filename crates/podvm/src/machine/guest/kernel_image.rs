//! TODO/machine.md T-1994, part 2 (PodUtils/podrunner#316 ask 2): a guest
//! of the machine tier boots the kernel of its image in place of the
//! pinned kernel.
//!
//! `PODVM_MACHINE_IMAGE_KERNEL=1` takes the one kernel of the image, and
//! `PODVM_MACHINE_IMAGE_KERNEL=RELEASE` takes the kernel of that release:
//! `/boot/vmlinuz-RELEASE`, or `/usr/lib/modules/RELEASE/vmlinuz` where an
//! image keeps it there. The initrd of the guest is podvm's own: podvm and
//! the modules that the run needs, out of `/lib/modules/RELEASE` of the
//! image, each with each module that its `modules.dep` names for it. A
//! name of `modules.builtin`, or `=y` in the config of the release, needs
//! no file. `podvm guest-root` loads them, mounts the root disk of the
//! image, and execs the init of the run or the init of the image, as on
//! the pinned kernel. A compressed module (`.ko.gz`, `.ko.xz`, `.ko.zst`)
//! is decompressed on the host, so the kernel needs no decompressor.
//!
//! The initrd of the image does not serve: the root of a guest is the
//! read-only disk of the image under an overlay that `guest-root` makes,
//! and an initrd of a distribution mounts `root=` itself and execs the
//! init with no unit of the run.
//!
//! A driver that the run needs and the kernel does not have is a refusal
//! by name before the boot (exit 125). A module that the kernel does not
//! load is one line of `guest-root` in the guest, and exit 125.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::kernel::Assets;
use super::pid1::resolve;

mod files;
#[cfg(test)]
use files::read_module_within;
use files::{check_bzimage, kernel_file, text};
pub use files::{initrd_file, pick, read_module, releases};

/// The variable of the choice.
pub const VAR: &str = "PODVM_MACHINE_IMAGE_KERNEL";
/// The file that tells `guest-net` and `guest-9p` that `guest-root`
/// loaded each module of the run: they load no module of the pin.
pub const MARK: &str = "/podvm/modules/image-kernel";
/// The suffixes of a module file, plain or compressed.
pub const SUFFIXES: [&str; 4] = [".ko", ".ko.gz", ".ko.xz", ".ko.zst"];

/// Which kernel of the image the variable asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Want {
    /// `1`: the one kernel of the image.
    One,
    /// The kernel of a release.
    Release(String),
}

/// One part of a path, in printable ASCII.
fn is_release(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s != "."
        && s != ".."
        && s.bytes().all(|b| b.is_ascii_graphic() && b != b'/')
}

/// The value of [`VAR`]: none for the pinned kernel. `other_kernel`:
/// `PODVM_MACHINE_KERNEL` names a kernel too.
pub fn wanted(var: Option<&str>, other_kernel: bool) -> Result<Option<Want>, String> {
    let want = match var {
        None | Some("") | Some("0") => return Ok(None),
        Some("1") => Want::One,
        Some(r) if is_release(r) => Want::Release(r.to_string()),
        Some(r) => {
            return Err(format!(
                "{VAR}={r:?}: the values are 0, 1 and the release of a kernel of the image"
            ))
        }
    };
    if other_kernel {
        return Err(format!(
            "{VAR} and PODVM_MACHINE_KERNEL each name a kernel of the guest: set one"
        ));
    }
    Ok(Some(want))
}

/// One group of drivers that a run needs: its words for a refusal, the
/// names that must be there, and the names that load only where they are
/// modules.
pub struct Group {
    pub what: &'static str,
    pub need: &'static [&'static str],
    pub optional: &'static [&'static str],
}

/// The root disk of each guest of this route, and the sized disk. ext4
/// asks for the algorithm `crc32c` at its mount, and `modules.dep` does
/// not name it (`modules.softdep`), so `crc32c_generic` loads first where
/// it is a module (as on the pin, [`super::disk::MODULES`]).
pub const ROOT: Group = Group {
    what: "the root disk of the image",
    need: &["virtio_pci", "virtio_blk", "ext4", "overlay"],
    optional: &["crc32c_generic"],
};
/// The NIC of the guest.
pub const NET: Group = Group {
    what: "the network (--podvm-net none runs with no NIC)",
    need: &["virtio_net"],
    optional: &[],
};
/// A `-v` volume.
pub const NINE: Group = Group {
    what: "a -v volume (9p)",
    need: &["9pnet_virtio", "9p"],
    optional: &[],
};
/// `-t`, `run -d`, `--podvm-channel` and `--podvm-sealed`: a port of a
/// virtio console.
pub const CONSOLE: Group = Group {
    what: "-t, run -d, --podvm-channel and --podvm-sealed (a virtio console)",
    need: &["virtio_console"],
    optional: &[],
};

/// The config symbol of a driver, for the words of a refusal.
fn symbol(name: &str) -> Option<&'static str> {
    Some(match name {
        "virtio_pci" => "VIRTIO_PCI",
        "virtio_blk" => "VIRTIO_BLK",
        "ext4" => "EXT4_FS",
        "overlay" => "OVERLAY_FS",
        "virtio_net" => "VIRTIO_NET",
        "9pnet_virtio" => "NET_9P_VIRTIO",
        "9p" => "9P_FS",
        "virtio_console" => "VIRTIO_CONSOLE",
        "8250" => "SERIAL_8250",
        _ => return None,
    })
}

/// What podvm knows of the drivers of one release of the image.
pub struct Drivers {
    pub release: String,
    /// The text of `modules.dep`.
    dep: String,
    /// The name of each module of `modules.dep`.
    modules: std::collections::HashSet<String>,
    /// The name of each line of `modules.builtin`.
    builtin: std::collections::HashSet<String>,
    /// The config of the release (`CONFIG_X` without its prefix), where
    /// the image has it.
    config: Option<HashMap<String, String>>,
}

/// Where a driver is.
#[derive(Debug, PartialEq, Eq)]
enum Have {
    Module,
    Builtin,
    Missing,
}

impl Drivers {
    /// The drivers of `release` from the text of its `modules.dep`, its
    /// `modules.builtin` and its config.
    pub fn parse(release: &str, dep: &str, builtin: &str, config: Option<&str>) -> Drivers {
        let name = super::engine::name_of;
        let config = config.map(|c| {
            let mut m = HashMap::new();
            for l in c.lines() {
                if let Some((k, v)) = l.strip_prefix("CONFIG_").and_then(|l| l.split_once('=')) {
                    m.insert(k.to_string(), v.to_string());
                } else if let Some(k) = l
                    .strip_prefix("# CONFIG_")
                    .and_then(|l| l.strip_suffix(" is not set"))
                {
                    m.insert(k.to_string(), "n".to_string());
                }
            }
            m
        });
        Drivers {
            release: release.to_string(),
            dep: dep.to_string(),
            modules: dep
                .lines()
                .filter_map(|l| l.split_once(':'))
                .map(|(p, _)| name(p.trim()))
                .collect(),
            builtin: builtin
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(name)
                .collect(),
            config,
        }
    }

    fn config(&self, name: &str) -> Option<&str> {
        let c = self.config.as_ref()?;
        Some(c.get(symbol(name)?).map_or("n", String::as_str))
    }

    fn have(&self, name: &str) -> Have {
        if self.modules.contains(name) {
            Have::Module
        } else if self.builtin.contains(name) || self.config(name) == Some("y") {
            Have::Builtin
        } else {
            Have::Missing
        }
    }

    /// The words of a missing driver: its name and what its config says.
    fn missing(&self, name: &str) -> String {
        match (symbol(name), self.config(name)) {
            (Some(s), Some("n")) => format!("{name} (CONFIG_{s} is not set)"),
            (Some(s), Some(v)) => format!("{name} (CONFIG_{s}={v})"),
            _ => name.to_string(),
        }
    }

    /// Why the kernel cannot run a guest of podvm at all, or `None`: the
    /// serial console of the markers and `devtmpfs` must be built in.
    /// With no config and no `modules.builtin`, only the boot can tell.
    pub fn base(&self) -> Option<String> {
        let mut no = Vec::new();
        match self.have("8250") {
            Have::Builtin => {}
            Have::Module => no.push("8250 built in (it is a module)".to_string()),
            Have::Missing if self.builtin.is_empty() && self.config.is_none() => {}
            Have::Missing => no.push(format!("{} built in", self.missing("8250"))),
        }
        for s in ["SERIAL_8250_CONSOLE", "DEVTMPFS", "BLK_DEV_INITRD"] {
            if let Some(c) = &self.config
                && c.get(s).map(String::as_str) != Some("y")
            {
                no.push(format!("CONFIG_{s}=y"));
            }
        }
        (!no.is_empty()).then(|| format!("the console of a guest of podvm needs {}", no.join(", ")))
    }

    /// The modules that `groups` need, each module that `modules.dep`
    /// names for them first, each once: `(name, path)`, the path under
    /// the directory of the release. Err names each missing driver with
    /// what needs it.
    pub fn plan(&self, groups: &[&Group]) -> Result<Vec<(String, String)>, String> {
        let mut top = Vec::new();
        let mut no = Vec::new();
        for g in groups {
            for n in g.optional {
                if self.have(n) == Have::Module {
                    top.push(*n);
                }
            }
            let lack: Vec<String> = g
                .need
                .iter()
                .filter(|n| match self.have(n) {
                    Have::Module => {
                        top.push(**n);
                        false
                    }
                    Have::Builtin => false,
                    Have::Missing => true,
                })
                .map(|n| self.missing(n))
                .collect();
            if !lack.is_empty() {
                no.push(format!("{} for {}", lack.join(" and "), g.what));
            }
        }
        if !no.is_empty() {
            return Err(format!(
                "the kernel {} of the image has no {}: not in its modules.dep, not in its \
                 modules.builtin",
                self.release,
                no.join("; no ")
            ));
        }
        let set = super::engine::closure(&self.dep, &top)?;
        for (n, p) in &set {
            let plain = !n.is_empty() && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
            let rel = !p.starts_with('/') && p.split('/').all(|s| !s.is_empty() && s != "..");
            if !plain || !rel {
                return Err(format!(
                    "modules.dep of {} names {p:?} ({n:?}), which is not a plain module path",
                    self.release
                ));
            }
        }
        Ok(set)
    }
}

/// The kernel of the image for a run: its file, its release and each
/// module of `groups` with the file of each in the image, in their order.
pub struct Own {
    pub release: String,
    pub load: Vec<(String, PathBuf)>,
}

/// The drivers of the kernel `release` of the image at `rootfs`, and the
/// directory of its modules in the image. TODO/machine.md T-1998: the
/// incus tier reads them too, so that the two tiers have one search.
pub fn drivers(rootfs: &Path, release: &str) -> Result<(Drivers, String), String> {
    let dirs = [
        format!("/lib/modules/{release}"),
        format!("/usr/lib/modules/{release}"),
    ];
    let dir = dirs
        .iter()
        .find(|d| resolve(rootfs, &format!("{d}/modules.dep")).is_some_and(|p| p.is_file()))
        .ok_or_else(|| {
            format!(
                "the image has no /lib/modules/{release}/modules.dep: the modules of its kernel \
                 need one (depmod {release} in the image)"
            )
        })?;
    let dep = text(rootfs, &[format!("{dir}/modules.dep")])?.unwrap_or_default();
    let builtin = text(rootfs, &[format!("{dir}/modules.builtin")])?.unwrap_or_default();
    let config = text(
        rootfs,
        &[format!("/boot/config-{release}"), format!("{dir}/config")],
    )?;
    let d = Drivers::parse(release, &dep, &builtin, config.as_deref());
    Ok((d, dir.clone()))
}

/// TODO/machine.md T-1998: the kernel of the image that [`VAR`] chooses
/// (the one kernel of the image where it is not set), its release, and
/// its file after the bzImage check.
pub fn chosen(rootfs: &Path) -> Result<(String, PathBuf), String> {
    let var = match std::env::var_os(VAR) {
        None => None,
        Some(v) => Some(v.into_string().map_err(|_| format!("{VAR} is not UTF-8"))?),
    };
    let want = wanted(var.as_deref(), false)?.unwrap_or(Want::One);
    let release = pick(&want, &releases(rootfs))?;
    let kernel = kernel_file(rootfs, &release)
        .ok_or_else(|| format!("the image has no kernel file of {release}"))?;
    check_bzimage(&kernel)?;
    Ok((release, kernel))
}

/// The kernel of `release` of the image at `rootfs`, for `groups`.
pub fn own(rootfs: &Path, release: &str, groups: &[&Group]) -> Result<(PathBuf, Own), String> {
    let kernel = kernel_file(rootfs, release)
        .ok_or_else(|| format!("the image has no kernel file of {release}"))?;
    check_bzimage(&kernel)?;
    let (d, dir) = drivers(rootfs, release)?;
    if let Some(why) = d.base() {
        return Err(format!("the kernel {release} of the image: {why}"));
    }
    let mut load = Vec::new();
    for (n, rel) in d.plan(groups)? {
        let p = resolve(rootfs, &format!("{dir}/{rel}"))
            .filter(|p| p.is_file())
            .ok_or_else(|| {
                format!("modules.dep of {release} names {rel}, and the image has no such file")
            })?;
        load.push((n, p));
    }
    Ok((
        kernel,
        Own {
            release: release.to_string(),
            load,
        },
    ))
}

/// The groups of drivers of the run `r`.
pub fn groups(r: &super::Request) -> Vec<&'static Group> {
    let mut g = vec![&ROOT];
    if !r.net_none {
        g.push(&NET);
    }
    if !r.volumes.is_empty() {
        g.push(&NINE);
    }
    if r.tty || r.keeper.is_some() || r.channel.is_some() || r.sealed.is_some() {
        g.push(&CONSOLE);
    }
    g
}

/// Why the run `r` cannot boot the kernel of its image, before a look at
/// the image, or `None`.
pub fn refusal(r: &super::Request, root_var: Option<&str>) -> Option<String> {
    let why = if r.arch != super::arch::Arch::Amd64 {
        "a guest of linux/arm64 (TODO/machine.md T-1890)"
    } else if r.engine {
        "--podvm-engine, whose modules come from the pinned kernel (TODO/machine.md T-1780)"
    } else if root_var == Some("initramfs") {
        "PODVM_MACHINE_ROOT=initramfs: the kernel of the image boots on the root disk of the \
         image only"
    } else {
        return None;
    };
    Some(format!("{VAR} does not serve {why}"))
}

/// The kernel and the modules of the run `r`: those of [`super::arch::kernel`],
/// or, where [`VAR`] asks for it, the kernel of the image with no module of
/// the pin and the [`Own`] modules of the run.
pub fn of_run(r: &super::Request) -> Result<(Assets, Option<Own>), String> {
    let var = match std::env::var_os(VAR) {
        None => None,
        Some(v) => Some(v.into_string().map_err(|_| format!("{VAR} is not UTF-8"))?),
    };
    let other = std::env::var_os("PODVM_MACHINE_KERNEL").is_some();
    let Some(want) = wanted(var.as_deref(), other)? else {
        return Ok((super::arch::kernel(r.store_root, r.arch)?, None));
    };
    let root_var = std::env::var(super::disk::ROOT_VAR).ok();
    if let Some(why) = refusal(r, root_var.as_deref()) {
        return Err(why);
    }
    let release = pick(&want, &releases(r.rootfs)).map_err(|e| format!("{VAR}: {e}"))?;
    let (kernel, own) = own(r.rootfs, &release, &groups(r)).map_err(|e| format!("{VAR}: {e}"))?;
    let assets = Assets {
        kernel,
        modules: Vec::new(),
        volume_modules: Vec::new(),
    };
    Ok((assets, Some(own)))
}

#[cfg(test)]
#[path = "kernel_image_tests.rs"]
mod tests;
