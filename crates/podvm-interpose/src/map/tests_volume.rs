//! OS-02 of the third outside pass of 2026-11-18 (TODO/enter.md T-1979).

use super::*;

fn env_of<const N: usize>(entries: [&'static [u8]; N]) -> Vec<*const c_char> {
    let mut v: Vec<*const c_char> = entries
        .iter()
        .map(|e| e.as_ptr() as *const c_char)
        .collect();
    v.push(core::ptr::null());
    v
}

/// OS-02: a link made in a volume keeps the guest's target; one made in the
/// tree of the run (the pair of `/`) or in a device pair's file does not
/// count as a volume only where its pair is `/`.
#[test]
fn a_volume_is_a_pair_whose_guest_side_is_not_the_root() {
    let env = env_of([b"PODVM_MAPS=/:/store/tree,/vol:/host/v,/dev/null:/dev/null\0"]);
    let t = unsafe { parse(env.as_ptr()) };
    for (host, want) in [
        (&b"/host/v/a1"[..], true),
        (b"/host/v", true),
        (b"/host/v2/a1", false),
        (b"/store/tree/tmp/a1", false),
        (b"/elsewhere/a1", false),
        (b"rel/a1", false),
    ] {
        assert_eq!(
            in_volume_of(&t, host),
            want,
            "{}",
            String::from_utf8_lossy(host)
        );
    }
}

/// TODO/enter.md T-2040: a host path under the host side of a volume reads
/// as its guest path (the longest host side decides); the rootfs, a host
/// side of `/`, a sibling and the never-rewrite paths keep their names.
#[test]
fn a_host_path_of_a_volume_reads_as_its_guest_path() {
    let env = env_of([
        b"PODVM_MAPS=/:/store/tree,/cfg:/host/v,/b:/host/v/sub/,/hr:/,/dev/null:/dev/null\0",
    ]);
    let t = unsafe { parse(env.as_ptr()) };
    for (host, want) in [
        (&b"/host/v/f"[..], Some(&b"/cfg/f"[..])),
        (b"/host/v", Some(b"/cfg")),
        (b"/host/v/sub/x", Some(b"/b/x")),
        (b"/host/v/sub", Some(b"/b")),
        (b"/dev/null", Some(b"/dev/null")),
        (b"/host/v2/f", None),
        (b"/store/tree/etc/hostname", None),
        (b"/etc/hostname", None),
        (b"/proc/self/fd/3", None),
        (b"cfg/f", None),
    ] {
        let mut buf = [0u8; OUT];
        buf[..host.len()].copy_from_slice(host);
        let got = unsafe { to_volume_guest(&t, &mut buf, host.len()) };
        assert_eq!(
            got.map(|m| &buf[..m]),
            want,
            "{}",
            String::from_utf8_lossy(host)
        );
        if got.is_none() {
            assert_eq!(&buf[..host.len()], host, "unchanged");
        }
    }
}
