//! `logs --podvm-tier=incus NAME`: the log of an instance that stays
//! (TODO/machine.md T-1722), with `-f` and `--limit-bytes N` (TODO/cli.md
//! T-2046).
//!
//! ⚠ The API of incus gives a file whole (`GET .../files?path=`), and
//! `pod-incus` reads that answer whole, 64 MiB at most (`Settings::max_body`):
//! a larger log is refused. `--limit-bytes` bounds what podvm writes, not
//! what it reads. A ranged or streamed read of a file is the ask to
//! podcrates in the record of T-2046.

use std::io::Write;

use pod_incus::{Client, FileContent};
use podvm_image::error::{EXIT_FLAG_ERROR, EXIT_RUNTIME_ERROR};

use super::verbs::{block_on, connect, ours};

/// Why the copy of the log stopped.
#[derive(Debug)]
pub(crate) enum Fault {
    /// incus, or the instance, refused: the sentence.
    Incus(String),
    /// stdout refused a write.
    Write(std::io::Error),
}

/// `podvm logs --podvm-tier=incus [-f] [--limit-bytes N] NAME`.
pub fn logs(args: &[String]) -> i32 {
    let usage = crate::lifecycle::LOGS_USAGE;
    let expanded = match crate::parity::expand("logs", args) {
        Ok(a) => a,
        Err(member) => return crate::parity::refuse_member("logs", &member, usage),
    };
    let o = match crate::lifecycle::parse_logs(&expanded) {
        Ok(o) => o,
        Err(c) => return c,
    };
    if o.tail.is_some() {
        eprintln!(
            "podvm logs: --tail is not served with --podvm-tier=incus (TODO/machine.md T-1722)"
        );
        return EXIT_FLAG_ERROR;
    }
    block_on("logs", async move {
        let c = match connect().await {
            Ok((c, _)) => c,
            Err(e) => {
                eprintln!("podvm logs: incus tier: {e}");
                return EXIT_RUNTIME_ERROR;
            }
        };
        let mut out = std::io::stdout().lock();
        let pause = std::time::Duration::from_millis(500);
        match copy(&c, &o.want, o.follow, o.limit, pause, &mut out).await {
            Ok(()) => 0,
            Err(Fault::Write(e)) => crate::names::stdout::failed(&e),
            Err(Fault::Incus(e)) => {
                eprintln!("podvm logs: {}: {e}", o.want);
                1
            }
        }
    })
}

/// Write the log of the instance `name` to `out`, at most `limit` bytes;
/// with `follow`, read it again each `pause` while the instance runs, and
/// one last time after it stopped.
pub(crate) async fn copy(
    c: &Client,
    name: &str,
    follow: bool,
    limit: Option<u64>,
    pause: std::time::Duration,
    out: &mut dyn Write,
) -> Result<(), Fault> {
    let incus = |e: pod_incus::Error| Fault::Incus(e.to_string());
    let i = c.instance(name).await.map_err(incus)?;
    if !ours(&i) {
        return Err(Fault::Incus(
            "not an instance of podvm (no user.podvm label)".into(),
        ));
    }
    let mut seen = 0usize;
    let mut left = limit;
    loop {
        // The state before the read: a read after a stop has each byte.
        let running = follow
            && c.instance(name)
                .await
                .map(|i| i.status == "Running")
                .map_err(incus)?;
        let bytes = match c.get_file(name, super::LOG).await {
            Ok(f) => match f.content {
                FileContent::Bytes(b) => b,
                FileContent::Entries(_) => {
                    return Err(Fault::Incus(format!("{} is a directory", super::LOG)))
                }
            },
            // The file can be missing before the command writes its first
            // line, while the instance runs.
            Err(_) if running => Vec::new(),
            Err(e) => return Err(incus(e)),
        };
        // A file that became shorter sets the offset back, as the follow
        // of a container does.
        let from = seen.min(bytes.len());
        let new = &bytes[from..];
        let n = left.map_or(new.len(), |l| {
            new.len().min(usize::try_from(l).unwrap_or(usize::MAX))
        });
        out.write_all(&new[..n])
            .and_then(|()| out.flush())
            .map_err(Fault::Write)?;
        seen = from + n;
        if let Some(l) = left.as_mut() {
            *l -= n as u64;
            if *l == 0 {
                return Ok(());
            }
        }
        if !running {
            return Ok(());
        }
        // The follow is the only task of this current-thread runtime.
        std::thread::sleep(pause);
    }
}

#[cfg(test)]
mod tests;
