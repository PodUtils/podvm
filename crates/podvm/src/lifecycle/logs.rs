//! `podvm logs`: the log of a container, a machine or an instance of incus
//! as a stream (TODO/cli.md T-1318, T-1337, T-2046).

use super::*;

/// `podvm logs [-f|--follow] [--tail N] [--limit-bytes N] <container>`
pub const LOGS_USAGE: &str = "\
usage: podvm logs [-f|--follow] [--tail N] [--limit-bytes N] [--podvm-tier incus] <container>

  podvm writes the log to stdout as the payload wrote it, from its start,
  through one buffer of 64 KiB: a large log does not fill the memory.

  -f, --follow     write what comes, until the container ends.
  --tail N         the last N lines only (0: none; `all`: the whole log, the
                   default); -f follows from there.
  --limit-bytes N  stop after the first N bytes and exit 0 (N from 1 to
                   9223372036854775807). With --tail: the first N bytes of
                   the tail. With -f: stop at N bytes or at the end.
  --podvm-tier incus  the log of an instance of incus. The API of incus
                   gives the file whole: podvm holds it in memory, 64 MiB at
                   most (a larger log is refused), and refuses --tail.
";

/// The largest `--limit-bytes`: the largest offset of a file (`off_t`).
pub const LIMIT_BYTES_MAX: u64 = i64::MAX as u64;

/// What `logs` was asked for. One parser, so the flag cannot be accepted in
/// one position and lost in another (TODO/supervise.md T-1318).
#[derive(Debug)]
pub struct LogsArgs {
    pub follow: bool,
    pub tail: Option<u64>,
    /// `--limit-bytes`: the bytes to write at most (TODO/cli.md T-2046).
    pub limit: Option<u64>,
    pub want: String,
}

/// The one parser of a whole number of a flag: decimal digits only, from
/// `lo` to `hi`. Each other value (empty, a sign, a letter, past the range)
/// is refused with the flag, the range and the value.
pub(crate) fn whole(flag: &str, what: &str, raw: &str, lo: u64, hi: u64) -> Result<u64, String> {
    let n = (!raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()))
        .then(|| raw.parse::<u64>().ok())
        .flatten()
        .filter(|n| (lo..=hi).contains(n));
    n.ok_or_else(|| format!("{flag} takes a {what} from {lo} to {hi}, not {raw:?}"))
}

/// The value of one valued flag of `logs`.
fn value(flag: &str, raw: &str) -> Result<u64, String> {
    match flag {
        "--tail" => whole(flag, "non-negative line count", raw, 0, u64::MAX),
        _ => whole(flag, "byte count", raw, 1, LIMIT_BYTES_MAX),
    }
}

pub fn parse_logs(args: &[String]) -> std::result::Result<LogsArgs, i32> {
    let mut o = LogsArgs {
        follow: false,
        tail: None,
        limit: None,
        want: String::new(),
    };
    let mut want: Option<String> = None;
    let mut expecting: Option<&'static str> = None;
    let set = |o: &mut LogsArgs, flag: &str, raw: &str| -> Result<(), i32> {
        // TODO/cli.md T-2061: docker's `--tail all` is the default, the whole log.
        if flag == "--tail" && raw == "all" {
            o.tail = None;
            return Ok(());
        }
        let n = value(flag, raw).map_err(|why| {
            eprintln!("podvm logs: {why}");
            EXIT_FLAG_ERROR
        })?;
        match flag {
            "--tail" => o.tail = Some(n),
            _ => o.limit = Some(n),
        }
        Ok(())
    };
    for a in args {
        if let Some(flag) = expecting.take() {
            set(&mut o, flag, a)?;
            continue;
        }
        match a.as_str() {
            "-f" | "--follow" => o.follow = true,
            "--tail" => expecting = Some("--tail"),
            "--limit-bytes" => expecting = Some("--limit-bytes"),
            "-h" | "--help" => {
                print!("{LOGS_USAGE}");
                return Err(0);
            }
            other if other.starts_with("--tail=") => set(&mut o, "--tail", &other[7..])?,
            other if other.starts_with("--limit-bytes=") => {
                set(&mut o, "--limit-bytes", &other[14..])?
            }
            other if other.starts_with('-') => {
                crate::parity::admit("logs", other, LOGS_USAGE)?;
                return Err(crate::parity::no_arm("logs", other));
            }
            other if want.is_none() => want = Some(other.to_string()),
            // PodUtils/podvm#486: `logs` names one container, and a second
            // word is refused, as `pull` refuses a second image.
            other => {
                eprintln!("podvm logs: logs takes one container, and {other:?} is a second one\n{LOGS_USAGE}");
                return Err(podvm_image::error::EXIT_CLI_ERROR);
            }
        }
    }
    if let Some(flag) = expecting {
        eprintln!("podvm logs: {flag} needs a value");
        return Err(EXIT_FLAG_ERROR);
    }
    let Some(want) = want else {
        eprintln!("{LOGS_USAGE}");
        return Err(EXIT_CLI_ERROR);
    };
    o.want = want;
    Ok(o)
}

/// `podvm logs [-f|--follow] [--tail N] [--limit-bytes N] <container>`
pub fn logs(args: &[String]) -> i32 {
    // ⭐ TODO/cli.md T-1330: bundled shorts expand before admission.
    let expanded = match crate::parity::expand("logs", args) {
        Ok(a) => a,
        Err(member) => return crate::parity::refuse_member("logs", &member, LOGS_USAGE),
    };
    let args: &[String] = &expanded;
    if let Some(c) = crate::parity::admit_all("logs", args, LOGS_USAGE) {
        return c;
    }
    let o = match parse_logs(args) {
        Ok(o) => o,
        Err(c) => return c,
    };
    // TODO/machine.md T-1870: the serial line of a detached machine.
    if let Some(r) = crate::windows::machine::find(&o.want) {
        return crate::windows::machine::logs(&r.name, &o);
    }
    let s = match store() {
        Ok(s) => s,
        Err(c) => return c,
    };
    let opened = match podvm_supervise::stream::open_log(&s, &o.want) {
        Ok(f) => f,
        Err(e) => return fail("logs", e),
    };
    // ⚠ The payload's own bytes, to stdout, unaltered. `logs` is the one
    // verb whose stdout is not podvm's, following or not.
    let mut out = std::io::stdout().lock();
    let (len, written) = match opened {
        Some((mut f, len)) => match copy_log(&mut f, len, &o, &mut out) {
            Ok(n) => (len, n),
            Err(c) => return c,
        },
        None => (0, 0),
    };
    if !o.follow || o.limit.is_some_and(|l| written >= l) {
        return 0;
    }
    // ⭐ TODO/cli.md T-1337: from the end of the first read, so a line that
    // comes between the two is written one time.
    let left = o.limit.map(|l| l - written);
    let r = match left {
        Some(left) => {
            let mut cap = podvm_supervise::stream::Capped::new(&mut out, left);
            match podvm_supervise::stream::follow_from(&s, &o.want, &mut cap, len) {
                // TODO/cli.md T-2046: the bound ends the follow, with 0.
                Err(_) if cap.full() => Ok(()),
                r => r,
            }
        }
        None => podvm_supervise::stream::follow_from(&s, &o.want, &mut out, len),
    };
    match r {
        Ok(()) => 0,
        Err(e) => fail("logs", e),
    }
}

/// The first part of a log for `o`: from the start, or from the last
/// `--tail` lines, to `len`, at most `--limit-bytes` of it. The bytes
/// written, or the exit: a read that fails is the error of the log, and a
/// write that fails is the error of stdout (TODO/cli.md T-1816), never a
/// panic and never 0.
pub(crate) fn copy_log(
    f: &mut std::fs::File,
    len: u64,
    o: &LogsArgs,
    out: &mut dyn Write,
) -> Result<u64, i32> {
    use podvm_supervise::stream::{self, Fault};
    let start = match o.tail {
        Some(n) => match stream::tail_start(f, len, n) {
            Ok(at) => at,
            Err(e) => {
                eprintln!("podvm logs: {}: reading the log: {e}", o.want);
                return Err(EXIT_RUNTIME_ERROR);
            }
        },
        None => 0,
    };
    match stream::copy_range(f, start, len, o.limit, out) {
        Ok(n) => Ok(n),
        Err(Fault::Write(e)) => Err(crate::names::stdout::failed(&e)),
        Err(Fault::Read(e)) => {
            eprintln!("podvm logs: {}: reading the log: {e}", o.want);
            Err(EXIT_RUNTIME_ERROR)
        }
    }
}

#[cfg(test)]
mod tests;
