//! The wait of the payload that forwards signals, with the name of the
//! program before each line that `pod-enter` writes.

use crate::{Child, Result};

/// TODO/cli.md T-2064: a writer that starts each line with `podvm: `. The
/// wait of `pod-enter` names a forwarded signal and the signal exit of the
/// payload on the writer that it gets, with no name of the program.
pub struct Prefixed<'a> {
    inner: &'a mut dyn std::io::Write,
    at_start: bool,
}

impl<'a> Prefixed<'a> {
    pub fn new(inner: &'a mut dyn std::io::Write) -> Prefixed<'a> {
        Prefixed {
            inner,
            at_start: true,
        }
    }
}

impl std::io::Write for Prefixed<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // A line that has the name already keeps it.
        if self.at_start && !buf.is_empty() && !buf.starts_with(b"podvm") {
            self.inner.write_all(b"podvm: ")?;
        }
        if !buf.is_empty() {
            self.at_start = buf.ends_with(b"\n");
        }
        self.inner.write_all(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// The wait of the payload that forwards the signals of `pod-enter`, with
/// each line that it writes named `podvm:`.
pub fn wait_forwarding(child: &Child, err: &mut dyn std::io::Write) -> Result<i32> {
    child.wait_forwarding(&mut Prefixed::new(err))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-2064: each line of the wait starts with the name, once.
    #[test]
    fn the_lines_of_the_wait_start_with_the_name() {
        use std::io::Write;
        let mut out = Vec::new();
        {
            let mut p = Prefixed::new(&mut out);
            writeln!(p, "forwarded SIGINT to the payload (pid {})", 7).unwrap();
            writeln!(p, "podvm: kept").unwrap();
            writeln!(p, "the payload died on SIGINT: exit 130").unwrap();
        }
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "podvm: forwarded SIGINT to the payload (pid 7)\npodvm: kept\n\
             podvm: the payload died on SIGINT: exit 130\n"
        );
    }
}
