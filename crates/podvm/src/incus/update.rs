//! `podvm update --podvm-tier=incus`, TODO/machine.md T-2093.

use std::collections::BTreeMap;

use pod_incus::{Client, ErrorKind, Instance};
use podvm_image::error::{EXIT_FLAG_ERROR, EXIT_RUNTIME_ERROR};

use crate::run::limits::{incus_limit, CPU, CPU_ALLOWANCE};

use super::verbs::{block_on, connect, ours, valid_name};

const VERB: &str = "update";
pub(crate) const USAGE: &str = "\
usage: podvm update --podvm-tier=incus NAME [--memory BYTES] [--cpus N] [--pids-limit N]

  Update at least one limit on a running podvm container in Incus.
  -m, --memory BYTES  from 6291456 to 9223372036854775807 bytes; b, k, m, g or t.
  --cpus N            a CPU count or, for a fraction, a scheduler share;
                      from 0.01 to 1024.
  --pids-limit N      from 1 to 4194304 processes.
";

#[derive(Debug)]
struct Update {
    name: String,
    config: BTreeMap<String, String>,
}

fn bad(why: impl std::fmt::Display) -> i32 {
    eprintln!("podvm update: {why} (TODO/machine.md T-2093)");
    EXIT_FLAG_ERROR
}

fn take_name(name: &mut Option<String>, arg: &str) -> Result<(), i32> {
    if name.is_some() {
        return Err(bad(format!(
            "takes one instance name, and {arg:?} is another"
        )));
    }
    if !valid_name(arg) {
        return Err(bad(format!(
            "instance name {arg:?} must start with a letter and contain at most 63 letters, \
             digits and hyphens"
        )));
    }
    *name = Some(arg.to_string());
    Ok(())
}

fn other_cpu(key: &str) -> Option<&'static str> {
    match key {
        CPU => Some(CPU_ALLOWANCE),
        CPU_ALLOWANCE => Some(CPU),
        _ => None,
    }
}

fn parse(args: &[String]) -> Result<Option<Update>, i32> {
    let (options, operands) = args
        .iter()
        .position(|a| a == "--")
        .map_or((args, &[][..]), |i| (&args[..i], &args[i + 1..]));
    let expanded = crate::parity::expand(VERB, options)
        .map_err(|flag| crate::parity::refuse_member(VERB, &flag, USAGE))?;
    let mut name = None;
    let mut incus = false;
    let mut config = BTreeMap::new();
    let mut it = expanded.iter().map(String::as_str);
    while let Some(arg) = it.next() {
        if !arg.starts_with('-') {
            take_name(&mut name, arg)?;
            continue;
        }
        crate::parity::admit(VERB, arg, USAGE)?;
        match arg {
            "-h" | "--help" => return Ok(None),
            "--podvm-tier" => match it.next() {
                Some("incus") => incus = true,
                Some(tier) => {
                    return Err(bad(format!(
                        "--podvm-tier={tier:?}: update takes incus only"
                    )));
                }
                None => return Err(bad("--podvm-tier needs incus")),
            },
            "-m" | "--memory" | "--cpus" | "--pids-limit" => {
                let value = it
                    .next()
                    .ok_or_else(|| bad(format!("{arg} needs a value")))?;
                let (key, value) =
                    incus_limit(VERB, if arg == "-m" { "--memory" } else { arg }, value)?;
                if let Some(other) = other_cpu(key) {
                    config.remove(other);
                }
                config.insert(key.to_string(), value);
            }
            _ => return Err(crate::parity::no_arm(VERB, arg)),
        }
    }
    for arg in operands {
        take_name(&mut name, arg)?;
    }
    if !incus {
        return Err(bad("requires --podvm-tier=incus"));
    }
    let name = name.ok_or_else(|| bad("name a running Incus instance"))?;
    if config.is_empty() {
        return Err(bad(
            "give at least one of --memory, --cpus, or --pids-limit",
        ));
    }
    Ok(Some(Update { name, config }))
}

pub(crate) fn dispatch(args: &[String]) -> i32 {
    let request = match parse(args) {
        Ok(Some(r)) => r,
        Ok(None) => {
            print!("{USAGE}");
            return 0;
        }
        Err(code) => return code,
    };
    block_on(VERB, async {
        let done = match connect().await {
            Ok((client, _)) => apply(&client, &request).await,
            Err(e) => Err(e),
        };
        match done {
            Ok(()) => {
                println!("{}", request.name);
                0
            }
            Err(e) => {
                eprintln!("podvm update: {}: {e}", request.name);
                EXIT_RUNTIME_ERROR
            }
        }
    })
}

fn active(instance: &Instance, name: &str) -> Result<(), String> {
    if instance.name != name || !ours(instance) || instance.kind != "container" {
        return Err("the daemon did not return a podvm container with this name".into());
    }
    if instance.status != "Running" {
        return Err(format!(
            "the Incus container must be Running, got {:?}",
            instance.status
        ));
    }
    Ok(())
}

fn effective<'a>(instance: &'a Instance, key: &str) -> &'a str {
    instance
        .expanded_config
        .get(key)
        .or_else(|| instance.config.get(key))
        .map_or("", String::as_str)
}

async fn apply(client: &Client, request: &Update) -> Result<(), String> {
    let name = &request.name;
    let (before, etag) = client
        .instance_with_etag(name)
        .await
        .map_err(|e| e.to_string())?;
    active(&before, name)?;
    let etag = etag
        .filter(|tag| !matches!(tag.as_str().trim(), "" | "*"))
        .ok_or("incusd returned no usable ETag for the instance")?;

    let mut patch = request.config.clone();
    // One CPU option selects one form. Clear the other form if it still
    // limits the container, including a value inherited from its profile.
    let other = patch.keys().find_map(|key| other_cpu(key));
    if let Some(other) = other
        && !effective(&before, other).is_empty()
    {
        patch.insert(other.to_string(), String::new());
    }
    if let Err(e) = client
        .patch_instance_config(name, &patch, Some(&etag))
        .await
    {
        return Err(match e.kind() {
            ErrorKind::EtagMismatch { .. } => {
                "the instance config changed during update (HTTP 412)".into()
            }
            _ => e.to_string(),
        });
    }

    let after = client
        .instance(name)
        .await
        .map_err(|e| format!("reading back the limits: {e}"))?;
    active(&after, name)?;
    // An instance can be replaced after the PATCH while reusing its name.
    if before.created_at != after.created_at
        || before.config.get("volatile.uuid") != after.config.get("volatile.uuid")
    {
        return Err("the instance was replaced before the limits could be verified".into());
    }
    for (key, want) in &request.config {
        let local = after.config.get(key).map_or("", String::as_str);
        let got = effective(&after, key);
        if local != want || got != want {
            return Err(format!(
                "{key} did not hold: expected {want:?}, read local {local:?} and effective {got:?}"
            ));
        }
    }
    if let Some(key) = other {
        let got = effective(&after, key);
        if !got.is_empty() {
            return Err(format!("{key} still limits CPU: read {got:?}"));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
