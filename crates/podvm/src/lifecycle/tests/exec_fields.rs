//! TODO/cli.md T-2087 (OR-B40-02): `.Exec.Mode` and `.Exec.Shares` say
//! what `exec` into the container does, against one table.

use super::*;
use podvm_probe::isolation::{Opened, Part};

/// One rule: `exec` joins the namespaces that the run opened only while it
/// runs on the namespace rung; into a container that was created, that
/// ended, or that runs on another rung, it is a fresh chroot that shares
/// the filesystem only.
#[test]
fn the_exec_fields_say_what_exec_does_in_each_state() {
    let opened = Opened::of(&[Part::User, Part::Pid, Part::Mount]).words();
    // (case, state, rung, .Exec.Mode, .Exec.Shares starts with "filesystem+")
    let rows = [
        (
            "created on the namespace rung",
            State::Created,
            "namespace",
            "fresh-chroot",
            false,
        ),
        (
            "running on the namespace rung",
            State::Running,
            "namespace",
            "join",
            true,
        ),
        (
            "exited on the namespace rung",
            State::Exited,
            "namespace",
            "fresh-chroot",
            false,
        ),
        (
            "running on the chroot rung",
            State::Running,
            "chroot",
            "fresh-chroot",
            false,
        ),
    ];
    for (case, state, rung, mode, joined) in rows {
        let mut c = super::command_width::with_argv(&["sleep", "50"]);
        c.state = state;
        c.of.rung = rung.into();
        c.of.hardening.opened = opened.clone();
        let o = exec_opened(&c);
        assert_eq!(o.exec_mode(rung), mode, "{case}");
        let shares = o.exec_shares(rung);
        assert_eq!(
            shares.starts_with("filesystem+"),
            joined,
            "{case}: {shares}"
        );
        if !joined {
            assert_eq!(shares, "filesystem", "{case}");
        }
    }
}
