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

/// One flag and its value as the incus key and value. Each number goes
/// through its rule of [`crate::number`], so a value out of the range, a
/// sign, an exponent and a word are one refusal that names the range
/// (TODO/gate.md T-1947).
pub(crate) fn incus_limit(verb: &str, flag: &str, v: &str) -> Result<(&'static str, String), i32> {
    use crate::number::{check, CPUS, MEMORY, PIDS_LIMIT, STORAGE_SIZE};
    let refuse = |text: String| {
        eprintln!("podvm {verb}: {text} (TODO/machine.md T-1897)");
        EXIT_FLAG_ERROR
    };
    match flag {
        "--memory" => {
            let bytes = check(&MEMORY, v).map_err(refuse)?;
            Ok(("limits.memory", format!("{bytes}B")))
        }
        "--cpus" => {
            let milli = check(&CPUS, v).map_err(refuse)?;
            // A whole number is a count of CPUs; a part is a share of time.
            if milli.is_multiple_of(1000) {
                Ok((CPU, (milli / 1000).to_string()))
            } else {
                Ok((CPU_ALLOWANCE, format!("{}%", (milli + 5) / 10)))
            }
        }
        "--storage-opt" => {
            let Some(size) = v.strip_prefix("size=") else {
                return Err(refuse(format!(
                    "--storage-opt {v:?} takes size=N (docker's form); podvm knows no other option"
                )));
            };
            // `10g`, `10G`, `10GB` and `10GiB` (docker's and incus's
            // spellings) name the same size.
            let bytes = check(&STORAGE_SIZE, size).map_err(refuse)?;
            Ok((ROOT_SIZE, format!("{bytes}B")))
        }
        _ => {
            let n = check(&PIDS_LIMIT, v).map_err(refuse)?;
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
    /// T-1968: the machine tier takes each kept size up to 16 TiB as the
    /// bytes of the disk of its guest, and refuses a larger one by name.
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
            let disk =
                got.map(|(_, kept)| crate::machine::guest::disk::of_value(Some(&kept), true));
            match want {
                Some(b) if b <= 16 << 40 => assert_eq!(disk, Some(Ok(Some(b))), "{v}"),
                Some(_) => panic!("{v}: a row of more than 16 TiB belongs below"),
                None => assert_eq!(disk, None, "{v}"),
            }
        }
        let (_, kept) = incus_limit("run", "--storage-opt", "size=17T").unwrap();
        let e = crate::machine::guest::disk::of_value(Some(&kept), true).unwrap_err();
        assert!(
            e.starts_with("--storage-opt size=") && e.contains("16 TiB"),
            "{e}"
        );
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
