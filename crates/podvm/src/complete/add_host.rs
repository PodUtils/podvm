//! `--add-host` of `run`, `create` and `exec` (TODO/cli.md T-1828).

use super::Ask;
use podvm_image::error::EXIT_FLAG_ERROR;

/// Parse `--add-host name:ip` and friends out of the shared argument surface.
///
/// ⚠ Returned as an error code rather than a panic so the caller's own usage
/// text is what the user sees.
pub fn add_host(ask: &mut Ask, verb: &str, value: &str) -> Result<(), i32> {
    if let Err(why) = check_add_host(value) {
        eprintln!(
            "podvm {verb}: --add-host takes name:ip, and {value:?} {why} (TODO/cli.md T-1828)"
        );
        return Err(EXIT_FLAG_ERROR);
    }
    ask.add_hosts.push(normal(value));
    Ok(())
}

/// PodUtils/podvm#480: the entry that `/etc/hosts` takes. `host-gateway` is
/// the address at which the payload reaches this machine: each rung of a
/// run shares the network of this machine, so it is 127.0.0.1 (the machine
/// tier refuses `--add-host`). An IPv6 address loses its brackets.
pub fn normal(value: &str) -> String {
    let Some((name, ip)) = value.split_once(':') else {
        return value.to_string();
    };
    let ip = if ip == "host-gateway" {
        "127.0.0.1"
    } else {
        ip.strip_prefix('[')
            .and_then(|i| i.strip_suffix(']'))
            .unwrap_or(ip)
    };
    format!("{name}:{ip}")
}

#[cfg(test)]
#[test]
fn the_entry_is_an_address_of_the_hosts_file() {
    assert_eq!(normal("bar:host-gateway"), "bar:127.0.0.1");
    assert_eq!(normal("six:[::1]"), "six:::1");
    assert_eq!(normal("a:10.0.0.1"), "a:10.0.0.1");
}

/// TODO/cli.md T-1828: the name is a host name and the address an IPv4 or
/// IPv6 address (in brackets or not), or docker's `host-gateway`. A value
/// with a space or a newline broke the whole fixup of `/etc/hosts`.
pub fn check_add_host(value: &str) -> Result<(), String> {
    let Some((name, ip)) = value.split_once(':') else {
        return Err("has no `:`".into());
    };
    let name_ok = !name.is_empty()
        && name.len() <= 253
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'_');
    if !name_ok {
        return Err(format!("has no host name before the `:` ({name:?})"));
    }
    let bare = ip
        .strip_prefix('[')
        .and_then(|i| i.strip_suffix(']'))
        .unwrap_or(ip);
    if ip == "host-gateway" || bare.parse::<std::net::IpAddr>().is_ok() {
        return Ok(());
    }
    Err(format!("has no IP address after the `:` ({ip:?})"))
}
