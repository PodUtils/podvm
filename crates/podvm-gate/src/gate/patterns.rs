//! TODO/gate.md T-1855: a scan of the product sources for the patterns
//! behind five defects, as a ratchet. `crates/podvm-gate/patterns.baseline`
//! holds the count of each pattern in each file as reviewed on 2026-11-15;
//! a file with more hits of a pattern than its baseline fails the gate,
//! with the path and the line of each hit. A fixed hit lowers the count,
//! and the baseline is written down to it in the same commit.
//!
//! The patterns (`docs/conventions/forbidden-patterns.md`):
//!
//! * `exit125`: `|| exit 125` in a guest script skips the exit marker
//!   (T-1823);
//! * `tmp_pid`: a temporary name of `temp_dir()` and a pid, or
//!   `.so.tmp`, is predictable (T-1782, T-1844);
//! * `byte_slice`: `&name[..N]` with a fixed N cuts user text inside a
//!   character (T-1800);
//! * `discard_write`: `let _ =` of a write hides a full stdout (T-1816);
//! * `joined_literal`: a run of 8 or more spaces after the fourth word of
//!   a string literal is the indentation of a continuation line that lost its
//!   `\\`, and the user reads it (T-2100, OR-B41-17).
//!
//! The print macros of a verb (T-1816) are not scanned: 232 sites print
//! through `println!`, and `tests/stdout_full.rs` holds the case of a full
//! stdout for the verbs.
//!
//! A file of tests (`tests.rs`, `*_tests.rs`, `tests/`), the gate itself,
//! the lines from `#[cfg(test)]` to the end of a file, and comment lines
//! are not scanned.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::*;

/// The file of the baseline, relative to the root.
pub(super) const BASELINE: &str = "crates/podvm-gate/patterns.baseline";

/// True when the line has `&IDENT[..DIGITS]`.
fn has_byte_slice(line: &str) -> bool {
    let b = line.as_bytes();
    let mut from = 0;
    while let Some(at) = line[from..].find("[..") {
        let open = from + at;
        from = open + 3;
        let mut i = open;
        while i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_') {
            i -= 1;
        }
        if i == open || i == 0 || b[i - 1] != b'&' {
            continue;
        }
        let digits = b[open + 3..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if digits > 0 && b.get(open + 3 + digits) == Some(&b']') {
            return true;
        }
    }
    false
}

/// True when a string literal of the line has a run of 8 or more spaces
/// after a word that follows a space, and text after the run: the
/// indentation of a continuation line that lost its `\\`. A run after the
/// first word of a literal is the alignment of a column, and stays.
fn has_joined_literal(line: &str) -> bool {
    let mut inside = false;
    let mut lit = String::new();
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match (inside, c) {
            (true, '\\') => {
                lit.push(c);
                lit.extend(chars.next());
            }
            (true, '"') => {
                if joined(&lit) {
                    return true;
                }
                lit.clear();
                inside = false;
            }
            (true, _) => lit.push(c),
            (false, '"') => inside = true,
            (false, _) => {}
        }
    }
    false
}

/// The rule of [`has_joined_literal`] on the text of one literal.
fn joined(lit: &str) -> bool {
    let b = lit.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b' ' {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && b[i] == b' ' {
            i += 1;
        }
        if i - start >= 8
            && i < b.len()
            && b[..start]
                .trim_ascii_start()
                .iter()
                .filter(|c| **c == b' ')
                .count()
                >= 3
        {
            return true;
        }
    }
    false
}

/// The names of the patterns that `line` holds.
pub(super) fn hits(line: &str) -> Vec<&'static str> {
    let mut v = Vec::new();
    if line.contains("|| exit 125") {
        v.push("exit125");
    }
    if (line.contains("temp_dir()") && line.contains("process::id")) || line.contains(".so.tmp") {
        v.push("tmp_pid");
    }
    if has_byte_slice(line) {
        v.push("byte_slice");
    }
    if line.contains("let _ =")
        && (line.contains(".write_all(") || line.contains("writeln!(") || line.contains("write!("))
    {
        v.push("discard_write");
    }
    if has_joined_literal(line) {
        v.push("joined_literal");
    }
    v
}

/// True for a file of the product that the scan reads.
pub(super) fn scanned(rel: &str) -> bool {
    rel.starts_with("crates/")
        && rel.contains("/src/")
        && rel.ends_with(".rs")
        && !rel.starts_with("crates/podvm-gate/")
        && !rel.ends_with("tests.rs")
        && !rel.contains("/tests/")
}

/// The hits of `text`: each pattern with the lines that hold it.
pub(super) fn scan(text: &str) -> BTreeMap<&'static str, Vec<usize>> {
    let mut m: BTreeMap<&'static str, Vec<usize>> = BTreeMap::new();
    for (n, line) in text.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("#[cfg(test)]") {
            break;
        }
        if t.starts_with("//") {
            continue;
        }
        for p in hits(line) {
            m.entry(p).or_default().push(n + 1);
        }
    }
    m
}

/// The baseline: `(pattern, path)` to the count.
pub(super) fn baseline(text: &str) -> BTreeMap<(String, String), usize> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut it = l.split('\t');
            let (p, path, n) = (it.next()?, it.next()?, it.next()?.parse().ok()?);
            Some(((p.to_string(), path.to_string()), n))
        })
        .collect()
}

pub(super) fn check_source_patterns(gate: &mut Gate, root: &Path, files: &BTreeSet<String>) {
    let base = baseline(&read_text(&root.join(BASELINE)).unwrap_or_default());
    for rel in files.iter().filter(|r| scanned(r)) {
        gate.bump("source_patterns");
        let Ok(text) = read_text(&root.join(rel)) else {
            continue;
        };
        for (p, lines) in scan(&text) {
            let allowed = base
                .get(&(p.to_string(), rel.clone()))
                .copied()
                .unwrap_or(0);
            if lines.len() > allowed {
                let at: Vec<String> = lines.iter().map(|n| n.to_string()).collect();
                gate.err(
                    &format!("{rel}:{}", lines[0]),
                    format!(
                        "{} hit(s) of the pattern {p} (lines {}), and {BASELINE} allows {allowed}: \
                         use the approved form of docs/conventions/forbidden-patterns.md \
                         (TODO/gate.md T-1855)",
                        lines.len(),
                        at.join(", ")
                    ),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_patterns_find_each_class() {
        assert_eq!(hits(r#""cd /w || exit 125""#), ["exit125"]);
        assert_eq!(
            hits("let p = std::env::temp_dir().join(format!(\"x-{}\", std::process::id()));"),
            ["tmp_pid"]
        );
        assert_eq!(hits("let s = &joined[..17];"), ["byte_slice"]);
        assert!(hits("let s = &joined[..n];").is_empty());
        assert!(hits("let s = joined[..17].to_vec();").is_empty());
        assert_eq!(hits("let _ = out.write_all(b);"), ["discard_write"]);
        assert!(hits("out.write_all(b)?;").is_empty());
        // T-2100: the continuation that lost its backslash, and the
        // alignment of a column, a usage line and an escaped quote that stay.
        assert_eq!(
            hits(r#"eprintln!("a b c d,              e f");"#),
            ["joined_literal"]
        );
        assert!(hits(r#"say("qemu flags        {}");"#).is_empty());
        assert!(hits(r#"say("== build 1           {}");"#).is_empty());
        assert!(hits(r#"say("a b c \"q\"              ");"#).is_empty());
        assert!(hits(r#"x("  -t, --tty        give");"#).is_empty());
        let m = scan(
            "let _ = writeln!(o);\n// let _ = writeln!(o);\n#[cfg(test)]\nlet _ = writeln!(o);\n",
        );
        assert_eq!(m.get("discard_write"), Some(&vec![1]));
        assert!(scanned("crates/podvm/src/run.rs"));
        assert!(!scanned("crates/podvm/src/run/tests.rs"));
        assert!(!scanned("crates/podvm/tests/x.rs"));
        let b = baseline("# c\nbyte_slice\tcrates/a/src/b.rs\t2\n");
        assert_eq!(
            b.get(&("byte_slice".into(), "crates/a/src/b.rs".into())),
            Some(&2)
        );
    }
}
