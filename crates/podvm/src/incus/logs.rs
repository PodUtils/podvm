//! `logs --podvm-tier=incus NAME`: the log of an instance that stays
//! (TODO/machine.md T-1722), with `-f` and `--limit-bytes N` (TODO/cli.md
//! T-2046).
//!
//! Each read is `pod_incus::Client::get_file_to` (podcrates `v0.81.0`,
//! PodUtils/podvm#631): the bytes from the offset that podvm wrote last, in
//! pieces of 64 KiB, to stdout, at most the bytes that `--limit-bytes`
//! leaves. The memory of a read is one piece, not the size of the log, and
//! a log of any size is read.

use std::io::Write;

use pod_incus::{ByteRange, Client, ErrorKind};
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
    // T-2087 (OR-B40-18): docker gives an empty log for a created
    // container; an instance that never started has no log file.
    if super::ps::never_started(&i) {
        return Ok(());
    }
    let mut seen = 0u64;
    let mut left = limit;
    loop {
        // The state before the read: a read after a stop has each byte.
        let running = follow
            && c.instance(name)
                .await
                .map(|i| i.status == "Running")
                .map_err(incus)?;
        let read = c
            .get_file_to(
                name,
                super::LOG,
                Some(ByteRange::from(seen)),
                left,
                &mut *out,
            )
            .await;
        match read {
            Ok(r) => {
                seen += r.written;
                // A file that became shorter sets the offset back, as the
                // follow of a container does.
                if r.written == 0 && r.size.is_some_and(|size| size < seen) {
                    seen = r.size.unwrap_or(0);
                }
                if let Some(l) = left.as_mut() {
                    *l -= r.written.min(*l);
                    if *l == 0 {
                        return Ok(());
                    }
                }
            }
            Err(e) => {
                if let ErrorKind::Output(w) = e.kind() {
                    return Err(Fault::Write(std::io::Error::new(w.kind(), w.to_string())));
                }
                // The file can be missing before the command writes its
                // first line, while the instance runs.
                if !running {
                    return Err(match super::verbs::gone(&e) {
                        true => Fault::Incus(missing_log(c, name).await),
                        false => incus(e),
                    });
                }
            }
        }
        if !running {
            return Ok(());
        }
        // The follow is the only task of this current-thread runtime.
        std::thread::sleep(pause);
    }
}

/// TODO/cli.md T-2079 (OR-B39-11): the line where incusd has no log file
/// of the instance `name`: the payload runs as root of the instance and
/// can remove the files of podvm. The exit code stays (the init writes it
/// again at the end).
pub(crate) async fn missing_log(c: &Client, name: &str) -> String {
    let dir = match super::walk::find(c, name, super::DIR).await {
        super::walk::Found::Missing => format!("the directory {} is not there", super::DIR),
        _ => format!("{} is not there", super::LOG),
    };
    let running = c.instance(name).await.is_ok_and(|i| i.status == "Running");
    let who = match running {
        true => "Its command has not started yet, or the payload removed it",
        false => "The payload removed it",
    };
    format!(
        "the instance has no log: {dir}. {who} (the payload runs as root of the \
         instance), so no output stays to show; `podvm inspect --podvm-tier incus --format \
         '{{{{.ExitCode}}}}' {name}` gives the exit code"
    )
}

#[cfg(test)]
mod tests;
