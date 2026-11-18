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
        assert_eq!(in_volume_of(&t, host), want, "{}", String::from_utf8_lossy(host));
    }
}
