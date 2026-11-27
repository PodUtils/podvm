//! The bound of `--timeout` (TODO/cli.md T-1655, T-2064): the rule `TIMEOUT`
//! of [`crate::number`] (TODO/gate.md T-1947).

/// The seconds of `--timeout`: a whole number from 1 to the seconds that fit
/// the milliseconds of a `u64`. TODO/cli.md T-1655.
pub(super) fn timeout(verb: &str, a: &str) -> Result<u64, i32> {
    crate::number::flag(verb, &crate::number::TIMEOUT, a)
}
