//! `--device HOST[:GUEST[:PERMS]]` on a rung of this host.
//!
//! TODO/enter.md T-1939 (#473, OR-B41-08): the descriptor serve of
//! `pod-enter` answers an open of the exact guest path only, so `stat`,
//! `access`, a listing and an open of the kernel saw no node, and a write
//! that creates made a file in the tree. A device that reads goes as a
//! volume of one file now: a bind on the namespace rung and an entry of the
//! path table on the userland rung, so each call of the payload sees the
//! host node at the guest path. A device with `r` and no `w` is a read-only
//! volume. A rung that serves no volume refuses the device by name
//! (`volume::refuse_unserved`), as it refuses a `-v`. A device with `w`
//! and no `r` keeps the descriptor serve: a bind cannot refuse a read.

use super::*;

/// Move each `--device` that reads from `o.devices` to `o.volumes`, and
/// note its guest path in `o.device_guests` for the line of a refusal.
/// The guest tiers refused each device before this call.
pub(super) fn fold(o: &mut Opts) {
    let mut kept = Vec::new();
    for spec in std::mem::take(&mut o.devices) {
        // `parse` refused each malformed shape as a flag error.
        let m = match podvm_enter::device::parse(&spec) {
            Ok(m) if m.read => m,
            _ => {
                kept.push(spec);
                continue;
            }
        };
        let guest = crate::lifecycle::volume::guest_spelling(&m.guest);
        if !m.write {
            o.ro_volumes.push(guest.clone());
        }
        o.volume_given.push(m.host.clone());
        o.device_guests.push(guest.clone());
        o.volumes.push(podvm_enter::userland::Volume {
            host: m.host,
            guest,
        });
    }
    o.devices = kept;
}

/// TODO/enter.md T-1939: a bind with `MS_RDONLY` does not refuse a write
/// to a device node (the kernel checks a read-only mount for a file, a
/// directory and a link only), so the namespace rung cannot keep a device
/// node read-only, and refuses it by name. The userland rung serves the
/// read-only table of the interposer, which answers EROFS.
pub(super) fn check_rung(verb: &str, o: &Opts, rung: &str) -> Result<(), i32> {
    let Some(why) = read_only_node(o, rung) else {
        return Ok(());
    };
    eprintln!("podvm {verb}: {why}");
    Err(podvm_image::error::EXIT_RUNTIME_ERROR)
}

/// The line of [`check_rung`], or none.
pub(super) fn read_only_node(o: &Opts, rung: &str) -> Option<String> {
    use std::os::unix::fs::FileTypeExt;
    if rung != "namespace" {
        return None;
    }
    let node = |host: &str| {
        std::fs::metadata(host).is_ok_and(|m| {
            let t = m.file_type();
            t.is_char_device() || t.is_block_device()
        })
    };
    let v = o.volumes.iter().find(|v| {
        o.device_guests.contains(&v.guest) && o.ro_volumes.contains(&v.guest) && node(&v.host)
    })?;
    Some(format!(
        "--device {}:{}:r is refused on the namespace rung: it binds the device node, and a \
         read-only bind does not refuse a write to a device node; give rw, or leave out the \
         device (TODO/enter.md T-1939)",
        v.host, v.guest
    ))
}

/// The device spec of the devices that stay on the descriptor serve, in
/// `env`, and the devices for the banner. TODO/enter.md T-0501: the entering process (foreground `run`, the
/// launcher for `start`) opens the host paths before the root changes and
/// serves duplicates where the interposer holds.
pub(super) fn push_spec(
    verb: &str,
    o: &Opts,
    env: &mut Vec<String>,
) -> Result<Vec<podvm_enter::device::Mapping>, i32> {
    let mut devices = Vec::with_capacity(o.devices.len());
    for d in &o.devices {
        match podvm_enter::device::parse(d) {
            Ok(m) => devices.push(m),
            Err(why) => {
                eprintln!("podvm {verb}: --device {why}");
                return Err(EXIT_FLAG_ERROR);
            }
        }
    }
    podvm_enter::device::push_spec(env, &devices);
    Ok(devices)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The host and the guest path of a volume, or none.
    type Pair = Option<(&'static str, &'static str)>;

    /// One rule for each row: a device that reads is a volume of its guest
    /// path (read-only with no `w`), named as a device; a device that only
    /// writes stays on the descriptor serve.
    #[test]
    fn a_device_that_reads_goes_as_a_volume() {
        // (spec, volume (host, guest), read-only, kept on the serve)
        let rows: [(&str, Pair, bool, bool); 5] = [
            (
                "/dev/null:/dev/mynull",
                Some(("/dev/null", "/dev/mynull")),
                false,
                false,
            ),
            (
                "/dev/zero:/dev/z:r",
                Some(("/dev/zero", "/dev/z")),
                true,
                false,
            ),
            ("/dev/fuse", Some(("/dev/fuse", "/dev/fuse")), false, false),
            (
                "/dev/zero:/dev/z/:rwm",
                Some(("/dev/zero", "/dev/z")),
                false,
                false,
            ),
            ("/dev/null:/dev/w:w", None, false, true),
        ];
        for (spec, volume, ro, kept) in rows {
            let mut o = Opts::new();
            o.devices.push(spec.to_string());
            fold(&mut o);
            let got = o
                .volumes
                .first()
                .map(|v| (v.host.as_str(), v.guest.as_str()));
            assert_eq!(got, volume, "{spec}");
            assert_eq!(!o.ro_volumes.is_empty(), ro, "{spec}");
            assert_eq!(o.devices.is_empty(), !kept, "{spec}");
            assert_eq!(o.device_guests.len(), o.volumes.len(), "{spec}");
            assert_eq!(o.volume_given.len(), o.volumes.len(), "{spec}");
        }
    }

    /// One rule for each row: only a read-only device node on the namespace
    /// rung is refused; a read-only file binds read-only, and the userland
    /// rung serves its read-only table.
    #[test]
    fn a_read_only_device_node_is_refused_on_the_namespace_rung() {
        let exe = std::env::current_exe().unwrap();
        let file = format!("{}:/opt/f:r", exe.to_str().unwrap());
        // (spec, rung, refused)
        let rows = [
            ("/dev/zero:/dev/z:r", "namespace", true),
            ("/dev/zero:/dev/z:rw", "namespace", false),
            ("/dev/zero:/dev/z:r", "userland", false),
            (file.as_str(), "namespace", false),
        ];
        for (spec, rung, refused) in rows {
            let mut o = Opts::new();
            o.devices.push(spec.to_string());
            fold(&mut o);
            assert_eq!(
                check_rung("run", &o, rung).is_err(),
                refused,
                "{spec} {rung}"
            );
        }
    }
}
