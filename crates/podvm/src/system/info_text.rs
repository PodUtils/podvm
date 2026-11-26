//! The text of `podvm system info` (split from `df_info.rs`).

use super::*;

/// ⚠ Read from `/proc`, and a dash where it cannot be read. AGENTS.md
/// absolute 3: a dash where a value is unknown, never a plausible-looking one.
pub(super) fn kernel_release() -> String {
    std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "-".into())
}

pub(super) fn pick(fields: &[(&str, String)], key: &str) -> String {
    fields
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

pub(super) fn human(fields: &[(&str, String)]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "podvm {}\n  rung          {}   (--strict would {})\n  entered with  {}\n  \
         platform      {}\n  kernel        {}\n  store         {}\n\n",
        pick(fields, "Version"),
        pick(fields, "Rung"),
        if pick(fields, "StrictOk") == "true" {
            "pass"
        } else {
            "refuse"
        },
        pick(fields, "EnteredRung"),
        pick(fields, "Platform"),
        pick(fields, "Kernel"),
        pick(fields, "Store"),
    ));
    out.push_str(&format!(
        "parity (TOOL.md section 6.8): {} rows, {} verbs\n  \
         Native {}   Degraded {}   Stub {}   None {}\n\n",
        pick(fields, "ParityRows"),
        pick(fields, "Verbs"),
        pick(fields, "VerbsNative"),
        pick(fields, "VerbsDegraded"),
        pick(fields, "VerbsStub"),
        pick(fields, "VerbsNone"),
    ));
    // ⛔ **WHICH INTERPOSER OBJECTS THIS BINARY CARRIES, and it is printed
    // because a caller cannot otherwise tell.** A podvm built where
    // `scripts/build-interpose.sh` could not run carries none and declines
    // every payload, which is a different tool from one that carries both.
    // `TOOL.md` section 4.1 forbids output that implies a capability podvm
    // does not have, and silence here would imply one.
    // `TODO/interpose.md` T-0702.
    out.push_str(&format!("{}\n\n", crate::interpose::carried()));
    out.push_str(&parity::text());
    out
}
