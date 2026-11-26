//! The tests of `drive.rs`.

use super::ended_how;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

/// PodUtils/podrunner#195: the message names the end of the emulator.
#[test]
fn the_end_of_the_emulator_is_named() {
    let code = ExitStatus::from_raw(1 << 8);
    let killed = ExitStatus::from_raw(9);
    assert_eq!(
        ended_how(Some(code), false),
        "the emulator exited with code 1, before the guest reached the command"
    );
    assert_eq!(
        ended_how(Some(killed), true),
        "the emulator ended on signal 9, after the guest started the command"
    );
    assert!(ended_how(None, false).starts_with("the emulator was still running"));
}

/// PodUtils/podrunner#198: the wait, its time and the initrd are named.
#[test]
fn the_wait_is_named_with_its_time_and_the_initrd() {
    use super::{initrd_bytes, waited};
    use std::time::Duration;
    assert_eq!(
        waited(Duration::from_millis(2500), true, Some(300 << 20)),
        "the console of the emulator closed 2.5 s after the start, with an initrd of 300 MiB"
    );
    assert_eq!(
        waited(Duration::from_secs(600), false, None),
        "the bound of the run came 600.0 s after the start"
    );
    assert_eq!(initrd_bytes(&["-m".to_string(), "512M".to_string()]), None);
}
