//! What an entry opened, as one table (TODO/cli.md T-2053, OQ-07 of the
//! sixth outside pass).
//!
//! ⛔ The banner of the namespace rung said "this mode does NOT provide:
//! user, pid or network isolation, or a /proc filesystem" and "no /proc is
//! mounted in the payload root", and `--strict` warned that "/proc is not
//! mounted in the root", while the same run had a PID namespace and its own
//! `/proc`. `inspect` gave `.Exec.Shares` as `filesystem` for a container
//! whose `exec` joins its mount and PID namespaces. Each text came from the
//! name of the rung, not from the run.
//!
//! ⭐ Each statement about the isolation of a run now reads [`rows`] and the
//! facts of one entry ([`Opened`], made by [`of_entry`] from what podvm
//! opens and what `pod-enter` mounts): the cells and the "does NOT provide"
//! line of the banner, its `/proc` line, the `entry_mounts_proc` of the
//! completion (the mtab warning of `--strict`), `.Exec.Mode` and
//! `.Exec.Shares` of `inspect`, the line of `exec`, and the ISOLATION
//! section of the manual ([`section`]).

use crate::select::Rung;

/// One part of the isolation of an entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    User,
    Pid,
    Uts,
    Net,
    Mount,
    Proc,
}

/// How `exec` into a running container of the run takes a part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Join {
    /// `exec` joins it (the mount namespace, T-1765; the user and the PID
    /// namespace, T-1910 and pod-enter's `spawn_joined`).
    Always,
    /// `exec` joins it only with the user namespace of the run: podvm's
    /// own join of T-1910 takes the UTS namespace, pod-enter's does not.
    WithUser,
    /// Not a namespace: `exec` sees it through the mount namespace.
    Seen,
}

/// One row of the table.
pub struct Row {
    pub part: Part,
    /// The word of the part in `namespaces:`, `.Exec.Shares` and the record.
    pub word: &'static str,
    /// What the part gives where the entry opened it.
    pub gives: &'static str,
    /// The words of the "does NOT provide" line where the entry did not.
    pub not: &'static str,
    pub join: Join,
}

/// The parts, in the order of each list that the table gives.
pub const PARTS: [Part; 6] = [
    Part::User,
    Part::Pid,
    Part::Uts,
    Part::Net,
    Part::Mount,
    Part::Proc,
];

/// The row of a part. ⚠ A `match`, not a static table of `&str`: each
/// pointer of a static table is a relocation of the static binary
/// (call C24, the size ceiling).
pub fn row(part: Part) -> Row {
    let (word, gives, not, join) = match part {
        Part::User => (
            "user",
            "a user namespace, where the caller is uid 0",
            "user isolation",
            Join::Always,
        ),
        Part::Pid => (
            "pid",
            "a PID namespace: the payload sees only the processes of the run",
            "pid isolation",
            Join::Always,
        ),
        Part::Uts => (
            "uts",
            "a UTS namespace: --hostname names it",
            "a host name of its own",
            Join::WithUser,
        ),
        Part::Net => (
            "net",
            "a network namespace: no entry opens one",
            "network isolation",
            Join::Always,
        ),
        Part::Mount => (
            "mount",
            "a mount namespace, with a private tmpfs on /tmp",
            "mount isolation",
            Join::Always,
        ),
        Part::Proc => (
            "proc",
            "a /proc in the root, of the PID namespace of the payload",
            "a /proc filesystem",
            Join::Seen,
        ),
    };
    Row {
        part,
        word,
        gives,
        not,
        join,
    }
}

/// The table: the row of each part of [`PARTS`].
pub fn rows() -> impl Iterator<Item = Row> {
    PARTS.into_iter().map(row)
}

/// The parts that one entry opened.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Opened {
    bits: u8,
}

/// The bit of a part: its place in [`PARTS`], the order of the enum.
fn bit(p: Part) -> u8 {
    1 << p as u8
}

impl Opened {
    /// Nothing opened: the chroot rung and the userland rung.
    pub const NONE: Opened = Opened { bits: 0 };

    pub fn of(parts: &[Part]) -> Opened {
        Opened {
            bits: parts.iter().fold(0, |b, p| b | bit(*p)),
        }
    }

    pub fn has(self, p: Part) -> bool {
        self.bits & bit(p) != 0
    }

    fn rows(self) -> impl Iterator<Item = Row> {
        rows().filter(move |r| self.has(r.part))
    }

    /// The words of the parts, in the order of [`rows`], one space
    /// between two: the record keeps them.
    pub fn words(self) -> String {
        self.rows().map(|r| r.word).collect::<Vec<_>>().join(" ")
    }

    /// The parts of the words of a record. An unknown word (a record of
    /// another podvm) is left out.
    pub fn from_words(words: &str) -> Opened {
        let bits = rows()
            .filter(|r| words.split(' ').any(|w| w == r.word))
            .fold(0, |b, r| b | bit(r.part));
        Opened { bits }
    }

    /// The `namespaces:` cell of the banner, or `None` where the entry
    /// opened no namespace (the cell of the probe then stands).
    pub fn namespaces_cell(self) -> Option<String> {
        let ns: Vec<&str> = self
            .rows()
            .filter(|r| r.part != Part::Proc)
            .map(|r| r.word)
            .collect();
        match ns.as_slice() {
            [] => None,
            ["mount"] => Some("mount-only".to_string()),
            _ => Some(ns.join("+")),
        }
    }

    /// The `pids:` cell of the banner.
    pub fn pids_cell(self) -> &'static str {
        if self.has(Part::Pid) {
            "private"
        } else {
            "host-shared"
        }
    }

    /// The parts that `pod_hostprobe` reads for the "does NOT provide" line
    /// of the namespace rung (`Rung::must_never_claim_with`).
    pub fn hostprobe(self) -> pod_hostprobe::select::Opened {
        pod_hostprobe::select::Opened {
            user: self.has(Part::User),
            pid: self.has(Part::Pid),
            proc: self.has(Part::Proc),
        }
    }

    /// The "does NOT provide" words of `rung` for this entry: the rows that
    /// the namespace rung did not open, in the words of `pod_hostprobe`;
    /// the text of each other rung as it is.
    pub fn never_claim(self, rung: Rung) -> String {
        rung.must_never_claim_with(self.hostprobe())
    }

    /// The line of the banner where the payload has a `/proc` and no PID
    /// namespace: that `/proc` lists the processes of the host.
    pub fn line(self) -> Option<&'static str> {
        (self.has(Part::Proc) && !self.has(Part::Pid)).then_some(
            "the /proc of the payload lists the processes of the host: the run has no \
             PID namespace (TODO/cli.md T-2053)\n",
        )
    }

    /// `.Exec.Mode` of `inspect` for a container of `rung`: `join` where
    /// `exec` into it while it runs joins its mount namespace, else
    /// `fresh-chroot`, a new entry that shares the tree only.
    pub fn exec_mode(self, rung: &str) -> &'static str {
        if rung == "namespace" && self.has(Part::Mount) {
            "join"
        } else {
            FRESH
        }
    }

    /// `.Exec.Shares` of `inspect`: `filesystem`, then each namespace that
    /// `exec` into the running container joins.
    pub fn exec_shares(self, rung: &str) -> String {
        let mut out = vec![SHARES];
        if self.exec_mode(rung) == "join" {
            let user = self.has(Part::User);
            out.extend(
                self.rows()
                    .filter(|r| match r.join {
                        Join::Always => true,
                        Join::WithUser => user,
                        Join::Seen => false,
                    })
                    .map(|r| r.word),
            );
        }
        out.join("+")
    }
}

/// `.Exec.Mode` of a new entry: an image, or a container of a rung that
/// opens no mount namespace.
pub const FRESH: &str = "fresh-chroot";
/// `.Exec.Shares` of a new entry: the tree, and nothing else.
pub const SHARES: &str = "filesystem";

/// What podvm knows of an entry before it enters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// podvm opened its user, PID and UTS namespace for this foreground run
    /// (`userns::active`, T-1909).
    pub userns_active: bool,
    /// The caller is uid 0 of the initial user namespace: pod-enter makes
    /// no user namespace for it.
    pub host_root: bool,
    /// `run -d` or `create`: the launcher enters with a PID namespace of
    /// pod-enter (`Plan::pid_namespace`, T-1930).
    pub detached: bool,
    /// `--hostname`: a UTS namespace (T-1908).
    pub hostname: bool,
    /// The image has a `/proc` directory: pod-enter mounts a procfs there
    /// (or binds the one of its caller), and nothing where it has none.
    pub proc_dir: bool,
}

/// The parts that an entry of `rung` opens, from `f`.
pub fn of_entry(rung: Rung, f: Facts) -> Opened {
    if rung != Rung::Namespace {
        return Opened::NONE;
    }
    let mut parts = vec![Part::Mount];
    if f.userns_active || !f.host_root {
        parts.push(Part::User);
    }
    if f.userns_active || f.detached {
        parts.push(Part::Pid);
    }
    if f.userns_active || f.hostname {
        parts.push(Part::Uts);
    }
    if f.proc_dir {
        parts.push(Part::Proc);
    }
    Opened::of(&parts)
}

/// The shape of a run that the manual names, by its index, with its
/// facts; `None` past the last (a `match`, as [`row`]).
pub fn shape(i: usize) -> Option<(&'static str, Facts)> {
    let f = |userns_active, host_root, detached| Facts {
        userns_active,
        host_root,
        detached,
        hostname: false,
        proc_dir: true,
    };
    Some(match i {
        0 => (
            "a foreground run of a caller that is not uid 0",
            f(true, false, false),
        ),
        1 => ("the same with PODVM_USERNS=0", f(false, false, false)),
        2 => ("a foreground run of uid 0", f(false, true, false)),
        3 => (
            "run -d or create of a caller that is not uid 0",
            f(false, false, true),
        ),
        4 => ("run -d or create of uid 0", f(false, true, true)),
        _ => return None,
    })
}

/// Each shape of [`shape`].
pub fn shapes() -> impl Iterator<Item = (&'static str, Facts)> {
    (0..).map_while(shape)
}

/// The ISOLATION section of the text manual: the rows, then what each
/// shape of a run on the namespace rung opens and what `exec` shares.
pub fn section() -> String {
    let mut out = String::from(
        "ISOLATION\n\
         What a run of the namespace rung opens, by caller and verb; --verbose, \
         --strict and inspect read this table. --hostname adds uts; an image with \
         no /proc directory gets no proc. The other rungs open none.\n\n",
    );
    for r in rows() {
        out.push_str(&format!("  {:<6} {}\n", r.word, r.gives));
    }
    out.push('\n');
    for (shape, f) in shapes() {
        let o = of_entry(Rung::Namespace, f);
        out.push_str(&format!(
            "  {shape}\n    opens: {}\n    does not provide: {}\n    exec: {}, shares {}\n",
            o.words(),
            o.never_claim(Rung::Namespace),
            o.exec_mode("namespace"),
            o.exec_shares("namespace"),
        ));
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests;
