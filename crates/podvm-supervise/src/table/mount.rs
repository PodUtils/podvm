//! The mounts of a run in its record (TODO/cli.md T-2108, OR-B42-08): one
//! list for each rung and tier, written where the run parses `-v` and
//! `--tmpfs`. `inspect` gives docker's `.Mounts`, `.HostConfig.Binds` and
//! `.HostConfig.Tmpfs` from it.

use serde::{Deserialize, Serialize};

/// The kind of a mount, as docker's `Type` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MountKind {
    /// `-v HOST:GUEST[:OPTIONS]`: a host directory or file.
    Bind,
    /// `--tmpfs GUEST[:OPTIONS]`.
    Tmpfs,
    /// A kind that this podvm does not know (a record of a later podvm):
    /// the table still reads, and `inspect` leaves the mount out.
    #[serde(other)]
    Other,
}

/// One mount of a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mount {
    pub kind: MountKind,
    /// The canonical host path of a bind; empty for a tmpfs.
    #[serde(default)]
    pub source: String,
    /// The path in the container.
    pub destination: String,
    /// The options as the caller wrote them (`ro`, `z`, `overlay`,
    /// `size=64m`), or empty: docker's `Mode` of a bind.
    #[serde(default)]
    pub options: String,
    /// False where the options make the mount read-only.
    pub rw: bool,
}
