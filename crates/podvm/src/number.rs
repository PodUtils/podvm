//! The one bounded parser of each number that a flag or a variable takes
//! (TODO/gate.md T-1947, the owner's ruling "Quality first"; PodUtils/podvm#493,
//! #506).
//!
//! Each number has a [`Rule`]: its name, what it is, its kind and its range.
//! [`check`] parses the text with no sign, no space and no exponent, and
//! refuses a value outside the range with one line that names the flag, the
//! range and the value. The three failures are the same line family: a text
//! that is not a number of the kind, a value under the range, and a value
//! over it (a product that overflows included). No value wraps, clamps or
//! panics. [`RULES`] is the table that the test of this module walks: each
//! rule, a value below, at each edge, above, overflowing and not a number,
//! through the production parser of each verb that takes the flag.

/// How the text of a number is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Decimal digits only.
    Whole,
    /// Digits, then an optional unit: `b`, or `k`, `m`, `g` or `t` (powers of
    /// 1024), with `i` and `b` after it as in `GiB` and `mb`. The value is
    /// bytes.
    Bytes,
    /// Digits, and a point with digits after it. The value is thousandths;
    /// the digits after the third are cut.
    Milli,
}

/// Why a text is not a value of its rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edge {
    /// Not a number of the kind: a sign, a letter, a space, an exponent,
    /// nothing.
    Bad,
    /// A number under the range.
    Low,
    /// A number over the range, or one that does not fit 64 bits.
    High,
}

/// The rule of one number.
#[derive(Debug)]
pub(crate) struct Rule {
    /// The flag or the variable, as the line names it.
    pub flag: &'static str,
    /// What the flag takes, after "takes": `a count of processes`.
    pub what: &'static str,
    pub kind: Kind,
    /// The least and the greatest value, in the unit of [`Kind`].
    pub lo: u64,
    pub hi: u64,
}

/// The largest value of a signed 64-bit count: docker's own bound of a size.
pub(crate) const I64_MAX: u64 = i64::MAX as u64;
/// The largest count of seconds whose milliseconds fit a `u64`.
pub(crate) const SECONDS_MAX: u64 = u64::MAX / 1000;
/// The kernel's `PID_MAX_LIMIT`: `pids.max` refuses a limit above it.
pub(crate) const PIDS_MAX: u64 = 4_194_304;
const SIZE: &str = "a size in bytes, with k, m, g or t";

const fn rule(flag: &'static str, what: &'static str, kind: Kind, lo: u64, hi: u64) -> Rule {
    Rule {
        flag,
        what,
        kind,
        lo,
        hi,
    }
}

/// `run --timeout`: the seconds of the wall bound (TODO/cli.md T-1655).
pub(crate) static TIMEOUT: Rule = rule("--timeout", "whole seconds", Kind::Whole, 1, SECONDS_MAX);
/// `stop -t`: the grace in seconds; 0 kills at once, as `docker stop -t 0`.
pub(crate) static STOP_GRACE: Rule = rule("-t", "whole seconds", Kind::Whole, 0, SECONDS_MAX);
/// `--podvm-timeout` of `machine ssh` and `windows`.
pub(crate) static PODVM_TIMEOUT: Rule = rule(
    "--podvm-timeout",
    "whole seconds",
    Kind::Whole,
    1,
    SECONDS_MAX,
);
pub(crate) static PODVM_CPUS: Rule = rule("--podvm-cpus", "a whole number", Kind::Whole, 1, 64);
pub(crate) static PODVM_MEM: Rule = rule("--podvm-mem", SIZE, Kind::Bytes, 1, I64_MAX);
pub(crate) static PIDS_LIMIT: Rule = rule(
    "--pids-limit",
    "a count of processes",
    Kind::Whole,
    1,
    PIDS_MAX,
);
pub(crate) static CPUS: Rule = rule("--cpus", "a number of CPUs", Kind::Milli, 10, 1_024_000);
/// 6 MiB, as docker holds it.
pub(crate) static MEMORY: Rule = rule("--memory", SIZE, Kind::Bytes, 6 << 20, I64_MAX);
pub(crate) static STORAGE_SIZE: Rule =
    rule("--storage-opt size", SIZE, Kind::Bytes, 64 << 20, I64_MAX);
/// 0 is docker's "no limit".
pub(crate) static TMPFS_SIZE: Rule = rule("--tmpfs size", SIZE, Kind::Bytes, 0, I64_MAX);
pub(crate) static PORTS: Rule = rule("--ports", "a port", Kind::Whole, 1, 65535);
pub(crate) static PUBLISH: Rule = rule("-p port", "a port", Kind::Whole, 1, 65535);
pub(crate) static CHANNEL_PORT: Rule =
    rule("--podvm-channel port", "a port", Kind::Whole, 1, 65535);
pub(crate) static ALLOW_CONNECT: Rule = rule("--allow-connect", "a port", Kind::Whole, 1, 65535);
/// The number of a signal (`kill -s 9`); a name is another spelling.
pub(crate) static SIGNAL: Rule = rule("--signal", "a signal number", Kind::Whole, 1, 64);
pub(crate) static TAIL: Rule = rule(
    "--tail",
    "a non-negative line count",
    Kind::Whole,
    0,
    u64::MAX,
);
pub(crate) static LIMIT_BYTES: Rule =
    rule("--limit-bytes", "a byte count", Kind::Whole, 1, I64_MAX);
pub(crate) static MAX_BYTES: Rule = rule("--max-bytes", "a byte count", Kind::Whole, 1, I64_MAX);
pub(crate) static CYCLES: Rule = rule("--cycles", "a whole number", Kind::Whole, 1, 1000);
pub(crate) static KILL_AFTER: Rule = rule(
    "--kill-after",
    "a whole number",
    Kind::Whole,
    1,
    365 * 24 * 3600,
);
pub(crate) static RESTART_AFTER: Rule = rule(
    "--restart-kernel-after",
    "a whole number",
    Kind::Whole,
    1,
    u32::MAX as u64,
);
pub(crate) static BOOT_TIMEOUT: Rule = rule(
    "PODVM_INIT_BOOT_TIMEOUT",
    "a number of seconds",
    Kind::Whole,
    1,
    3600,
);
pub(crate) static INSTALL_MEM: Rule =
    rule("PODVM_WINDOWS_INSTALL_MEM", SIZE, Kind::Bytes, 1, I64_MAX);
pub(crate) static INSTALL_CPUS: Rule = rule(
    "PODVM_WINDOWS_INSTALL_CPUS",
    "a whole number",
    Kind::Whole,
    1,
    64,
);
pub(crate) static INSTALL_DEADLINE: Rule = rule(
    "PODVM_WINDOWS_INSTALL_DEADLINE",
    "whole seconds",
    Kind::Whole,
    1,
    SECONDS_MAX,
);

/// Each rule. The test (`number::tests`) walks it; a number with no row here has no bounded
/// parser, and the test of the production parsers names it.
#[cfg(test)]
pub(crate) static RULES: &[&Rule] = &[
    &TIMEOUT,
    &STOP_GRACE,
    &PODVM_TIMEOUT,
    &PODVM_CPUS,
    &PODVM_MEM,
    &PIDS_LIMIT,
    &CPUS,
    &MEMORY,
    &STORAGE_SIZE,
    &TMPFS_SIZE,
    &PORTS,
    &PUBLISH,
    &CHANNEL_PORT,
    &ALLOW_CONNECT,
    &SIGNAL,
    &TAIL,
    &LIMIT_BYTES,
    &MAX_BYTES,
    &CYCLES,
    &KILL_AFTER,
    &RESTART_AFTER,
    &BOOT_TIMEOUT,
    &INSTALL_MEM,
    &INSTALL_CPUS,
    &INSTALL_DEADLINE,
];

/// The digits of `raw` as a number: `Bad` for each other text, `High` for
/// digits that do not fit 64 bits.
fn digits(raw: &str) -> Result<u64, Edge> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Edge::Bad);
    }
    raw.parse::<u64>().map_err(|_| Edge::High)
}

/// The multiplier of a unit text (`k`, `GiB`, `mb`, `b`, empty), or `None`.
fn unit(text: &str) -> Option<u64> {
    let t = text.to_ascii_lowercase();
    let t = t.strip_suffix('b').unwrap_or(&t);
    let letter = match t.strip_suffix('i') {
        Some(rest) if !rest.is_empty() => rest,
        _ => t,
    };
    match letter {
        "" => Some(1),
        "k" => Some(1 << 10),
        "m" => Some(1 << 20),
        "g" => Some(1 << 30),
        "t" => Some(1 << 40),
        _ => None,
    }
}

/// The value of `raw` in the unit of `kind`, with no range.
fn value(kind: Kind, raw: &str) -> Result<u64, Edge> {
    match kind {
        Kind::Whole => digits(raw),
        Kind::Bytes => {
            let cut = raw.find(|c: char| !c.is_ascii_digit()).unwrap_or(raw.len());
            let (number, text) = raw.split_at(cut);
            let n = digits(number)?;
            let mult = unit(text).ok_or(Edge::Bad)?;
            n.checked_mul(mult).ok_or(Edge::High)
        }
        Kind::Milli => {
            let (whole, fraction) = raw.split_once('.').unwrap_or((raw, ""));
            let w = digits(whole)?;
            if raw.contains('.') && fraction.is_empty() {
                return Err(Edge::Bad);
            }
            if !fraction.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Edge::Bad);
            }
            let first: String = fraction.chars().take(3).collect();
            let milli = format!("{first:0<3}")
                .parse::<u64>()
                .map_err(|_| Edge::Bad)?;
            w.checked_mul(1000)
                .and_then(|n| n.checked_add(milli))
                .ok_or(Edge::High)
        }
    }
}

/// The value of `raw` under `rule`, or why not.
pub(crate) fn parse(rule: &Rule, raw: &str) -> Result<u64, Edge> {
    let n = value(rule.kind, raw)?;
    if n < rule.lo {
        Err(Edge::Low)
    } else if n > rule.hi {
        Err(Edge::High)
    } else {
        Ok(n)
    }
}

/// A bound as the line writes it: thousandths as a decimal.
fn show(kind: Kind, n: u64) -> String {
    match kind {
        Kind::Milli if !n.is_multiple_of(1000) => format!("{}.{:03}", n / 1000, n % 1000)
            .trim_end_matches('0')
            .to_string(),
        Kind::Milli => (n / 1000).to_string(),
        _ => n.to_string(),
    }
}

/// The one line of a refused value: the flag, the range and the value. No
/// `podvm VERB:` lead and no stop; the caller adds its own.
pub(crate) fn line(rule: &Rule, raw: &str, why: Edge) -> String {
    let (lo, hi) = (show(rule.kind, rule.lo), show(rule.kind, rule.hi));
    let head = format!("{} takes {} from {lo} to {hi}", rule.flag, rule.what);
    match why {
        Edge::High => format!("{head}, {raw:?} is too large (the bound is {hi})"),
        Edge::Bad | Edge::Low => format!("{head}, not {raw:?}"),
    }
}

/// The value of `raw` under `rule`, or the line that refuses it.
pub(crate) fn check(rule: &Rule, raw: &str) -> Result<u64, String> {
    parse(rule, raw).map_err(|why| line(rule, raw, why))
}

/// [`check`] for a flag error: the line goes to stderr as
/// `podvm VERB: LINE`, and the error is the flag-error exit.
pub(crate) fn flag(verb: &str, rule: &Rule, raw: &str) -> Result<u64, i32> {
    check(rule, raw).map_err(|text| {
        eprintln!("podvm {verb}: {text}");
        podvm_image::error::EXIT_FLAG_ERROR
    })
}

/// The value of the variable that `rule` names: `None` where it is not set,
/// a flag error with the line where it is set to a value out of the rule.
pub(crate) fn env(verb: &str, rule: &Rule) -> Result<Option<u64>, i32> {
    from_env(verb, rule, std::env::var_os(rule.flag).as_deref())
}

/// [`env`] for the value `v` of the variable.
pub(crate) fn from_env(
    verb: &str,
    rule: &Rule,
    v: Option<&std::ffi::OsStr>,
) -> Result<Option<u64>, i32> {
    match v {
        None => Ok(None),
        Some(v) => flag(verb, rule, &v.to_string_lossy()).map(Some),
    }
}

#[cfg(test)]
mod tests;
