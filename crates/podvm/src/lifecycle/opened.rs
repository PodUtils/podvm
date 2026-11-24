//! What the entry of a container opened, and what `exec` into it opens
//! (TODO/cli.md T-2053, T-2087).

use super::*;

/// What the entry of `c` opened (TODO/cli.md T-2053). A record of the
/// namespace rung from before the words were kept reads as its mount
/// namespace alone, the part that each entry of the rung opens.
pub(crate) fn opened_of(c: &Container) -> podvm_probe::isolation::Opened {
    use podvm_probe::isolation::{Opened, Part};
    match Opened::from_words(&c.hardening.opened) {
        o if o == Opened::NONE && c.rung == "namespace" => Opened::of(&[Part::Mount]),
        o => o,
    }
}

/// TODO/cli.md T-2087 (OR-B40-02): what `exec` into `c` opens. It joins
/// the namespaces of the run only while the run runs; into a container that
/// was only created, `exec` is a fresh chroot that shares the filesystem.
pub(crate) fn exec_opened(c: &Container) -> podvm_probe::isolation::Opened {
    if matches!(c.state, State::Running) {
        opened_of(c)
    } else {
        podvm_probe::isolation::Opened::NONE
    }
}
