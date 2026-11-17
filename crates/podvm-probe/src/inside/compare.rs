//! Compare a report with a reference (TODO/probe.md T-1666).
//!
//! A compare between two hosts is never a pass by itself. Only a difference
//! in an invariant section ([`super::INVARIANT`]) can fail, and only where
//! the expected-differences file gives no cause for it.

use std::collections::{BTreeMap, BTreeSet};

use super::{Record, Verdict};

/// One explained difference: `SECTION<TAB>TARGET<TAB>REASON`, where TARGET
/// may be `*` for each target of the section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expected {
    pub section: String,
    pub target: String,
    pub reason: String,
}

/// The lines of an expected-differences file; `#` starts a note.
pub fn parse_expected(text: &str) -> Vec<Expected> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let mut f = l.splitn(3, '\t');
            Some(Expected {
                section: f.next()?.to_string(),
                target: f.next()?.to_string(),
                // PodUtils/podvm#504: an empty reason explains nothing.
                reason: f.next().map(str::trim).unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// One difference of verdict for one key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    pub section: String,
    pub target: String,
    pub here: Verdict,
    pub reference: Verdict,
    /// The reason of the expected-differences file, where one names it.
    pub reason: Option<String>,
}

impl Difference {
    pub fn invariant(&self) -> bool {
        super::INVARIANT.contains(&self.section.as_str())
    }

    /// True where a strict compare fails on it.
    pub fn unexplained_invariant(&self) -> bool {
        self.invariant() && self.reason.as_deref().is_none_or(str::is_empty)
    }
}

/// The result of a compare.
#[derive(Debug, Clone, Default)]
pub struct Outcome {
    pub same: usize,
    pub differ: Vec<Difference>,
    pub only_reference: usize,
    pub only_here: usize,
    /// TODO/probe.md T-1835: each invariant section of the reference with
    /// no record here. A strict compare fails on it: a report that lost a
    /// whole section measured nothing there.
    pub missing_invariant: Vec<String>,
}

impl Outcome {
    /// The differences that fail a strict compare.
    pub fn unexplained(&self) -> Vec<&Difference> {
        self.differ
            .iter()
            .filter(|d| d.unexplained_invariant())
            .collect()
    }

    /// The exit code: 0, or 1 under `strict` where an invariant section
    /// differs with no cause.
    pub fn code(&self, strict: bool) -> i32 {
        if strict && (!self.unexplained().is_empty() || !self.missing_invariant.is_empty()) {
            1
        } else {
            0
        }
    }

    /// The summary line of the issue's format.
    pub fn summary(&self) -> String {
        let explained = self.differ.iter().filter(|d| d.reason.is_some()).count();
        format!(
            "{} same, {} differ ({explained} explained, {} unexplained in an invariant \
             section), {} only in the reference, {} only here",
            self.same,
            self.differ.len(),
            self.unexplained().len(),
            self.only_reference,
            self.only_here
        ) + &if self.missing_invariant.is_empty() {
            String::new()
        } else {
            format!(
                "; invariant sections of the reference with no record here: {}",
                self.missing_invariant.join(", ")
            )
        }
    }
}

fn keyed(records: &[Record]) -> BTreeMap<(String, String), Verdict> {
    records
        .iter()
        .map(|r| ((r.section.clone(), r.target.clone()), r.verdict))
        .collect()
}

/// Compare `here` with `reference`, explaining by `expected`.
pub fn compare(here: &[Record], reference: &[Record], expected: &[Expected]) -> Outcome {
    let h = keyed(here);
    let r = keyed(reference);
    let keys: BTreeSet<_> = h.keys().chain(r.keys()).cloned().collect();
    let mut out = Outcome::default();
    for k in keys {
        match (h.get(&k), r.get(&k)) {
            (Some(a), Some(b)) if a == b => out.same += 1,
            (Some(a), Some(b)) => {
                let reason = expected
                    .iter()
                    .find(|e| e.section == k.0 && (e.target == "*" || e.target == k.1))
                    .map(|e| e.reason.clone());
                out.differ.push(Difference {
                    section: k.0.clone(),
                    target: k.1.clone(),
                    here: *a,
                    reference: *b,
                    reason,
                });
            }
            (Some(_), None) => out.only_here += 1,
            (None, Some(_)) => out.only_reference += 1,
            (None, None) => {}
        }
    }
    let sections_here: BTreeSet<&str> = here.iter().map(|r| r.section.as_str()).collect();
    out.missing_invariant = reference
        .iter()
        .filter(|r| r.invariant() && !sections_here.contains(r.section.as_str()))
        .map(|r| r.section.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    out
}

#[cfg(test)]
mod strict_tests {
    use super::*;
    use crate::inside::{parse_report_checked, Record, Verdict};

    fn report(rows: &[(&str, &str, Verdict)]) -> String {
        rows.iter()
            .map(|(s, t, v)| Record::new(s, t, *v, "").line())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// TODO/probe.md T-1835: an empty file and a file of prose are no
    /// report, so the compare refuses them before it counts anything.
    #[test]
    fn compare_strict_refuses_an_empty_report_and_prose() {
        assert!(parse_report_checked("").is_err());
        assert!(parse_report_checked("# only a note\n\n").is_err());
        let e = parse_report_checked("This is prose.\nIt has two lines.\n").unwrap_err();
        assert!(e.contains("line 1"), "{e}");
    }

    /// TODO/probe.md T-1835: a report with an invariant section of the
    /// reference removed fails under `--strict` and names the section.
    #[test]
    fn compare_strict_fails_on_a_missing_invariant_section() {
        let sec = crate::inside::INVARIANT[0];
        let reference = report(&[(sec, "a", Verdict::Deny), ("other", "b", Verdict::Allow)]);
        let here = report(&[("other", "b", Verdict::Allow)]);
        let o = compare(
            &parse_report_checked(&here).unwrap(),
            &parse_report_checked(&reference).unwrap(),
            &[],
        );
        assert_eq!(o.missing_invariant, vec![sec.to_string()]);
        assert_eq!(o.code(true), 1);
        assert_eq!(o.code(false), 0);
        assert!(o.summary().contains(sec), "{}", o.summary());
        let full = compare(
            &parse_report_checked(&reference).unwrap(),
            &parse_report_checked(&reference).unwrap(),
            &[],
        );
        assert_eq!(full.code(true), 0);
    }
}
