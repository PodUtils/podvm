//! One table against the sources and the manual (TODO/cli.md T-2001 OP-01).

use super::*;
use std::collections::BTreeSet;
use std::path::Path;

/// Each `"PODVM_NAME"` that a source of any crate holds, but for this
/// module (it names each variable on purpose).
fn names_in_sources() -> BTreeSet<String> {
    fn walk(dir: &Path, out: &mut BTreeSet<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs")
                && !p.ends_with("podvm/src/environ.rs")
                && !p.ends_with("podvm/src/environ/tests.rs")
            {
                let Ok(text) = std::fs::read_to_string(&p) else {
                    continue;
                };
                for part in text.split('"') {
                    let named = part.starts_with("PODVM_")
                        && part.len() > 6
                        && part
                            .bytes()
                            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
                    if named {
                        out.insert(part.to_string());
                    }
                }
            }
        }
    }
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut out = BTreeSet::new();
    for c in std::fs::read_dir(crates)
        .expect("the crates directory reads")
        .flatten()
    {
        walk(&c.path().join("src"), &mut out);
    }
    out
}

#[test]
fn each_variable_of_the_sources_has_one_home_and_a_user_variable_is_in_the_manual() {
    let user: Vec<&str> = USER.iter().flat_map(|g| g.vars).map(|v| v.name).collect();
    let mut homes: Vec<(&str, &str)> = Vec::new();
    homes.extend(user.iter().map(|n| (*n, "USER")));
    homes.extend(INTERNAL.iter().map(|n| (*n, "INTERNAL")));
    homes.extend(DEV.iter().map(|n| (*n, "DEV")));
    let seen = names_in_sources();
    assert!(
        seen.contains("PODVM_STORE"),
        "the scan found no source: {seen:?}"
    );

    let mut faults = Vec::new();
    for name in &seen {
        let n = homes.iter().filter(|(h, _)| h == name).count();
        if n != 1 {
            faults.push(format!(
                "{name}: in {n} lists, not 1; add a row to USER (a user sets it), or a name to \
                 INTERNAL (podvm sets it) or DEV (a test or a build) in crates/podvm/src/environ.rs"
            ));
        }
    }
    for (name, home) in &homes {
        if !seen.contains(*name) {
            faults.push(format!("{name}: listed in {home}, and no source holds it"));
        }
    }
    let manual = section();
    for var in USER.iter().flat_map(|g| g.vars) {
        if !manual.contains(var.name) || var.effect.is_empty() || var.values.is_empty() {
            faults.push(format!(
                "{}: not in the ENVIRONMENT text with its values and effect",
                var.name
            ));
        }
    }
    let rendered = crate::man::render_for_test();
    for var in USER.iter().flat_map(|g| g.vars) {
        if !rendered.contains(var.name) {
            faults.push(format!("{}: not in the rendered manual", var.name));
        }
    }
    assert!(faults.is_empty(), "{}", faults.join("\n"));
}

#[test]
fn the_facts_carry_each_user_variable() {
    let facts = facts();
    let names: Vec<&str> = facts
        .variables
        .iter()
        .flat_map(|g| &g.vars)
        .flat_map(|v| &v.names)
        .map(String::as_str)
        .collect();
    let want: Vec<&str> = USER.iter().flat_map(|g| g.vars).map(|v| v.name).collect();
    assert_eq!(names, want);
}
