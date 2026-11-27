//! `--ulimit` (TODO/cli.md T-1655).

use pod_enter::{Limit, Resource};

/// One docker `--ulimit NAME=SOFT[:HARD]` as a limit of the payload.
///
/// TODO/cli.md T-1655. `NAME` is a word of `pod-enter`'s [`Resource`]
/// (`nofile`, `nproc`, `fsize`, ...); each value is a whole number or `-1`
/// / `unlimited` for no limit; a missing `HARD` is `SOFT`, as docker reads
/// it. A soft value over the hard one is refused here; a hard value over
/// the caller's own hard limit is refused by the child, by name.
pub fn ulimit(spec: &str) -> std::result::Result<Limit, String> {
    let (name, values) = spec
        .split_once('=')
        .ok_or_else(|| format!("{spec:?} is not NAME=SOFT[:HARD]"))?;
    let resource = Resource::from_word(name).ok_or_else(|| {
        format!("{name:?} names no resource limit (nofile, nproc, fsize, cpu, as, core, ...)")
    })?;
    // TODO/gate.md T-1947: decimal digits only; a sign, a letter, a value over
    // a `u64` and nothing are one line that names the range.
    let value = |v: &str| match v {
        "-1" | "unlimited" => Ok(pod_enter::limits::INFINITY),
        n => (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            .then(|| n.parse::<u64>().ok())
            .flatten()
            .ok_or_else(|| {
                format!(
                    "{v:?} in {spec:?} takes a whole number from 0 to {}, -1 or unlimited",
                    u64::MAX
                )
            }),
    };
    let (soft, hard) = match values.split_once(':') {
        Some((s, h)) => (value(s)?, value(h)?),
        None => {
            let v = value(values)?;
            (v, v)
        }
    };
    if soft > hard {
        return Err(format!("{spec:?}: the soft value is over the hard one"));
    }
    Ok(Limit {
        resource,
        soft,
        hard,
    })
}
