//! The statuses of a row of the parity table (`TOOL.md` section 6.8).

/// The four statuses `TOOL.md` section 6.8 defines, and podvm has no fifth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Real semantics: what docker does, for the same input.
    Native,
    /// Works, with a documented difference that the banner states every time.
    Degraded,
    /// Accepted and does nothing. Listed here, so a caller can see it is a
    /// no-op instead of concluding it worked.
    Stub,
    /// Fails, with a named reason. ⛔ Never silently ignored.
    None,
}

impl Status {
    pub fn word(self) -> &'static str {
        match self {
            Status::Native => "Native",
            Status::Degraded => "Degraded",
            Status::Stub => "Stub",
            Status::None => "None",
        }
    }
}
