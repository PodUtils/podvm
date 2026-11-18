//! `run --report PATH` (TODO/cli.md T-1695, A-V-09 and G-06 of
//! PodUtils/podcrates#39): one JSON document for each run, written to PATH
//! with a rename, also when the run refuses before its start.
//!
//! The document goes to a file, never to standard output, which belongs to
//! the payload. It holds no value of an environment variable. A value that
//! podvm does not know is `null`.

use std::path::{Path, PathBuf};
use std::time::Instant;

/// The version of the document's layout.
pub const SCHEMA: u32 = 1;

/// The report of one run, filled while the run goes.
#[derive(Debug)]
pub struct Report {
    pub schema: u32,
    pub podvm: &'static str,
    pub image: Option<String>,
    pub image_digest: Option<String>,
    /// The rung or tier that the run entered, and how podvm knows it.
    pub rung: Option<String>,
    pub measured_by: &'static str,
    pub argv: Vec<String>,
    /// The container of a detached run, which is still running.
    pub container: Option<String>,
    pub detached: bool,
    /// The exit code of the payload; null where the run refused or is
    /// detached.
    pub exit_code: Option<i32>,
    /// The signal that ended the payload, read from the code (128 + N).
    pub signal: Option<i32>,
    /// Why podvm did not run the payload: the exit code of the refusal.
    pub refused: Option<i32>,
    pub started: String,
    pub ended: Option<String>,
    pub wall_ms: Option<u64>,
    /// The CPU time and the peak resident set of the reaped children of
    /// the run (`getrusage(RUSAGE_CHILDREN)`): the payload's tree, and the
    /// steps of the completion layer before it.
    pub user_ms: Option<u64>,
    pub system_ms: Option<u64>,
    pub max_rss_kib: Option<u64>,
    /// The OOM kills of the cgroup leaf of the run (T-1969).
    pub oom_kills: Option<u64>,
    pub timed_out: bool,
    pub timeout_s: Option<u64>,
    pub limits: Vec<String>,
    pub no_new_privs: bool,
    pub confine: Option<String>,
    pub labels: Vec<String>,
    /// Each fixup of the completion layer that changed a byte or failed.
    pub degradations: Vec<String>,
    clock: Option<Instant>,
    /// The payload was entered (or, detached, started): its code is its
    /// own. False is a refusal of podvm.
    pub ran: bool,
}

impl Report {
    /// TODO/cli.md T-1802: the report starts with no argv. The argv of
    /// podvm holds the values of `-e`; the report holds the command of the
    /// payload only, from the parse or from the prepared run.
    pub fn begin() -> Report {
        Report {
            schema: SCHEMA,
            podvm: env!("CARGO_PKG_VERSION"),
            image: None,
            image_digest: None,
            rung: None,
            measured_by: "the rung that podvm entered, as its banner names it",
            argv: Vec::new(),
            container: None,
            detached: false,
            exit_code: None,
            signal: None,
            refused: None,
            started: podvm_image::clock::now(),
            ended: None,
            wall_ms: None,
            user_ms: None,
            system_ms: None,
            max_rss_kib: None,
            oom_kills: None,
            timed_out: false,
            timeout_s: None,
            limits: Vec::new(),
            no_new_privs: false,
            confine: None,
            labels: Vec::new(),
            degradations: Vec::new(),
            clock: Some(Instant::now()),
            ran: false,
        }
    }

    /// The image and the command that the parse read, for a run that a
    /// step after the parse refuses (TODO/cli.md T-1802).
    pub fn parsed(&mut self, image: Option<String>, command: Vec<String>) {
        if self.image.is_none() {
            self.image = image;
        }
        if self.argv.is_empty() {
            self.argv = command;
        }
    }

    /// The facts of a run of a guest tier, which ran: its exit code is the
    /// payload's (TODO/machine.md T-1803). A code of 137 with a timeout is
    /// the timeout of T-1809. The CPU time and the resident set are of the
    /// guest's emulator, which `rusage` of podvm's children does not read
    /// for the payload, so they stay null.
    pub fn guest(&mut self, g: Guest, code: i32) {
        self.rung = Some(g.tier.to_string());
        self.measured_by = "the tier that podvm entered, which runs the payload in a guest";
        self.image = Some(g.image);
        self.image_digest = Some(g.digest);
        self.timeout_s = g.timeout_s;
        self.limits = g.limits;
        self.timed_out = code == 137 && g.timeout_s.is_some();
        self.ran = true;
    }

    /// The facts of a prepared run.
    pub fn prepared(&mut self, p: &crate::lifecycle::Prepared) {
        self.image = Some(p.image.clone());
        self.image_digest = Some(p.record.manifest_digest.clone());
        self.rung = Some(p.rung.clone());
        self.argv = p.argv.clone();
        self.detached = p.detach;
        self.timeout_s = p.wall_ms.map(|m| m / 1000);
        self.limits = p.hardening.ulimits.clone();
        self.no_new_privs = p.hardening.no_new_privs;
        self.confine = p.hardening.confine.clone();
        self.labels = p.hardening.labels.clone();
        self.degradations = p.completion.clone();
    }

    /// The end of the run with exit code `code`: the payload's code where
    /// [`Report::ran`], else a refusal of podvm.
    pub fn end(&mut self, code: i32) {
        self.ended = Some(podvm_image::clock::now());
        self.wall_ms = self.clock.map(|c| c.elapsed().as_millis() as u64);
        if !self.ran {
            self.refused = Some(code);
        } else if !self.detached {
            self.exit_code = Some(code);
            self.signal = (129..=192).contains(&code).then(|| code - 128);
            // T-1969: the launcher of `run -d` writes its own at the end.
            self.oom_kills = super::cgroup::oom_kills();
            if self.measured_by.starts_with("the tier") {
                // A guest: no rusage of the payload (T-1803).
            } else if let Ok(u) = podvm_probe::sys::rusage_children() {
                self.user_ms = Some(u.user_us / 1000);
                self.system_ms = Some(u.system_us / 1000);
                self.max_rss_kib = Some(u.max_rss_kib);
            }
        }
    }

    /// The document.
    pub fn json(&self) -> serde_json::Value {
        let mut j = serde_json::json!({
            "schema": self.schema,
            "podvm": self.podvm,
            "image": self.image,
            "image_digest": self.image_digest,
            "rung": self.rung,
            "measured_by": self.measured_by,
            "argv": self.argv,
            "container": self.container,
            "detached": self.detached,
            "exit_code": self.exit_code,
            "signal": self.signal,
            "refused": self.refused,
            "started": self.started,
            "ended": self.ended,
            "wall_ms": self.wall_ms,
            "user_ms": self.user_ms,
            "system_ms": self.system_ms,
            "max_rss_kib": self.max_rss_kib,
            "timed_out": self.timed_out,
            "timeout_s": self.timeout_s,
            "limits": self.limits,
            "no_new_privs": self.no_new_privs,
            "confine": self.confine,
            "labels": self.labels,
            "degradations": self.degradations,
        });
        // T-1969: an added optional field; the schema id stays.
        if let Some(n) = self.oom_kills {
            j["oom_kills"] = n.into();
        }
        j
    }

    /// Write the document to `path`: a file beside it, then a rename.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(&self.json()).map_err(|e| e.to_string())?;
        let dir = path
            .parent()
            .filter(|d| !d.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
        std::fs::write(&tmp, text + "\n").map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("{}: {e}", path.display())
        })
    }
}

thread_local! {
    /// The PATH of `--report`, as the parser of `run` read it.
    static PATH: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// The parser of `run` notes the PATH of `--report` when it reads the flag,
/// so that a run that a later word or a later step refuses still gets its
/// report. TODO/cli.md T-1801: only the parser decides which words are
/// podvm's; a word after the image is the payload's and is never read here.
pub(crate) fn note_path(p: &str) {
    PATH.with(|s| *s.borrow_mut() = Some(PathBuf::from(p)));
}

thread_local! {
    /// The image and the command of the payload, as the parse of `run` read
    /// them (TODO/cli.md T-1802).
    static PARSED: std::cell::RefCell<Option<(Option<String>, Vec<String>)>> =
        const { std::cell::RefCell::new(None) };
}

/// The parse of `run` notes the image and the command of the payload.
pub(crate) fn note_parsed(image: Option<String>, command: Vec<String>) {
    PARSED.with(|s| *s.borrow_mut() = Some((image, command)));
}

/// What the parse noted, once.
pub fn take_parsed() -> Option<(Option<String>, Vec<String>)> {
    PARSED.with(|s| s.borrow_mut().take())
}

/// A run of a guest tier (machine, incus), noted when its guest starts
/// (TODO/machine.md T-1803): the tier, the image and its digest, and the
/// timeout in seconds.
#[derive(Debug, Clone)]
pub struct Guest {
    pub tier: &'static str,
    pub image: String,
    pub digest: String,
    pub timeout_s: Option<u64>,
    /// The limits that the tier holds (TODO/machine.md T-1897: the keys
    /// of an incus instance), for `limits` of the report.
    pub limits: Vec<String>,
}

thread_local! {
    static GUEST: std::cell::RefCell<Option<Guest>> = const { std::cell::RefCell::new(None) };
}

/// A guest tier notes its run when the guest starts.
pub(crate) fn note_guest(g: Guest) {
    GUEST.with(|s| *s.borrow_mut() = Some(g));
}

/// What the guest tier noted, once.
pub fn take_guest() -> Option<Guest> {
    GUEST.with(|s| s.borrow_mut().take())
}

/// The PATH that the parser noted, once. A refusal before the parser read
/// the flag has none, and writes no report.
pub fn take_path() -> Option<PathBuf> {
    PATH.with(|s| s.borrow_mut().take())
}

/// TODO/cli.md T-1825: the record of a detached run keeps the absolute
/// PATH of its report, which its launcher finishes from its own working
/// directory. The PATH stays for [`take_path`].
pub fn with_detached_path(
    mut h: podvm_supervise::table::Hardening,
    detach: bool,
) -> podvm_supervise::table::Hardening {
    if detach {
        h.report = PATH.with(|s| s.borrow().clone()).map(|p| {
            std::path::absolute(&p)
                .unwrap_or(p)
                .to_string_lossy()
                .into_owned()
        });
    }
    h
}

#[cfg(test)]
mod tests;
