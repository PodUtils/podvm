//! The run archive of a guest: `/podvm-init`, the console nodes, the
//! copies of `-v` where no 9p module serves them, and podvm itself with the
//! modules that the guest loads (TODO/machine.md T-1656, T-1723).

use std::path::{Path, PathBuf};

use super::cpio;

/// podvm's own bytes and the modules of the network, for a guest
/// ([`net`]).
pub struct Tools {
    pub podvm: Vec<u8>,
    pub modules: Vec<(String, Vec<u8>)>,
    /// The modules of `--podvm-engine`, in their order (T-1780): the init
    /// loads the names of [`ENGINE_LIST`].
    pub engine: Vec<String>,
}

/// The names of the modules of `--podvm-engine`, one on a line.
pub const ENGINE_LIST: &str = "/podvm/engine.list";

/// The archive of one run: `/podvm-init`, the console nodes, and, where
/// `tools` gives them, podvm itself and the modules of the network.
pub fn run_archive(
    script: &str,
    tools: Option<&Tools>,
    copies: &[(PathBuf, String)],
    stdin: Option<&[u8]>,
) -> std::io::Result<Vec<u8>> {
    let mut w = cpio::Newc::new(Vec::new());
    for (host, guest) in copies {
        w.tree_at(host, guest)?;
    }
    if let Some(bytes) = stdin {
        w.entry("podvm", cpio::DIR | 0o755, &[], (0, 0))?;
        w.entry("podvm/stdin", cpio::REG | 0o600, bytes, (0, 0))?;
    }
    w.entry("dev", cpio::DIR | 0o755, &[], (0, 0))?;
    w.entry("dev/console", cpio::CHR | 0o600, &[], (5, 1))?;
    w.entry("dev/ttyS0", cpio::CHR | 0o660, &[], (4, 64))?;
    if let Some(t) = tools {
        w.entry("podvm", cpio::DIR | 0o755, &[], (0, 0))?;
        w.entry("podvm/podvm", cpio::REG | 0o755, &t.podvm, (0, 0))?;
        w.entry("podvm/modules", cpio::DIR | 0o755, &[], (0, 0))?;
        for (n, bytes) in &t.modules {
            w.entry(
                &format!("podvm/modules/{n}.ko"),
                cpio::REG | 0o644,
                bytes,
                (0, 0),
            )?;
        }
        if !t.engine.is_empty() {
            let list = t.engine.join("\n") + "\n";
            w.entry(
                ENGINE_LIST.trim_start_matches('/'),
                cpio::REG | 0o644,
                list.as_bytes(),
                (0, 0),
            )?;
        }
    }
    w.entry("podvm-init", cpio::REG | 0o755, script.as_bytes(), (0, 0))?;
    w.finish()
}

/// podvm's own bytes and the module files, for the run archive.
pub(super) fn tools(
    modules: &[(String, PathBuf)],
    engine: Vec<String>,
) -> Result<Option<Tools>, String> {
    if modules.is_empty() {
        return Ok(None);
    }
    let podvm = std::fs::read("/proc/self/exe").map_err(|e| format!("/proc/self/exe: {e}"))?;
    let mut m = Vec::new();
    for (n, p) in modules {
        m.push((
            n.clone(),
            std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?,
        ));
    }
    Ok(Some(Tools {
        podvm,
        modules: m,
        engine,
    }))
}

/// The cache file of the base archive of `digest`.
pub fn base_path(store_root: &Path, digest: &str) -> PathBuf {
    let hex = digest.trim_start_matches("sha256:");
    store_root
        .join("machine")
        .join(format!("base-{hex}-v{}.cpio", super::BASE_FORMAT))
}

/// The base archive of the rootfs, from the cache or built now. `true`
/// where the cache answered.
pub fn base_archive(
    store_root: &Path,
    digest: &str,
    rootfs: &Path,
) -> Result<(PathBuf, bool), String> {
    let path = base_path(store_root, digest);
    if path.is_file() {
        return Ok((path, true));
    }
    let dir = path.parent().expect("a file under machine/");
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    // A name of its own: a pid is not unique across PID namespaces.
    let tmp = path.with_extension(format!("partial-{}", super::init::nonce()?));
    let file = std::fs::File::create(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let mut w = cpio::Newc::new(std::io::BufWriter::new(file));
    // TODO/machine.md T-1776: the owners of the image, from the sidecar
    // beside the rootfs; with no sidecar each file stays root's.
    if let Some(dir) = rootfs.parent() {
        let _ = w.owners_from(&dir.join(".meta.jsonl"));
    }
    let built = w
        .tree(rootfs)
        .and_then(|()| w.finish())
        .and_then(|mut b| std::io::Write::flush(&mut b));
    if let Err(e) = built {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("the base archive of {}: {e}", rootfs.display()));
    }
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((path, false))
}

/// Why the run `r` cannot boot a guest with no podvm in it (the kernel of
/// `PODVM_MACHINE_KERNEL` brings no module set), or `None`.
pub(super) fn needs_tools(r: &super::Request, init_image: bool) -> Option<String> {
    let (what, entry) = if r.sealed.is_some() {
        ("--podvm-sealed", "TODO/machine.md T-1751")
    } else if init_image {
        ("--podvm-init=image", "TODO/enter.md T-2002")
    } else if r.tty {
        ("-t", "TODO/machine.md T-1723")
    } else {
        return None;
    };
    Some(format!(
        "{what} needs podvm in the guest, and the kernel of PODVM_MACHINE_KERNEL brings no \
         module set for it ({entry})"
    ))
}
