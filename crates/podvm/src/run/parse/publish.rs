//! TODO/machine.md T-1869: `-p, --publish` of a detached Windows guest
//! (`run -d -p 3389:3389 windows:2025`). The guest has a user-mode NIC, and
//! a publish is a `hostfwd` of QEMU: `[IP:]HOST:GUEST[/tcp|/udp]`, or
//! `GUEST` alone for a free port of the host. The address is 127.0.0.1
//! where none is written; `0.0.0.0` only where the caller writes it. Each
//! other run refuses `-p`: its payload shares the network of this machine.

use std::net::Ipv4Addr;

use podvm_image::error::EXIT_FLAG_ERROR;
use podvm_windows::plan::{Forward, Proto};

/// The text of a forward in the record of a machine:
/// `PROTO:IP:HOST:GUEST`.
pub(crate) fn text(f: &Forward) -> String {
    format!(
        "{}:{}:{}:{}",
        f.proto.word(),
        f.host,
        f.host_port,
        f.guest_port
    )
}

/// The forward of a text that [`text`] wrote.
pub(crate) fn forward_of(t: &str) -> Option<Forward> {
    let mut it = t.split(':');
    let proto = match it.next()? {
        "tcp" => Proto::Tcp,
        "udp" => Proto::Udp,
        _ => return None,
    };
    let host: Ipv4Addr = it.next()?.parse().ok()?;
    let host_port = it.next()?.parse().ok()?;
    let guest_port = it.next()?.parse().ok()?;
    Some(Forward {
        proto,
        host,
        host_port,
        guest_port,
    })
}

/// A port of the host that no one listens on now, on `ip`.
fn free_port(ip: Ipv4Addr) -> Option<u16> {
    std::net::TcpListener::bind((ip, 0))
        .ok()?
        .local_addr()
        .ok()
        .map(|a| a.port())
}

/// One value of `-p`, as the text of a forward.
pub(crate) fn parse(verb: &str, spec: &str) -> Result<String, i32> {
    parse_forward(spec).map(|f| text(&f)).map_err(|why| {
        eprintln!("podvm {verb}: -p {spec:?}: {why} (TODO/machine.md T-1869)");
        EXIT_FLAG_ERROR
    })
}

fn parse_forward(spec: &str) -> Result<Forward, String> {
    let (body, proto) = match spec.rsplit_once('/') {
        Some((b, "tcp")) => (b, Proto::Tcp),
        Some((b, "udp")) => (b, Proto::Udp),
        Some((_, p)) => return Err(format!("the protocol {p:?} is not tcp or udp")),
        None => (spec, Proto::Tcp),
    };
    // TODO/gate.md T-1947: the rule `PUBLISH` of `crate::number`.
    let port = |s: &str| -> Result<u16, String> {
        crate::number::check(&crate::number::PUBLISH, s).map(|p| p as u16)
    };
    let parts: Vec<&str> = body.split(':').collect();
    let (host, host_port, guest_port) = match parts.as_slice() {
        [g] => {
            let ip = Ipv4Addr::LOCALHOST;
            let h = free_port(ip).ok_or("no free port of the host")?;
            (ip, h, port(g)?)
        }
        [h, g] => (Ipv4Addr::LOCALHOST, port(h)?, port(g)?),
        [ip, h, g] => {
            let ip: Ipv4Addr = ip
                .parse()
                .map_err(|_| format!("{ip:?} is no IPv4 address"))?;
            (ip, port(h)?, port(g)?)
        }
        _ => return Err("takes [IP:]HOST:GUEST[/tcp|/udp] or GUEST".to_string()),
    };
    Ok(Forward {
        proto,
        host,
        host_port,
        guest_port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The address is the loopback where none is written, and the text
    /// round-trips into the `hostfwd` of QEMU.
    #[test]
    fn windows_ports_default_to_the_loopback() {
        let f = parse_forward("33890:3389").unwrap();
        assert_eq!(f.host, Ipv4Addr::LOCALHOST);
        assert_eq!(f.spec(), "tcp:127.0.0.1:33890-:3389");
        assert_eq!(forward_of(&text(&f)), Some(f));
        let any = parse_forward("0.0.0.0:8080:80/udp").unwrap();
        assert_eq!(any.spec(), "udp:0.0.0.0:8080-:80");
        let free = parse_forward("3389").unwrap();
        assert_eq!(free.guest_port, 3389);
        assert!(free.host_port > 0 && free.host == Ipv4Addr::LOCALHOST);
    }

    /// A bad value is a flag error that names the part.
    #[test]
    fn windows_ports_refuse_a_bad_value() {
        for (bad, says) in [
            ("0:3389", "takes a port"),
            ("70000:1", "takes a port"),
            ("1:2/sctp", "not tcp or udp"),
            ("x.y:1:2", "no IPv4"),
            ("1:2:3:4", "takes"),
        ] {
            let e = parse_forward(bad).unwrap_err();
            assert!(e.contains(says), "{bad}: {e}");
        }
    }

    /// The forward of a desktop plan is the `hostfwd` of the NIC.
    #[test]
    fn windows_ports_reach_the_hostfwd_line() {
        let f = parse_forward("33890:3389").unwrap();
        let o = podvm_windows::options::desktop(podvm_windows::Accel::Tcg, vec![f]);
        let d = std::path::PathBuf::from("/run/t");
        let argv = podvm_windows::argv(&podvm_windows::Plan {
            emulator: "qemu-system-x86_64".into(),
            accel: podvm_windows::Accel::Tcg,
            share: "/usr/share/qemu".into(),
            firmware_code: "/c.fd".into(),
            firmware_vars: d.join("v.fd"),
            root: d.join("r.qcow2"),
            mailbox: d.join("m.img"),
            serial: d.join("s.log"),
            monitor: d.join("q.sock"),
            memory_mib: 4096,
            cpus: 2,
            emu_args: Vec::new(),
            options: o,
        });
        assert!(
            argv.iter()
                .any(|a| a.contains("hostfwd=tcp:127.0.0.1:33890-:3389")),
            "{argv:?}"
        );
    }
}
