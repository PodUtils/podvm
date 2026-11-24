//! TODO/machine.md T-2078 (remaining 2), OR-B39-13 of the eighth outside
//! pass: the room of the pool for an image that incus already has.
//!
//! The check of T-2078 ran only before an import, so on a full pool a run
//! of an image that incus had got incusd's raw 500 ("run out of disk
//! space"). The need of the root of the instance is the tree of the image,
//! and since T-2085 the store keeps no tree after an import. So the import
//! records the need of the tree in the store (`incus/ALIAS.need`), and a
//! run of the alias checks the pool with it before the create. pod-incus
//! has no call that reads an image (#651 asks podcrates for the resources
//! of an image); an alias with no record (an import of an older podvm, or
//! of another store) is not checked, as before.

use std::path::PathBuf;

use super::space::Need;

/// The most bytes of a record that a reader takes.
const MAX: u64 = 256;

/// The path of the record of `alias` in `store`, or `None` where the alias
/// is not one path segment.
pub fn path(store: &podvm_image::Store, alias: &str) -> Option<PathBuf> {
    super::space::segment(alias).then(|| store.root().join("incus").join(format!("{alias}.need")))
}

/// The text of a record: a version, the bytes and the entries.
pub fn text(need: Need) -> String {
    format!("v1 {} {}\n", need.bytes, need.entries)
}

/// The need of the text of a record; `None` where it is not a record of
/// version 1 with two counts.
pub fn parse(text: &str) -> Option<Need> {
    let mut w = text.split_ascii_whitespace();
    if w.next()? != "v1" {
        return None;
    }
    let bytes = w.next()?.parse().ok()?;
    let entries = w.next()?.parse().ok()?;
    w.next().is_none().then_some(Need { bytes, entries })
}

/// Write the record of `alias` (after a good import). A failure is one
/// line for the caller: the image is in incus all the same.
pub fn write(store: &podvm_image::Store, alias: &str, need: Need) -> Result<(), String> {
    let p = path(store, alias).ok_or_else(|| format!("the alias {alias:?} is no file name"))?;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    pod_core::files::replace(
        &p,
        text(need).as_bytes(),
        pod_core::files::Mode::Exact(0o600),
    )
    .map(drop)
    .map_err(|e| format!("{}: {e}", p.display()))
}

/// The record of `alias`, or `None` where there is none or it is not one.
pub fn read(store: &podvm_image::Store, alias: &str) -> Option<Need> {
    use std::io::Read;
    let p = path(store, alias)?;
    let f = std::fs::File::open(&p).ok()?;
    if !f.metadata().ok()?.is_file() {
        return None;
    }
    let mut s = String::new();
    f.take(MAX).read_to_string(&mut s).ok()?;
    parse(&s)
}

/// Before the create of an instance of `alias`, which incus has: the
/// refusal where the pool of the default profile has no room for the root
/// (a pool of the driver `dir`), else each warning on stderr. The plan of
/// the job gets the need, so that a failure of the create names it too.
pub async fn check(
    c: &pod_incus::Client,
    job: &super::run::Job<'_>,
    storage: &str,
    alias: &str,
) -> Result<(), super::run::Fault> {
    let Some(need) = job.store.and_then(|s| read(s, alias)) else {
        return Ok(());
    };
    let plan = super::space::Plan {
        tree: need,
        ..super::space::Plan::default()
    };
    let _ = job.plan.set(plan);
    if let Some(t) = super::space::Target::find(c, storage).await {
        for w in t.check(&plan, false).map_err(|e| (e, None))? {
            eprintln!("podvm {}: incus tier: {w}", job.verb);
        }
    }
    Ok(())
}
