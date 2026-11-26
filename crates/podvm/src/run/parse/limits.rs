//! PodUtils/podrunner#110, TODO/machine.md T-1897: `--memory`, `--cpus`
//! and `--pids-limit`. The incus tier holds them as the keys of the
//! instance (`limits.memory`, `limits.cpu`, `limits.processes`); a native
//! run holds them in a delegated cgroup2 subtree (`run::cgroup`,
//! TODO/cli.md T-1747); each other tier refuses them by name.

use podvm_image::error::EXIT_FLAG_ERROR;

/// The key of `--storage-opt size=N`: the size of the root disk of an
/// incus instance, a device and not a key of its configuration
/// (PodUtils/podrunner#296 ask 1, TODO/machine.md T-1964).
pub(crate) const ROOT_SIZE: &str = "podvm.root.size";
pub(crate) const CPU: &str = "limits.cpu";
pub(crate) const CPU_ALLOWANCE: &str = "limits.cpu.allowance";

/// One flag and its value as the incus key and value.
pub(crate) fn incus_limit(verb: &str, flag: &str, v: &str) -> Result<(&'static str, String), i32> {
    let bad = |why: &str| {
        eprintln!("podvm {verb}: {flag} {v:?}: {why} (TODO/machine.md T-1897)");
        EXIT_FLAG_ERROR
    };
    match flag {
        "--memory" => {
            let (num, mult) = match v.to_ascii_lowercase().chars().last() {
                Some('k') => (&v[..v.len() - 1], 1u64 << 10),
                Some('m') => (&v[..v.len() - 1], 1 << 20),
                Some('g') => (&v[..v.len() - 1], 1 << 30),
                Some('b') => (&v[..v.len() - 1], 1),
                _ => (v, 1),
            };
            let n: u64 = num
                .parse()
                .map_err(|_| bad("takes bytes with b, k, m or g"))?;
            // OR-B41-16: a product past u64 is too large, not too small.
            let bytes = n.checked_mul(mult).ok_or_else(|| {
                bad("is larger than 16 EiB, the most that a count of bytes holds")
            })?;
            if bytes < 6 << 20 {
                return Err(bad("takes at least 6m, as docker does"));
            }
            Ok(("limits.memory", format!("{bytes}B")))
        }
        "--cpus" => {
            let c: f64 = v.parse().map_err(|_| bad("takes a number of CPUs"))?;
            if !(c > 0.0 && c <= 1024.0) {
                return Err(bad("takes more than 0 and at most 1024 CPUs"));
            }
            // A whole number is a count of CPUs; a part is a share of time.
            if c.fract() == 0.0 {
                Ok((CPU, format!("{}", c as u64)))
            } else {
                Ok((CPU_ALLOWANCE, format!("{}%", (c * 100.0).round() as u64)))
            }
        }
        "--storage-opt" => {
            let Some(size) = v.strip_prefix("size=") else {
                return Err(bad(
                    "takes size=N (docker's form); podvm knows no other option",
                ));
            };
            // `10g`, `10G`, `10GB` and `10GiB` (docker's and incus's
            // spellings) name the same size.
            let t = size.trim_end_matches(['b', 'B']);
            let t = t.strip_suffix(['i', 'I']).unwrap_or(t);
            let (num, mult) = match t.chars().last().map(|c| c.to_ascii_lowercase()) {
                Some('k') => (&t[..t.len() - 1], 1u64 << 10),
                Some('m') => (&t[..t.len() - 1], 1 << 20),
                Some('g') => (&t[..t.len() - 1], 1 << 30),
                Some('t') => (&t[..t.len() - 1], 1 << 40),
                _ => (t, 1),
            };
            let n: u64 = num
                .parse()
                .map_err(|_| bad("takes size=N with k, m, g or t"))?;
            let bytes = n.checked_mul(mult).filter(|b| *b >= 64 << 20);
            let bytes = bytes.ok_or_else(|| bad("takes a size of at least 64m"))?;
            Ok((ROOT_SIZE, format!("{bytes}B")))
        }
        _ => {
            let n: u64 = v.parse().map_err(|_| bad("takes a count of processes"))?;
            if n == 0 {
                return Err(bad("takes 1 or more"));
            }
            Ok(("limits.processes", n.to_string()))
        }
    }
}

/// Refuse the limits on a tier that does not hold them.
pub(crate) fn refuse_off_incus(
    verb: &str,
    limits: &[(String, &'static str, String)],
) -> Result<(), i32> {
    if let Some((flag, _, _)) = limits.first() {
        eprintln!(
            "podvm {verb}: {flag} needs a cgroup, which this tier does not hold: the incus \
             tier (--podvm-tier=incus) holds it as a limit of the instance; --ulimit nproc= \
             and --ulimit as= hold on each tier (TODO/machine.md T-1897, TODO/cli.md T-1747)"
        );
        return Err(EXIT_FLAG_ERROR);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T-1964: each spelling of a size names the same bytes; a size below
    /// 64m, an unknown unit and an option other than size= are refusals.
    #[test]
    fn storage_opt_takes_each_spelling_of_a_size() {
        for (v, want) in [
            ("size=1G", Some(1u64 << 30)),
            ("size=1g", Some(1 << 30)),
            ("size=1GB", Some(1 << 30)),
            ("size=1GiB", Some(1 << 30)),
            ("size=512MiB", Some(512 << 20)),
            ("size=2T", Some(2 << 40)),
            ("size=1073741824", Some(1 << 30)),
            ("size=1m", None),
            ("size=1X", None),
            ("size=", None),
            ("dm.basesize=1G", None),
        ] {
            let got = incus_limit("run", "--storage-opt", v).ok();
            assert_eq!(got, want.map(|b| (ROOT_SIZE, format!("{b}B"))), "{v}");
        }
    }

    #[test]
    fn incus_limits_map_the_docker_values() {
        assert_eq!(
            incus_limit("run", "--memory", "512m").unwrap(),
            ("limits.memory", "536870912B".to_string())
        );
        assert_eq!(
            incus_limit("run", "--cpus", "2").unwrap(),
            ("limits.cpu", "2".to_string())
        );
        assert_eq!(
            incus_limit("run", "--cpus", "1.5").unwrap(),
            ("limits.cpu.allowance", "150%".to_string())
        );
        assert_eq!(
            incus_limit("run", "--pids-limit", "64").unwrap(),
            ("limits.processes", "64".to_string())
        );
        assert!(incus_limit("run", "--memory", "1k").is_err());
        assert!(incus_limit("run", "--cpus", "0").is_err());
        assert!(incus_limit("run", "--pids-limit", "x").is_err());
        assert!(refuse_off_incus("run", &[]).is_ok());
        let l = vec![("--memory".to_string(), "limits.memory", "1B".to_string())];
        assert!(refuse_off_incus("run", &l).is_err());
    }
}
