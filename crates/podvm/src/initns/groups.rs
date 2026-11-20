//! TODO/enter.md T-2002: the supplementary groups of the command of a run
//! with the init of the image, read from the files of the image.

use std::path::Path;

/// The most bytes of `/etc/passwd` or `/etc/group` of an image that podvm
/// reads for the groups of a user.
const MAX_ACCOUNTS: u64 = 4 << 20;
/// The most supplementary groups that a command gets.
const MAX_GROUPS: usize = 256;

/// TODO/enter.md T-2002: the supplementary groups that the image gives
/// the user of `ids`, as `login` and docker read them: each group of
/// `group` (the text of `/etc/group`) that lists, as a member, a name of
/// `passwd` with the uid and the primary gid of `ids`. A user whose `--user`
/// names another group than its primary one gets none, as in docker. The
/// primary group is not in the list. A line that is not a record is not
/// read.
pub fn groups_of(passwd: &str, group: &str, ids: (u32, u32)) -> Vec<u32> {
    let id = |f: Option<&str>| f.and_then(|x| x.trim().parse::<u32>().ok());
    let names: Vec<&str> = passwd
        .lines()
        .filter_map(|l| {
            let mut f = l.split(':');
            let name = f.next().filter(|n| !n.is_empty() && !n.starts_with('#'))?;
            f.next()?;
            (id(f.next()) == Some(ids.0) && id(f.next()) == Some(ids.1)).then_some(name)
        })
        .collect();
    let mut out: Vec<u32> = group
        .lines()
        .filter_map(|l| {
            let mut f = l.split(':');
            f.next().filter(|n| !n.is_empty() && !n.starts_with('#'))?;
            f.next()?;
            let gid = id(f.next())?;
            let members = f.next()?;
            (gid != ids.1 && members.split(',').any(|m| names.contains(&m.trim()))).then_some(gid)
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out.truncate(MAX_GROUPS);
    out
}

/// [`groups_of`] for the tree `rootfs`. A file that is missing, too large
/// or not text gives the groups that its readable part names, or none: the
/// command then runs with its primary group.
pub fn image_groups(rootfs: &str, ids: (u32, u32)) -> Vec<u32> {
    use std::io::Read;
    let read = |name: &str| {
        let mut bytes = Vec::new();
        if let Ok(f) = std::fs::File::open(Path::new(rootfs).join(name)) {
            let _ = f.take(MAX_ACCOUNTS).read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    };
    groups_of(&read("etc/passwd"), &read("etc/group"), ids)
}
