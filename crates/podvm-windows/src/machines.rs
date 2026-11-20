//! TODO/machine.md T-1870: the record of a detached Windows or DOS guest.
//!
//! A machine lives in `machines/NAME/` beside the Windows catalog:
//! `machine.json` (this record), `disk.qcow2` (an overlay on the base or
//! golden image), `mailbox.img`, `vars.fd`, `serial.log` and
//! `keeper.log`. A keeper process owns the emulator: it starts it, writes
//! the pids into the record, bounds its lifetime and writes the end.
//!
//! ⛔ A pid alone names no process: the kernel gives a free pid again. The
//! record keeps the start time of each process (field 22 of
//! `/proc/PID/stat`), and a process is the one of the record only when
//! both match ([`alive`]).

use std::path::{Path, PathBuf};

/// The file of the record in the directory of a machine.
pub const RECORD: &str = "machine.json";

/// The default lifetime of a detached machine: 24 h (an estimate of a work
/// day and a night; `--timeout` changes it).
pub const DEFAULT_LIFETIME_SECS: u64 = 24 * 3600;

/// One machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    pub name: String,
    /// 64 hex digits, as docker's container id (TODO/machine.md T-2042);
    /// empty in a record written before it.
    pub id: String,
    /// The image as the caller wrote it, for example `windows:dos`.
    pub image: String,
    /// The base or golden image under the overlay.
    pub base: String,
    /// The kind of the boot line: `dos`, `mailbox` or `desktop`.
    pub flavor: String,
    /// Seconds since the epoch.
    pub created: u64,
    pub lifetime: u64,
    pub memory_mib: u64,
    pub keeper_pid: u32,
    pub keeper_start: u64,
    pub emulator_pid: u32,
    pub emulator_start: u64,
    /// Empty while the machine runs; else how it ended.
    pub ended: String,
    /// The forwards of `-p`, as `PROTO:IP:HOST:GUEST`, separated by `,`.
    pub forwards: String,
}

/// The directory of all machines: `PODVM_MACHINES_DIR` where it is set,
/// else `machines` beside the catalog.
pub fn root() -> PathBuf {
    if let Some(p) = std::env::var_os("PODVM_MACHINES_DIR")
        && !p.is_empty()
    {
        return PathBuf::from(p);
    }
    let base = crate::base_cache();
    base.parent().unwrap_or(Path::new(".")).join("machines")
}

/// A name of a machine: a letter or digit, then letters, digits, `_`, `.`
/// and `-`, at most 64 bytes (the rule of a docker name).
pub fn valid_name(name: &str) -> bool {
    let b = name.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && b[0].is_ascii_alphanumeric()
        && b.iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-'))
}

/// The start time of a process in clock ticks after the boot, from the
/// text of `/proc/PID/stat`: field 22, counted after the `)` that ends the
/// name (a name can hold spaces and parentheses).
pub fn parse_start(stat: &str) -> Option<u64> {
    let rest = &stat[stat.rfind(')')? + 1..];
    // After `)`: field 3 (state) is the first; field 22 is the 20th.
    rest.split_whitespace().nth(19)?.parse().ok()
}

/// The state of a process (field 3 of `/proc/PID/stat`): `Z` is a zombie.
pub fn parse_state(stat: &str) -> Option<char> {
    stat[stat.rfind(')')? + 1..]
        .split_whitespace()
        .next()?
        .chars()
        .next()
}

/// The start time of `pid`, or `None` where no such process runs: a
/// zombie has ended, though its entry stays until its parent reaps it.
pub fn start_of(pid: u32) -> Option<u64> {
    if pid == 0 {
        return None;
    }
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    if parse_state(&stat)? == 'Z' {
        return None;
    }
    parse_start(&stat)
}

/// The QMP socket of the machine `name` whose records are under `root`:
/// in the runtime directory (`XDG_RUNTIME_DIR`, else the temporary
/// directory), as a socket path holds at most 107 bytes and the directory
/// of a machine can be longer. A digest of `root` keeps two stores apart.
pub fn socket(root: &Path, name: &str) -> PathBuf {
    runtime_path(root, name, "qmp")
}

/// TODO/machine.md T-1899: the VNC socket of the machine `name`, beside
/// its QMP socket.
pub fn vnc_socket(root: &Path, name: &str) -> PathBuf {
    runtime_path(root, name, "vnc")
}

/// TODO/machine.md T-1750: the exec port of a Linux guest of the machine
/// tier (`run -d --podvm-tier=machine`).
pub fn exec_socket(root: &Path, name: &str) -> PathBuf {
    runtime_path(root, name, "exec")
}

/// TODO/machine.md T-1868: the socket of the virtio-serial channel of the
/// QEMU Guest Agent of a full Windows machine, beside its QMP socket.
pub fn qga_socket(root: &Path, name: &str) -> PathBuf {
    runtime_path(root, name, "qga")
}

/// A file of the machine `name` in the runtime directory, with the
/// extension `ext`.
fn runtime_path(root: &Path, name: &str, ext: &str) -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    // FNV-1a of the root: short, stable, and no crate.
    let mut h: u32 = 0x811c_9dc5;
    for b in root.as_os_str().as_encoded_bytes() {
        h = (h ^ u32::from(*b)).wrapping_mul(0x0100_0193);
    }
    dir.join(format!("podvm-m-{h:08x}-{name}.{ext}"))
}

/// True when `pid` runs and started at `start`: the process of the record.
pub fn alive(pid: u32, start: u64) -> bool {
    start != 0 && start_of(pid) == Some(start)
}

impl Record {
    /// True while the emulator of the record runs.
    pub fn running(&self) -> bool {
        alive(self.emulator_pid, self.emulator_start)
    }

    /// The JSON of the record: one object, each value a string.
    pub fn to_json(&self) -> String {
        let pairs: [(&str, String); 14] = [
            ("name", self.name.clone()),
            ("id", self.id.clone()),
            ("image", self.image.clone()),
            ("base", self.base.clone()),
            ("flavor", self.flavor.clone()),
            ("created", self.created.to_string()),
            ("lifetime", self.lifetime.to_string()),
            ("memory_mib", self.memory_mib.to_string()),
            ("keeper_pid", self.keeper_pid.to_string()),
            ("keeper_start", self.keeper_start.to_string()),
            ("emulator_pid", self.emulator_pid.to_string()),
            ("emulator_start", self.emulator_start.to_string()),
            ("ended", self.ended.clone()),
            ("forwards", self.forwards.clone()),
        ];
        let body: Vec<String> = pairs
            .iter()
            .map(|(k, v)| format!("{}:{}", crate::qmp::json_text(k), crate::qmp::json_text(v)))
            .collect();
        format!("{{{}}}\n", body.join(","))
    }

    /// The record of a JSON text that [`Record::to_json`] wrote.
    pub fn from_json(text: &str) -> Option<Record> {
        let members = flat_members(text)?;
        let s = |k: &str| {
            members
                .iter()
                .find(|(n, _)| n == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        let n = |k: &str| s(k).parse::<u64>().unwrap_or(0);
        let name = s("name");
        valid_name(&name).then(|| Record {
            name,
            id: s("id"),
            image: s("image"),
            base: s("base"),
            flavor: s("flavor"),
            created: n("created"),
            lifetime: n("lifetime"),
            memory_mib: n("memory_mib"),
            keeper_pid: n("keeper_pid") as u32,
            keeper_start: n("keeper_start"),
            emulator_pid: n("emulator_pid") as u32,
            emulator_start: n("emulator_start"),
            ended: s("ended"),
            forwards: s("forwards"),
        })
    }

    /// The STATUS column of `ps`.
    pub fn status(&self) -> String {
        if self.running() {
            "Up".to_string()
        } else if self.ended.is_empty() {
            "Exited (the emulator is gone)".to_string()
        } else {
            format!("Exited ({})", self.ended)
        }
    }
}

/// One JSON string at the start of `t` (after its `"`): the text and the
/// rest after the closing `"`.
fn string_at(t: &str) -> Option<(String, &str)> {
    let mut out = String::new();
    let mut it = t.char_indices();
    while let Some((i, c)) = it.next() {
        match c {
            '"' => return Some((out, &t[i + 1..])),
            '\\' => match it.next()?.1 {
                'u' => {
                    let hex: String = (0..4).filter_map(|_| it.next().map(|x| x.1)).collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                }
                'n' => out.push('\n'),
                't' => out.push('\t'),
                e => out.push(e),
            },
            c => out.push(c),
        }
    }
    None
}

/// The members of one flat JSON object whose values are strings, as
/// [`Record::to_json`] writes it; `None` for another shape.
fn flat_members(text: &str) -> Option<Vec<(String, String)>> {
    let mut rest = text.trim().strip_prefix('{')?.trim_start();
    let mut v = Vec::new();
    while let Some(r) = rest.strip_prefix('"') {
        let (k, r) = string_at(r)?;
        let r = r
            .trim_start()
            .strip_prefix(':')?
            .trim_start()
            .strip_prefix('"')?;
        let (val, r) = string_at(r)?;
        v.push((k, val));
        let r = r.trim_start();
        rest = r.strip_prefix(',').unwrap_or(r).trim_start();
    }
    rest.starts_with('}').then_some(v)
}

/// The directory of the machine `name` under `root`.
pub fn dir_in(root: &Path, name: &str) -> PathBuf {
    root.join(name)
}

/// Read the record of `name` under `root`.
pub fn read(root: &Path, name: &str) -> Option<Record> {
    Record::from_json(&std::fs::read_to_string(dir_in(root, name).join(RECORD)).ok()?)
}

/// Write the record under `root`, through a file beside it and a rename,
/// so that a reader never sees half a record.
pub fn write(root: &Path, r: &Record) -> std::io::Result<()> {
    let dir = dir_in(root, &r.name);
    let tmp = dir.join(format!("{RECORD}.tmp"));
    std::fs::write(&tmp, r.to_json())?;
    std::fs::rename(tmp, dir.join(RECORD))
}

/// Each machine under `root`, by name.
pub fn list(root: &Path) -> Vec<Record> {
    let mut v: Vec<Record> = std::fs::read_dir(root)
        .map(|d| {
            d.flatten()
                .filter_map(|e| read(root, &e.file_name().to_string_lossy()))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

impl Record {
    /// The id of the record, or its name where a record has none.
    pub fn id_or_name(&self) -> &str {
        if self.id.is_empty() {
            &self.name
        } else {
            &self.id
        }
    }
}

/// A new id of 64 hex digits from the random source of the kernel; where
/// it does not read, one from the clock, the pid and `name` (TODO/machine.md
/// T-2042: a run that cannot read the source still gets an id).
pub fn new_id(name: &str) -> String {
    use std::io::Read;
    let mut b = [0u8; 32];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .is_err()
    {
        use std::hash::{Hash, Hasher};
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        for (i, chunk) in b.chunks_mut(8).enumerate() {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (t, std::process::id(), name, i).hash(&mut h);
            chunk.copy_from_slice(&h.finish().to_be_bytes());
        }
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The shortest prefix of an id that finds a machine: the 12 digits of the
/// CONTAINER ID column of `ps`.
pub const ID_PREFIX: usize = 12;

/// The machine of `want` under `root` (TODO/machine.md T-2042): the one
/// with that name, else the one whose id starts with `want`, where `want`
/// is [`ID_PREFIX`] hex digits or more and one id only starts so.
pub fn find_in(root: &Path, want: &str) -> Option<Record> {
    if valid_name(want)
        && let Some(r) = read(root, want)
    {
        return Some(r);
    }
    if want.len() < ID_PREFIX || !want.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut hits = list(root).into_iter().filter(|r| r.id.starts_with(want));
    let one = hits.next()?;
    hits.next().is_none().then_some(one)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The record round-trips through its JSON, also with quotes and a
    /// backslash in a value.
    #[test]
    fn windows_lifecycle_record_round_trips() {
        let r = Record {
            name: "dos1".into(),
            id: new_id("dos1"),
            image: "windows:dos".into(),
            base: "/s/windows/freedos-base.img".into(),
            flavor: "dos".into(),
            created: 1_794_700_000,
            lifetime: DEFAULT_LIFETIME_SECS,
            memory_mib: 512,
            keeper_pid: 41,
            keeper_start: 900,
            emulator_pid: 42,
            emulator_start: 901,
            ended: "stopped by \"podvm stop\" C:\\".into(),
            forwards: "tcp:127.0.0.1:33890:3389".into(),
        };
        assert_eq!(Record::from_json(&r.to_json()), Some(r));
        assert_eq!(Record::from_json("{\"name\":\"../x\"}"), None);
    }

    /// TODO/machine.md T-2042: one lookup of a machine: by its name, else
    /// by its whole id or a unique prefix of 12 hex digits or more; a
    /// shorter, ambiguous or unknown one finds none.
    #[test]
    fn a_machine_is_found_by_its_name_or_its_id() {
        let root = std::env::temp_dir().join(format!("podvm-find-{}", &new_id("t")[..16]));
        let id_a = format!("abcdef012345{}", "0".repeat(52));
        let id_b = format!("abcdef012399{}", "1".repeat(52));
        for (name, id) in [("ma", id_a.as_str()), ("mb", id_b.as_str()), ("old", "")] {
            std::fs::create_dir_all(dir_in(&root, name)).unwrap();
            let r = Record {
                name: name.into(),
                id: id.into(),
                ..Default::default()
            };
            write(&root, &r).unwrap();
        }
        let cases: [(&str, Option<&str>); 8] = [
            ("ma", Some("ma")),
            ("old", Some("old")),
            (id_a.as_str(), Some("ma")),
            ("abcdef012345", Some("ma")),
            ("abcdef012399", Some("mb")),
            ("abcdef01234", None),
            ("ffffffffffff", None),
            ("nosuch", None),
        ];
        for (want, got) in cases {
            assert_eq!(
                find_in(&root, want).map(|r| r.name),
                got.map(String::from),
                "{want}"
            );
        }
        assert_eq!(find_in(&root, "old").unwrap().id_or_name(), "old");
        assert_eq!(new_id("x").len(), 64);
        assert_ne!(new_id("x"), new_id("x"));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Field 22 is read after the last `)`, so a name with spaces and
    /// parentheses does not move it.
    #[test]
    fn windows_lifecycle_start_time_is_field_22() {
        let stat = "4242 (qemu (x) y) S 1 4242 4242 0 -1 4194560 100 0 0 0 5 6 0 0 20 0 3 0 777123 1000 50";
        assert_eq!(parse_start(stat), Some(777_123));
        assert_eq!(parse_start("garbage"), None);
        assert_eq!(parse_state(stat), Some('S'));
        assert_eq!(parse_state("9 (z) Z 1 9"), Some('Z'));
        let me = std::process::id();
        let start = start_of(me).expect("the test process has a stat");
        assert!(alive(me, start));
        assert!(
            !alive(me, start + 1),
            "a pid with another start time is another process"
        );
        assert!(!alive(0, start));
    }

    /// The rule of a name, and the status of a machine whose emulator is
    /// gone.
    #[test]
    fn windows_lifecycle_names_and_status() {
        for ok in ["dos1", "a", "win-11.test_2"] {
            assert!(valid_name(ok), "{ok}");
        }
        for bad in ["", "-a", "../x", "a/b", "a b", &"x".repeat(65)] {
            assert!(!valid_name(bad), "{bad}");
        }
        let mut r = Record {
            name: "gone".into(),
            emulator_pid: u32::MAX - 1,
            emulator_start: 5,
            ..Default::default()
        };
        assert_eq!(r.status(), "Exited (the emulator is gone)");
        r.ended = "lifetime of 60 s".into();
        assert_eq!(r.status(), "Exited (lifetime of 60 s)");
    }

    /// The socket of a machine fits the limit of a socket path for each
    /// valid name, and two stores give two sockets.
    #[test]
    fn windows_lifecycle_socket_is_short() {
        let long = Path::new(
            "/home/a-very-long-user-name/and/a/deep/store/path/of/podvm/windows/machines",
        );
        let name = "x".repeat(64);
        let s = socket(long, &name);
        assert!(
            s.as_os_str().len() < 108 || std::env::var_os("XDG_RUNTIME_DIR").is_some(),
            "{}",
            s.display()
        );
        assert_ne!(socket(long, "a"), socket(Path::new("/other"), "a"));
    }

    /// The list reads each record under the root and skips a directory with
    /// none.
    #[test]
    fn windows_lifecycle_list_reads_each_record() {
        let root = std::env::temp_dir().join(format!("podvm-machines-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for n in ["b", "a"] {
            std::fs::create_dir_all(dir_in(&root, n)).unwrap();
            write(
                &root,
                &Record {
                    name: n.into(),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        std::fs::create_dir_all(root.join("empty")).unwrap();
        let names: Vec<String> = list(&root).into_iter().map(|r| r.name).collect();
        assert_eq!(names, ["a", "b"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
