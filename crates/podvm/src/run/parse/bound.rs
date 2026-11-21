//! The bound of `--timeout` (TODO/cli.md T-1655, T-2064).

use super::*;

/// The largest `--timeout`: the seconds that fit the milliseconds of a `u64`.
pub(in crate::run) const MAX: u64 = u64::MAX / 1000;

/// TODO/cli.md T-2064: the text of a refused `--timeout`. A number of digits
/// that is over the bound says so and gives it.
pub(in crate::run) fn refusal(a: &str) -> String {
    let big =
        !a.is_empty() && a.bytes().all(|b| b.is_ascii_digit()) && a.bytes().any(|b| b != b'0');
    let why = if big {
        format!("{a:?} is too large (the bound is {MAX})")
    } else {
        format!("not {a:?}")
    };
    format!("--timeout takes whole seconds from 1 to {MAX}, {why}")
}

/// The seconds of `--timeout`: a whole number over 0. TODO/cli.md T-1655.
pub(super) fn timeout(verb: &str, a: &str) -> Result<u64, i32> {
    match a.parse::<u64>() {
        Ok(n) if n > 0 && n <= MAX => Ok(n),
        _ => {
            eprintln!("podvm {verb}: {}", refusal(a));
            Err(EXIT_FLAG_ERROR)
        }
    }
}
