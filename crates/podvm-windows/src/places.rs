//! TODO/cli.md T-2100 (OR-B41-02): the places of the files of the machine
//! tier and of the Windows guests. Each one is in the store of podvm
//! (`PODVM_STORE`, else `XDG_DATA_HOME/podvm`, else
//! `HOME/.local/share/podvm`: `podvm_image::Store::configured_root`), or in
//! the directory that its own variable names. No file goes directly under
//! `$HOME`, and a home that is read-only does not stop a run with a store
//! on a writable disk.
//!
//! The sockets of a machine are not in the store: a path under a deep store
//! can pass the 108 bytes of `sun_path`. They are in `XDG_RUNTIME_DIR` (else
//! the temporary directory), and their names hold a hash of the directory
//! of the machines (`machines::socket`), so two stores do not share one.

use std::ffi::OsString;
use std::path::PathBuf;

/// A reader of the environment: `std::env::var_os`, or a table in a test.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// The environment of the process.
pub fn process(name: &str) -> Option<OsString> {
    std::env::var_os(name)
}

/// The value of `name` in `env`, where it is set and not empty.
fn set(env: Env, name: &str) -> Option<PathBuf> {
    env(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// The store root of `env`. Where no variable gives one, the root that the
/// probe chooses (`podvm_image::default_root`), else `podvm` in the
/// temporary directory.
pub fn store_root_in(env: Env) -> PathBuf {
    podvm_image::Store::configured_root(env)
        .or_else(|| podvm_image::default_root().ok())
        .unwrap_or_else(|| std::env::temp_dir().join("podvm"))
}

/// The base image of a Windows guest: `PODVM_WINDOWS_BASE`, else
/// `windows/base.vhdx` in the store.
pub fn base_cache_in(env: Env) -> PathBuf {
    set(env, "PODVM_WINDOWS_BASE")
        .unwrap_or_else(|| store_root_in(env).join("windows").join("base.vhdx"))
}

/// The base image of a DOS guest: `PODVM_DOS_BASE`, else
/// `windows/freedos-base.img` in the store.
pub fn dos_base_in(env: Env) -> PathBuf {
    set(env, "PODVM_DOS_BASE")
        .unwrap_or_else(|| store_root_in(env).join("windows").join("freedos-base.img"))
}

/// The directory of all machines (the records, the serial logs and the
/// pid files of each detached guest): `PODVM_MACHINES_DIR`, else
/// `windows/machines` in the store.
pub fn machines_in(env: Env) -> PathBuf {
    set(env, "PODVM_MACHINES_DIR")
        .unwrap_or_else(|| store_root_in(env).join("windows").join("machines"))
}

/// The directory of the copy of QEMU: `PODVM_TOOLS_DIR`, else
/// `tools/qemu-VERSION` in the store.
pub fn tools_in(env: Env) -> PathBuf {
    set(env, "PODVM_TOOLS_DIR").unwrap_or_else(|| {
        store_root_in(env)
            .join("tools")
            .join(format!("qemu-{}", crate::tools::QEMU_VERSION))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (variables, tools, machines, Windows base, DOS base).
    type Row = (
        &'static [(&'static str, &'static str)],
        String,
        &'static str,
        &'static str,
        &'static str,
    );

    /// One table: each place against the variables. A place follows its own
    /// variable, else the store, and the store follows `PODVM_STORE`, then
    /// `XDG_DATA_HOME`, then `HOME`; an empty or relative value counts as
    /// unset. No row puts a file directly under `HOME`.
    #[test]
    fn each_file_of_the_machine_tier_goes_where_the_variables_say() {
        let q = format!("tools/qemu-{}", crate::tools::QEMU_VERSION);
        let rows: &[Row] = &[
            (
                &[
                    ("PODVM_STORE", "/s"),
                    ("XDG_DATA_HOME", "/x"),
                    ("HOME", "/h"),
                ],
                format!("/s/{q}"),
                "/s/windows/machines",
                "/s/windows/base.vhdx",
                "/s/windows/freedos-base.img",
            ),
            (
                &[("PODVM_STORE", ""), ("XDG_DATA_HOME", "/x"), ("HOME", "/h")],
                format!("/x/podvm/{q}"),
                "/x/podvm/windows/machines",
                "/x/podvm/windows/base.vhdx",
                "/x/podvm/windows/freedos-base.img",
            ),
            (
                &[("XDG_DATA_HOME", "rel"), ("HOME", "/h")],
                format!("/h/.local/share/podvm/{q}"),
                "/h/.local/share/podvm/windows/machines",
                "/h/.local/share/podvm/windows/base.vhdx",
                "/h/.local/share/podvm/windows/freedos-base.img",
            ),
            (
                &[
                    ("PODVM_STORE", "/s"),
                    ("HOME", "/h"),
                    ("PODVM_TOOLS_DIR", "/t"),
                    ("PODVM_MACHINES_DIR", "/m"),
                    ("PODVM_WINDOWS_BASE", "/w/b.vhdx"),
                    ("PODVM_DOS_BASE", "/d/f.img"),
                ],
                "/t".into(),
                "/m",
                "/w/b.vhdx",
                "/d/f.img",
            ),
            (
                &[
                    ("PODVM_STORE", "/s"),
                    ("PODVM_TOOLS_DIR", ""),
                    ("PODVM_MACHINES_DIR", ""),
                    ("PODVM_WINDOWS_BASE", "/w/b.vhdx"),
                ],
                format!("/s/{q}"),
                "/s/windows/machines",
                "/w/b.vhdx",
                "/s/windows/freedos-base.img",
            ),
        ];
        for (vars, tools, machines, base, dos) in rows {
            let env = |k: &str| {
                vars.iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| OsString::from(v))
            };
            let got = [
                tools_in(&env),
                machines_in(&env),
                base_cache_in(&env),
                dos_base_in(&env),
            ];
            let want = [tools.as_str(), machines, base, dos].map(PathBuf::from);
            assert_eq!(got, want, "{vars:?}");
            if let Some((_, home)) = vars.iter().find(|(n, _)| *n == "HOME") {
                for p in &got {
                    assert_ne!(p.parent(), Some(std::path::Path::new(home)), "{vars:?}");
                }
            }
        }
    }
}
