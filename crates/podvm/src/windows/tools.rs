//! TODO/machine.md T-1875: the fetch of the store copy of QEMU
//! (`podvm_windows::tools`) for a host with no emulator, no `qemu-img` or
//! no UEFI pair.
//!
//! ⛔ Each archive is checked against its size and sha256 before podvm
//! reads it (`podvm_windows::fetch::store`), and each member that the store
//! takes against its own size and sha256 as it lands ([`unpack`]). The
//! store is written into a directory beside it and renamed in one step: a
//! fetch that stops leaves no half store that a later run trusts.

use std::io::Read;
use std::path::{Path, PathBuf};

use podvm_image::error::EXIT_RUNTIME_ERROR;
use podvm_windows::tools::{self, Archive, Dest, Member};

/// The store copy under its root, fetched where it is not complete.
/// Prints one line before the fetch and one after, on stderr.
pub(crate) fn ensure(verb: &str) -> Result<PathBuf, i32> {
    let root = tools::root();
    if tools::complete(&root) {
        return Ok(root);
    }
    eprintln!(
        "podvm {verb}: this host has no QEMU; podvm fetches the static QEMU {} and its \
         firmware ({} MiB) into {} (TODO/machine.md T-1875)",
        tools::QEMU_VERSION,
        tools::total_bytes() >> 20,
        root.display()
    );
    let part = root.with_extension("part");
    let _ = std::fs::remove_dir_all(&part);
    let done = fetch_all(&part);
    if let Err(why) = done {
        let _ = std::fs::remove_dir_all(&part);
        eprintln!("podvm {verb}: the store copy of QEMU: {why}");
        return Err(EXIT_RUNTIME_ERROR);
    }
    let _ = std::fs::remove_dir_all(&root);
    std::fs::rename(&part, &root).map_err(|why| {
        eprintln!("podvm {verb}: {}: {why}", root.display());
        EXIT_RUNTIME_ERROR
    })?;
    eprintln!(
        "podvm {verb}: the store copy of QEMU {} is in {}",
        tools::QEMU_VERSION,
        root.display()
    );
    Ok(root)
}

/// TODO/machine.md T-1875: the emulator of the Linux guest of the machine
/// tier: the host one on PATH, else the store copy, fetched where it is
/// not there.
pub(crate) fn emulator_for(verb: &str) -> Result<PathBuf, i32> {
    if let Some(p) = crate::tier::which_qemu("qemu-system-x86_64") {
        return Ok(p);
    }
    let root = ensure(verb)?;
    Ok(tools::bin(&root, "qemu-system-x86_64"))
}

/// Each archive of the table into `part`: the download, its check, and
/// the members that the store takes. The download goes after its unpack.
fn fetch_all(part: &Path) -> Result<(), String> {
    let fsize = super::plan::ceiling().map_err(|_| "RLIMIT_FSIZE could not be read".to_string())?;
    let downloads = part.join(".downloads");
    std::fs::create_dir_all(&downloads).map_err(|e| format!("{}: {e}", downloads.display()))?;
    for a in tools::ARCHIVES {
        let download = downloads.join(a.name);
        let mut origin = super::origin::Http {
            url: a.url.to_string(),
        };
        let ceiling = podvm_windows::Ceiling {
            max_bytes: a.size,
            fsize,
        };
        podvm_windows::fetch::store(
            &mut origin,
            &download,
            Some(a.sha256),
            ceiling,
            super::origin::TRIES,
        )
        .map_err(|e| format!("{}: {e}", a.name))?;
        let f =
            std::fs::File::open(&download).map_err(|e| format!("{}: {e}", download.display()))?;
        unpack(a, f, part)?;
        std::fs::remove_file(&download).map_err(|e| format!("{}: {e}", download.display()))?;
    }
    std::fs::remove_dir(&downloads).map_err(|e| format!("{}: {e}", downloads.display()))
}

/// True where the path of a tar entry names the member path `want`: the
/// same components, with or without the leading `./`.
fn same_member(entry: &[u8], want: &str) -> bool {
    let trim = |p: &[u8]| -> Vec<u8> { p.strip_prefix(b"./").unwrap_or(p).to_vec() };
    trim(entry) == trim(want.as_bytes())
}

/// Each member of `a` from the tar.gz stream `input` into the store under
/// `root`: a regular file of the pinned size, written with no more bytes
/// than the pin, and then checked against the pinned sha256. A program gets
/// mode 0755. A member that is absent, twice, of another type, of another
/// size or of another sum is a refusal that names the archive and the
/// member; each other entry is skipped.
fn unpack(a: &Archive, input: impl Read, root: &Path) -> Result<(), String> {
    let mut got = vec![false; a.members.len()];
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(input));
    let entries = archive
        .entries()
        .map_err(|e| format!("{}: not a tar.gz archive: {e}", a.name))?;
    for e in entries {
        let mut e = e.map_err(|e| format!("{}: a damaged entry: {e}", a.name))?;
        let path = e.path_bytes().into_owned();
        let Some(i) = a.members.iter().position(|m| same_member(&path, m.path)) else {
            continue;
        };
        let m = &a.members[i];
        if got[i] {
            return Err(format!("{}: {} is in it twice", a.name, m.path));
        }
        if !e.header().entry_type().is_file() {
            return Err(format!("{}: {} is not a regular file", a.name, m.path));
        }
        let size = e
            .header()
            .size()
            .map_err(|e| format!("{}: {}: {e}", a.name, m.path))?;
        if size != m.size {
            return Err(format!(
                "{}: {} holds {size} bytes, and the pin is {} bytes",
                a.name, m.path, m.size
            ));
        }
        write_member(a, m, &mut e, root)?;
        got[i] = true;
    }
    match a.members.iter().zip(&got).find(|(_, g)| !**g) {
        Some((m, _)) => Err(format!("{}: it holds no {}", a.name, m.path)),
        None => Ok(()),
    }
}

/// The bytes of the member `m` from `input` into its file under `root`,
/// at most its pinned size, then the check of its sha256 and its mode. The
/// file goes again where the check fails.
fn write_member(a: &Archive, m: &Member, input: &mut dyn Read, root: &Path) -> Result<(), String> {
    use sha2::Digest;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    let dest = m.path_in(root);
    let dir = dest.parent().unwrap_or(root);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out = std::fs::File::create(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
    let mut h = sha2::Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut input = input.take(m.size);
    let mut n = 0u64;
    loop {
        let k = input
            .read(&mut buf)
            .map_err(|e| format!("{}: {}: {e}", a.name, m.path))?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
        out.write_all(&buf[..k])
            .map_err(|e| format!("{}: {e}", dest.display()))?;
        n += k as u64;
    }
    let sum: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if n != m.size || sum != m.sha256 {
        drop(out);
        let _ = std::fs::remove_file(&dest);
        return Err(format!(
            "{}: {} gave {n} bytes with the sha256 {sum}, and the pin is {} bytes with {}",
            a.name, m.path, m.size, m.sha256
        ));
    }
    let mode = if m.dest == Dest::Bin { 0o755 } else { 0o644 };
    std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(mode))
        .map_err(|e| format!("{}: {e}", dest.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tar.gz of `(path, bytes)` entries, as the release ships them.
    fn tgz(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut b = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        ));
        for (p, bytes) in entries {
            let mut h = tar::Header::new_gnu();
            h.set_size(bytes.len() as u64);
            h.set_mode(0o644);
            h.set_entry_type(tar::EntryType::Regular);
            b.append_data(&mut h, p, *bytes).unwrap();
        }
        b.into_inner().unwrap().finish().unwrap()
    }

    fn sum(b: &[u8]) -> String {
        use sha2::Digest;
        sha2::Sha256::digest(b)
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect()
    }

    /// One rule for each member of an archive: it lands with its pinned
    /// size and sum, a program with mode 0755; else the unpack refuses and
    /// names the archive and the member, and no file of a wrong member
    /// stays.
    #[test]
    fn an_archive_member_lands_only_with_its_pinned_size_and_sum() {
        use std::os::unix::fs::PermissionsExt;
        let prog: &[u8] = b"\x7fELF the emulator";
        let rom: &[u8] = b"the firmware";
        let members: &'static [Member] = Box::leak(Box::new([
            Member {
                path: "./bin/qemu-img",
                size: prog.len() as u64,
                sha256: Box::leak(sum(prog).into_boxed_str()),
                dest: Dest::Bin,
            },
            Member {
                path: "./share/qemu/bios.bin",
                size: rom.len() as u64,
                sha256: Box::leak(sum(rom).into_boxed_str()),
                dest: Dest::Share,
            },
        ]));
        let a = Archive {
            name: "t.tar.gz",
            url: "https://example.invalid/t.tar.gz",
            size: 1,
            sha256: "-",
            members,
        };
        let other: &[u8] = b"another emulator!!";
        // (the entries, the words of the refusal, or None where it lands)
        type Entries<'a> = Vec<(&'a str, &'a [u8])>;
        let cases: [(Entries, Option<&str>); 7] = [
            (
                vec![
                    ("./bin/qemu-img", prog),
                    ("./share/qemu/bios.bin", rom),
                    ("./share/qemu/x", rom),
                ],
                None,
            ),
            (
                vec![("bin/qemu-img", prog), ("share/qemu/bios.bin", rom)],
                None,
            ),
            (
                vec![("./bin/qemu-img", prog)],
                Some("it holds no ./share/qemu/bios.bin"),
            ),
            (
                vec![("./bin/qemu-img", other), ("./share/qemu/bios.bin", rom)],
                Some("./bin/qemu-img holds 18 bytes"),
            ),
            (
                vec![
                    ("./bin/qemu-img", &b"\x7fELF the emulatoR"[..]),
                    ("./share/qemu/bios.bin", rom),
                ],
                Some("./bin/qemu-img gave 17 bytes with the sha256"),
            ),
            (
                vec![
                    ("./bin/qemu-img", prog),
                    ("./bin/qemu-img", prog),
                    ("./share/qemu/bios.bin", rom),
                ],
                Some("./bin/qemu-img is in it twice"),
            ),
            (vec![], Some("it holds no ./bin/qemu-img")),
        ];
        for (n, (entries, refusal)) in cases.into_iter().enumerate() {
            let root = std::env::temp_dir().join(format!("podvm-tools-{}-{n}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let got = unpack(&a, &tgz(&entries)[..], &root);
            match refusal {
                None => {
                    got.unwrap();
                    let p = tools::bin(&root, "qemu-img");
                    assert_eq!(std::fs::read(&p).unwrap(), prog);
                    let mode = std::fs::metadata(&p).unwrap().permissions().mode();
                    assert_eq!(mode & 0o777, 0o755, "case {n}");
                    let r = tools::share(&root).join("bios.bin");
                    assert_eq!(std::fs::read(&r).unwrap(), rom);
                    assert!(!tools::share(&root).join("x").exists(), "case {n}");
                }
                Some(words) => {
                    let why = got.unwrap_err();
                    assert!(why.starts_with("t.tar.gz: "), "case {n}: {why}");
                    assert!(why.contains(words), "case {n}: {why}");
                    if words.contains("sha256") {
                        assert!(!tools::bin(&root, "qemu-img").exists(), "case {n}");
                    }
                }
            }
            let _ = std::fs::remove_dir_all(&root);
        }
        assert!(unpack(&a, &b"not gzip"[..], Path::new("/nonexistent")).is_err());
    }
}
