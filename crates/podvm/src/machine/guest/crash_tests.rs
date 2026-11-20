//! TODO/machine.md T-2016: the crash of the emulator before the command.

/// TODO/machine.md T-2016: only a crash signal before the command and
/// before the bound gives one more boot.
#[test]
fn a_crash_of_the_emulator_before_the_command_boots_once_more() {
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;
    let sig = |n: i32| Some(ExitStatus::from_raw(n));
    let exit1 = Some(ExitStatus::from_raw(1 << 8));
    // (a payload code, ready, timed out, the status, the answer)
    for (code, ready, late, status, want) in [
        (false, false, false, sig(11), Some(11)),
        (false, false, false, sig(7), Some(7)),
        (false, false, false, sig(6), Some(6)),
        (false, false, false, sig(9), None),
        (false, false, false, sig(15), None),
        (false, true, false, sig(11), None),
        (true, false, false, sig(11), None),
        (false, false, true, sig(11), None),
        (false, false, false, exit1, None),
        (false, false, false, None, None),
    ] {
        assert_eq!(
            super::drive::crashed_early(code, ready, late, status),
            want,
            "{code} {ready} {late} {status:?}"
        );
    }
}
