//! `--podvm-volume-base DIR` (PodUtils/podrunner#277, TODO/cli.md
//! T-1945): each `-v` source under DIR opens under it with no symbolic
//! link, and the bind takes that descriptor.

use super::*;

/// `--podvm-volume-base` and its value, as the caller gave it.
pub(super) fn set(verb: &str, a: &str, o: &mut Opts) -> std::result::Result<(), i32> {
    o.volume_base = Some(volume_base(verb, a)?);
    o.volume_base_given = Some(a.to_string());
    Ok(())
}

/// `--podvm-volume-base DIR` (PodUtils/podrunner#277): an absolute
/// directory, in its canonical spelling, so that the canonical source of
/// each `-v` compares with it.
fn volume_base(verb: &str, a: &str) -> std::result::Result<String, i32> {
    let refuse = |why: String| {
        eprintln!("podvm {verb}: --podvm-volume-base {a:?}: {why}");
        EXIT_FLAG_ERROR
    };
    if !a.starts_with('/') {
        return Err(refuse("takes an absolute path".to_string()));
    }
    let canon = std::fs::canonicalize(a).map_err(|e| refuse(format!("{e}")))?;
    if !canon.is_dir() {
        return Err(refuse("is not a directory".to_string()));
    }
    let Some(c) = canon.to_str() else {
        return Err(refuse("is not UTF-8".to_string()));
    };
    if c.contains([',', ':']) {
        return Err(refuse(
            "holds a `,` or a `:`, which the record cannot carry".to_string(),
        ));
    }
    Ok(c.to_string())
}

/// `path` with each `.` and `..` folded and no link read: the path as
/// the caller spelled it, made absolute against the working directory.
fn lexical(path: &str) -> std::path::PathBuf {
    use std::path::Component;
    let p = std::path::Path::new(path);
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(p)
    };
    let mut out = std::path::PathBuf::from("/");
    for c in joined.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(n) => out.push(n),
            _ => {}
        }
    }
    out
}

/// PodUtils/podrunner#277: `-v` reads the canonical host path, which
/// follows each link. A source that the caller spelled under the base must
/// name the same path under the canonical base, or a link under the base
/// moved it: a refusal that names the volume, before the bind.
pub(super) fn check(verb: &str, o: &Opts) -> std::result::Result<(), i32> {
    let (Some(base), Some(given)) = (&o.volume_base, &o.volume_base_given) else {
        return Ok(());
    };
    let given_base = lexical(given);
    for (v, raw) in o.volumes.iter().zip(&o.volume_given) {
        let Ok(rel) = lexical(raw)
            .strip_prefix(&given_base)
            .map(|r| r.to_path_buf())
        else {
            continue;
        };
        let canon = std::path::Path::new(&v.host);
        let same = canon.strip_prefix(base).is_ok_and(|r| r == rel.as_path());
        if !same {
            eprintln!(
                "podvm {verb}: -v {raw}:{}: the volume is refused: its host path holds a \
                 symbolic link under its base {base} (--podvm-volume-base); it resolves to {}",
                v.guest, v.host
            );
            return Err(podvm_image::error::EXIT_RUNTIME_ERROR);
        }
    }
    Ok(())
}
