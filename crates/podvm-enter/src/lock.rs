//! TODO/enter.md T-2105 (PodUtils/podrunner#378): the payload of the
//! namespace rung in a user namespace under the one that made its mounts.
//!
//! A caller that is uid 0 of a user namespace that is not the initial one
//! makes the mounts of the run in that namespace: podvm's pid 2 of a
//! foreground run (its binds of the volumes), the launcher of `run -d`
//! with an overlay volume (its layers), and pod-enter (the tmpfs on `/tmp`,
//! `/proc`, `--tmpfs GUEST:size=N`, its binds). A payload that is uid 0 of
//! the same namespace holds `CAP_SYS_ADMIN` over each of these mounts.
//! Measured: it remounted a `:ro` volume writable and wrote the host file,
//! gave a sized tmpfs another size, and unmounted a volume.
//!
//! With the ids `0:0` in `Plan::userns_ids`, pod-enter opens a nested user
//! namespace after its mounts and before the root change, with the maps
//! `0 0 1` (podcrates `v0.63.0`, `ns_user::nested_for`). The payload stays
//! uid 0 with each capability of the nested namespace, and holds none over
//! the mount namespace of the run: each mount, remount and unmount there
//! fails with `EPERM`. A mount namespace that the payload opens copies the
//! mounts into a less privileged user namespace, so the kernel locks their
//! flags (`MNT_LOCK_READONLY` and the others) and the mounts under its root.
//!
//! ⛔ Only where the map of the caller's namespace holds one id, uid 0: the
//! maps `0 0 1` then take no id from the payload. A map of more ids (the
//! subordinate range of T-1911, a container of a range) and the initial
//! namespace keep the entry as before.

/// The ids of `Plan::userns_ids` for a plan with the ids `ids`, in a process
/// whose `/proc/self/uid_map` reads `uid_map` (`None`: the read failed).
pub fn ids_for(ids: Option<(u32, u32)>, uid_map: Option<&str>) -> Option<(u32, u32)> {
    ids.or_else(|| uid_map.filter(|m| one_id(m)).map(|_| (0, 0)))
}

/// True for a map of one line that maps one id to uid 0.
fn one_id(map: &str) -> bool {
    let mut lines = map.lines().filter(|l| !l.trim().is_empty());
    let (Some(line), None) = (lines.next(), lines.next()) else {
        return false;
    };
    let words: Vec<&str> = line.split_whitespace().collect();
    words.len() == 3 && words[0] == "0" && words[2] == "1" && words[1].parse::<u32>().is_ok()
}

/// [`ids_for`] of this process. A map that cannot be read leaves `ids`.
pub fn payload_ids(ids: Option<(u32, u32)>) -> Option<(u32, u32)> {
    let map = std::fs::read_to_string("/proc/self/uid_map").ok();
    ids_for(ids, map.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One rule for each row: the ids of a plan stay; a map of one id to
    /// uid 0 gives `0:0`; each other map, and a failed read, gives none.
    #[test]
    fn locked_ids_of_each_map() {
        let one = "         0       1000          1\n";
        type Ids = Option<(u32, u32)>;
        let rows: [(Ids, Option<&str>, Ids); 10] = [
            (None, Some(one), Some((0, 0))),
            (Some((1000, 1000)), Some(one), Some((1000, 1000))),
            (None, Some("0 0 4294967295\n"), None),
            (None, Some("0 1000 1\n1 100000 65536\n"), None),
            (
                Some((33, 33)),
                Some("0 1000 1\n1 100000 65536\n"),
                Some((33, 33)),
            ),
            (None, Some("5 1000 1\n"), None),
            (None, Some("0 1000 2\n"), None),
            (None, Some("0 x 1\n"), None),
            (None, Some(""), None),
            (None, None, None),
        ];
        for (ids, map, want) in rows {
            assert_eq!(ids_for(ids, map), want, "{ids:?} {map:?}");
        }
    }
}
