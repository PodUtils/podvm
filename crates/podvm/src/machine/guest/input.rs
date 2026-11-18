//! podvm's stdin for a guest (TODO/machine.md T-1723).

/// The most bytes of stdin that a guest takes.
pub const MAX_STDIN: u64 = 64 << 20;

/// podvm's stdin for the guest: its bytes where it is a pipe or a regular
/// file; `None` for a terminal or anything else, which gives the guest
/// `/dev/null`. More than [`MAX_STDIN`] bytes is an error.
pub fn read_stdin() -> Result<Option<Vec<u8>>, String> {
    use std::io::Read;
    use std::os::unix::fs::FileTypeExt;
    let Ok(md) = std::fs::metadata("/proc/self/fd/0") else {
        return Ok(None);
    };
    if !(md.file_type().is_fifo() || md.is_file()) {
        return Ok(None);
    }
    let mut buf = Vec::new();
    std::io::stdin()
        .lock()
        .take(MAX_STDIN + 1)
        .read_to_end(&mut buf)
        .map_err(|e| format!("stdin: {e}"))?;
    if buf.len() as u64 > MAX_STDIN {
        return Err(format!(
            "stdin holds more than {MAX_STDIN} bytes, the most that the guest takes"
        ));
    }
    Ok(Some(buf))
}
