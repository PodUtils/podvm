//! The files of `--env-file` and `--env-file0` (TODO/cli.md T-0801, T-1971).

use super::values::from_host;
use super::*;

/// The line and the byte of the first byte that is not UTF-8 (TODO/cli.md
/// T-2064); `(1, 0)` for bytes that are.
pub(crate) fn utf8_line(bytes: &[u8]) -> (usize, usize) {
    let at = std::str::from_utf8(bytes).map_or_else(|e| e.valid_up_to(), |_| 0);
    (bytes[..at].iter().filter(|b| **b == b'\n').count() + 1, at)
}

/// `--env-file`: docker's file of `KEY=VALUE` lines, loaded at the flag's
/// position so a later `-e` wins over the file and a later file over an
/// earlier flag, the same rule `-e` already documents. TODO/cli.md T-0801.
///
/// The file is bounded (1 MiB) and must be UTF-8: buffering an unbounded
/// body or guessing at bytes is the shape
/// `docs/conventions/forbidden-patterns.md` refuses. A line with a name
/// and no `=` takes the value of the name from the caller's environment,
/// and gives nothing where the caller has none, as docker does and as
/// `-e NAME` does (TODO/cli.md T-2070, OR-B38-11).
pub(crate) fn read_env_file(verb: &str, path: &str) -> std::result::Result<Vec<String>, i32> {
    const MAX_ENV_FILE: u64 = 1_048_576;
    let meta = std::fs::metadata(path).map_err(|e| {
        eprintln!("podvm {verb}: --env-file {path:?}: {e}");
        EXIT_FLAG_ERROR
    })?;
    if !meta.is_file() {
        eprintln!("podvm {verb}: --env-file {path:?} is not a file");
        return Err(EXIT_FLAG_ERROR);
    }
    if !super::access::caller_may(&meta, 0o400) {
        eprintln!("podvm {verb}: --env-file {path:?}: Permission denied (the mode lets its owner not read it)");
        return Err(EXIT_FLAG_ERROR);
    }
    if meta.len() > MAX_ENV_FILE {
        eprintln!(
            "podvm {verb}: --env-file {path:?} is {} bytes, over the 1048576-byte ceiling",
            meta.len()
        );
        return Err(EXIT_FLAG_ERROR);
    }
    let bytes = std::fs::read(path).map_err(|e| {
        eprintln!("podvm {verb}: --env-file {path:?}: {e}");
        EXIT_FLAG_ERROR
    })?;
    // TODO/cli.md T-2064: bytes that are not UTF-8 name the line.
    let text = String::from_utf8(bytes).map_err(|e| {
        let (line, at) = utf8_line(e.as_bytes());
        eprintln!(
            "podvm {verb}: --env-file {path:?} line {line} is not UTF-8 (byte {at} of the file)"
        );
        EXIT_FLAG_ERROR
    })?;
    env_lines(&text, &from_host).map_err(|(n, why)| {
        eprintln!("podvm {verb}: --env-file {path:?} line {n} {why}");
        EXIT_FLAG_ERROR
    })
}

/// The entries of the text of an env file. `host` gives the entry of a
/// line with a name and no `=`. A UTF-8 byte order mark on line 1 is not
/// part of the first name (docker drops it). The error holds the line
/// number and the end of the line of the refusal.
pub(crate) fn env_lines(
    text: &str,
    host: &dyn Fn(&str) -> Option<String>,
) -> std::result::Result<Vec<String>, (usize, String)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut out = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            if line.contains(char::is_whitespace) {
                return Err((n + 1, format!("has white space in the name: {line:?}")));
            }
            out.extend(host(line));
            continue;
        };
        let key = k.trim();
        if key.is_empty() {
            return Err((n + 1, format!("has no name: {line:?}")));
        }
        // TODO/cli.md T-1828: the value as the line gives it, quotes too,
        // as docker and podman keep it (`B="two words"` is `"two words"`).
        out.push(format!("{key}={v}"));
    }
    Ok(out)
}

/// `--env-file0 PATH` (PodUtils/podrunner#300, TODO/cli.md T-1971): the
/// entries `KEY=VALUE` with a NUL after each, as `env -0` writes them, so
/// that a value can hold a newline. The checks of the file are those of
/// [`read_env_file`]; a value is kept byte for byte, with no trim.
pub(crate) fn read_env_file0(verb: &str, path: &str) -> std::result::Result<Vec<String>, i32> {
    const MAX_ENV_FILE: u64 = 1_048_576;
    let refuse = |why: String| {
        eprintln!("podvm {verb}: --env-file0 {path:?}{why}");
        EXIT_FLAG_ERROR
    };
    let meta = std::fs::metadata(path).map_err(|e| refuse(format!(": {e}")))?;
    if !meta.is_file() {
        return Err(refuse(" is not a file".into()));
    }
    if !super::access::caller_may(&meta, 0o400) {
        return Err(refuse(
            ": Permission denied (the mode lets its owner not read it)".into(),
        ));
    }
    if meta.len() > MAX_ENV_FILE {
        return Err(refuse(format!(
            " is {} bytes, over the 1048576-byte ceiling",
            meta.len()
        )));
    }
    let bytes = std::fs::read(path).map_err(|e| refuse(format!(": {e}")))?;
    let mut out = Vec::new();
    for (n, raw) in bytes.split(|b| *b == 0).enumerate() {
        if raw.is_empty() {
            continue;
        }
        let Ok(entry) = std::str::from_utf8(raw) else {
            return Err(refuse(format!(" entry {} is not UTF-8", n + 1)));
        };
        match entry.split_once('=') {
            Some((k, _)) if !k.is_empty() && !k.contains(['\n', '\r']) => {
                out.push(entry.to_string())
            }
            Some(_) => return Err(refuse(format!(" entry {} has no name", n + 1))),
            None => return Err(refuse(format!(" entry {} has no `=`", n + 1))),
        }
    }
    Ok(out)
}
