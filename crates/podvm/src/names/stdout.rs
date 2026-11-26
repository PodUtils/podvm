//! TODO/cli.md T-1816: a failed write to stdout ends the verb with exit 125
//! and a line on stderr that names the error, never with SIGABRT and never
//! with exit 0.
//!
//! The print macros panic when a write fails, and the release profile
//! aborts on a panic (`panic = "abort"`, which stays: a panic is a defect,
//! and a full disk is not one). The hook below runs before the abort: a
//! panic of a print macro becomes exit 125. [`flushed`] checks the last
//! flush at the exit of each verb, and [`failed`] is the answer of a verb
//! that writes through its own writer.

use std::io::Write;

/// The exit code of a failed write to stdout: podvm could not give its
/// answer, which is a runtime error of podvm.
pub const EXIT: i32 = podvm_image::error::EXIT_RUNTIME_ERROR;

/// The verb of this process, for the message.
fn verb() -> String {
    std::env::args_os()
        .nth(1)
        .map_or_else(|| "podvm".to_string(), |a| a.to_string_lossy().into_owned())
}

/// Write `line` to stderr, best effort: stderr can be full too.
fn say(line: &str) {
    let _ = std::io::stderr().write_all(line.as_bytes());
}

/// Set once a failed write was reported, so that the flush at the exit
/// does not report the same bytes again.
static REPORTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The answer of a verb whose write to stdout failed with `e`.
pub fn failed(e: &std::io::Error) -> i32 {
    if !REPORTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        say(&format!(
            "podvm {}: stdout: {e} (TODO/cli.md T-1816)\n",
            verb()
        ));
    }
    EXIT
}

/// The answer of a verb whose write to stdout failed, with the verb's own
/// `line` (which names stdout) in place of the line of [`failed`]: one
/// failure gives one line (TODO/cli.md T-2100, OR-B41-17).
pub fn failed_line(line: &str) -> i32 {
    if !REPORTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        say(&format!("{line}\n"));
    }
    EXIT
}

/// `code`, or [`EXIT`] where the last flush of stdout fails.
pub fn flushed(code: i32) -> i32 {
    match std::io::stdout().flush() {
        Ok(()) => code,
        Err(e) => failed(&e),
    }
}

/// While it lives, a write of podvm to a pipe that no process reads gives
/// EPIPE, not the end of podvm by SIGPIPE (TODO/cli.md T-2100, OR-B41-06):
/// a foreground run that wrote a line to a closed stderr ended before
/// it removed or ended its record. A handler, not `SIG_IGN`: `execve`
/// sets a caught signal back to its default, so the payload gets SIGPIPE
/// as with no podvm. At the drop SIGPIPE goes back to its default
/// (T-1769).
pub struct PipeCaught(());

/// Catch SIGPIPE until the [`PipeCaught`] drops.
pub fn catch_pipe() -> PipeCaught {
    extern "C" fn on_pipe(_: i32) {}
    set_pipe(on_pipe as *const () as usize);
    PipeCaught(())
}

impl Drop for PipeCaught {
    fn drop(&mut self) {
        set_pipe(0); // SIG_DFL
    }
}

fn set_pipe(handler: usize) {
    unsafe extern "C" {
        fn signal(sig: i32, handler: usize) -> usize;
    }
    // SAFETY: SIGPIPE (13) and SIG_DFL or a handler that does nothing,
    // which is safe in a signal.
    unsafe { signal(13, handler) };
}

/// The text of the panic of a print macro on `stream`, if `msg` is one.
pub fn print_failure<'a>(msg: &'a str, stream: &str) -> Option<&'a str> {
    msg.strip_prefix("failed printing to ")?
        .strip_prefix(stream)?
        .strip_prefix(": ")
}

/// Install the hook. A panic of a print macro on stdout says so and exits
/// 125; on stderr it exits 125 with no line, because stderr takes none. Each
/// other panic gives one named line and exits 125 (T-1947).
pub fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let p = info.payload();
        let msg = p
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| p.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        if let Some(why) = print_failure(&msg, "stdout") {
            say(&format!(
                "podvm {}: stdout: {why} (TODO/cli.md T-1816)\n",
                verb()
            ));
            std::process::exit(EXIT);
        }
        if print_failure(&msg, "stderr").is_some() {
            std::process::exit(EXIT);
        }
        // TODO/gate.md T-1947: each other panic is a bug of podvm. It ends
        // with one named line and exit 125, not SIGABRT (exit 134, the
        // release profile aborts), so that a caller reads a documented code
        // (PodUtils/podvm#505, #506). `RUST_BACKTRACE` keeps the default
        // report first.
        if std::env::var_os("RUST_BACKTRACE").is_some() {
            default(info);
        }
        say(&bug_line(
            &verb(),
            &msg,
            info.location().map(|l| (l.file(), l.line())),
        ));
        std::process::exit(EXIT);
    }));
}

/// The line of a panic that is a bug of podvm.
pub fn bug_line(verb: &str, msg: &str, at: Option<(&str, u32)>) -> String {
    let at = at.map_or(String::new(), |(f, l)| format!(" at {f}:{l}"));
    format!(
        "podvm {verb}: internal error: {msg}{at}. This is a bug of podvm, not of the \
         payload, and podvm stopped here. Please report this line \
         (https://github.com/PodUtils/podvm/issues) (TODO/gate.md T-1947)\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdout_full_the_panic_of_a_print_macro_is_read() {
        let m = "failed printing to stdout: No space left on device (os error 28)";
        assert_eq!(
            print_failure(m, "stdout"),
            Some("No space left on device (os error 28)")
        );
        assert_eq!(print_failure(m, "stderr"), None);
        assert_eq!(print_failure("index out of bounds", "stdout"), None);
    }

    #[test]
    fn a_bug_line_names_the_verb_the_message_and_the_place() {
        let l = bug_line("run", "overflow", Some(("src/x.rs", 7)));
        assert!(l.starts_with("podvm run: internal error: overflow at src/x.rs:7."));
        assert!(l.ends_with('\n') && l.matches('\n').count() == 1);
    }
}
