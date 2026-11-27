//! TODO/cli.md T-2087 (OR-B40-09) and T-2108 (OR-B42-04): an argument of
//! podvm that is not UTF-8.
//!
//! docker passes each argument of the payload and each `-e` value as
//! bytes. podvm reads its arguments as text, so [`carry`] keeps the bytes
//! of an argument that is not UTF-8 in a text: byte B (0x80 to 0xFF) of an
//! invalid sequence becomes the character U+10FF00 + B, and [`decode`]
//! gives the bytes back. A UTF-8 argument that holds one of these 128
//! characters is refused at the start, so a carried character always
//! names a byte.
//!
//! ⛔ The payload cannot pass yet: each tier and rung hands the command
//! and the environment to the plan of pod-enter (podcrates), whose
//! `Plan::argv` and `Plan::env` are UTF-8 text. [`word`] refuses each
//! carried argument where the parser reads it, with one line that names
//! its position and the reason: a flag, a value or an image name of
//! podvm, or a word of the payload. Exit 125, never a panic.

use std::sync::OnceLock;

/// The character of byte 0: byte B is `BASE + B`.
const BASE: u32 = 0x10_FF00;

/// The arguments as podvm read them, for the position of a line.
static ARGV: OnceLock<Vec<String>> = OnceLock::new();

/// True for a character that carries a byte.
fn is_carried(c: char) -> bool {
    (BASE + 0x80..=BASE + 0xFF).contains(&(c as u32))
}

/// The text of `arg`: the text itself where it is UTF-8, else its valid
/// parts and a carried character for each other byte. `Err` names a
/// character of the carried range that the argument holds.
pub fn carry(arg: &[u8]) -> Result<String, char> {
    let mut out = String::with_capacity(arg.len());
    for chunk in arg.utf8_chunks() {
        if let Some(c) = chunk.valid().chars().find(|c| is_carried(*c)) {
            return Err(c);
        }
        out.push_str(chunk.valid());
        for b in chunk.invalid() {
            // An invalid byte is 0x80 or more: each ASCII byte is valid.
            out.push(char::from_u32(BASE + u32::from(*b)).unwrap_or('\u{FFFD}'));
        }
    }
    Ok(out)
}

/// The bytes of a text of [`carry`].
pub fn decode(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut buf = [0u8; 4];
    for c in s.chars() {
        if is_carried(c) {
            out.push((c as u32 - BASE) as u8);
        } else {
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    }
    out
}

/// True where `s` carries a byte that is not UTF-8.
pub fn carries(s: &str) -> bool {
    s.chars().any(is_carried)
}

/// `bytes` in quotes, as a line shows them: UTF-8 as text, each other
/// byte as `\xNN`.
pub fn shown_bytes(bytes: &[u8]) -> String {
    let mut out = String::from("\"");
    for chunk in bytes.utf8_chunks() {
        out.extend(chunk.valid().chars().flat_map(char::escape_debug));
        for b in chunk.invalid() {
            out.push_str(&format!("\\x{b:02x}"));
        }
    }
    out.push('"');
    out
}

/// A text of [`carry`] as a line shows it.
pub fn shown(s: &str) -> String {
    shown_bytes(&decode(s))
}

/// Keep the arguments for [`position`].
pub fn record(argv: &[String]) {
    let _ = ARGV.set(argv.to_vec());
}

/// The position of `word` in the arguments of podvm (0 is podvm), or the
/// first that holds it (`--env=K=V` gives `K=V`).
fn position(word: &str) -> Option<usize> {
    let argv = ARGV.get()?;
    argv.iter()
        .position(|a| a == word)
        .or_else(|| argv.iter().position(|a| a.contains(word)))
}

fn at(word: &str) -> String {
    position(word).map_or_else(|| "an argument".to_string(), |n| format!("argument {n}"))
}

/// What a word of a parser is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// A flag, its value or the image name: podvm's own, UTF-8.
    Podvm,
    /// The command, an argument of it, or an `-e` value.
    Payload,
}

impl Class {
    /// [`Class::Payload`] where `payload` holds, else [`Class::Podvm`].
    pub fn of(payload: bool) -> Class {
        if payload {
            Class::Payload
        } else {
            Class::Podvm
        }
    }
}

/// The line that refuses the carried word `a` of `verb`.
pub fn line(verb: &str, a: &str, class: Class) -> String {
    let (at, shown) = (at(a), shown(a));
    let verb = if verb.is_empty() {
        String::new()
    } else {
        format!(" {verb}")
    };
    match class {
        Class::Podvm => format!(
            "podvm{verb}: {at} ({shown}) is not UTF-8: podvm takes its flags, their \
             values and the image name as UTF-8 (TODO/cli.md T-2087)"
        ),
        Class::Payload => format!(
            "podvm{verb}: {at} ({shown}) of the payload is not UTF-8: each tier hands the \
             command and the environment to the plan of pod-enter, which takes UTF-8 only \
             (TODO/cli.md T-2087)"
        ),
    }
}

/// `Err(125)` with the line where `a` carries a byte; a parser calls it for
/// each word it reads.
pub fn word(verb: &str, a: &str, class: Class) -> Result<(), i32> {
    if !carries(a) {
        return Ok(());
    }
    eprintln!("{}", line(verb, a, class));
    Err(podvm_image::error::EXIT_RUNTIME_ERROR)
}

/// The verbs whose parser reads each word with [`word`].
fn parses_words(argv: &[String]) -> bool {
    match argv.get(1).map(String::as_str) {
        Some("run" | "create" | "exec") => true,
        Some("container") => matches!(
            argv.get(2).map(String::as_str),
            Some("run" | "create" | "exec")
        ),
        _ => false,
    }
}

/// The exit code where a verb that takes UTF-8 only has a carried word.
pub fn gate(argv: &[String]) -> Option<i32> {
    if parses_words(argv) {
        return None;
    }
    // Argument 0 is the path of podvm: no verb reads it as text.
    let (n, a) = argv.iter().enumerate().skip(1).find(|(_, a)| carries(a))?;
    let verb = argv.get(1).map_or("podvm", String::as_str);
    eprintln!(
        "podvm: argument {n} ({}) is not UTF-8: `{}` takes UTF-8 arguments only \
         (TODO/cli.md T-2087)",
        shown(a),
        if carries(verb) { "podvm" } else { verb }
    );
    Some(podvm_image::error::EXIT_RUNTIME_ERROR)
}
