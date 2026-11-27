//! The parser of a `--format` template: a bounded part of Go's
//! `text/template` (TODO/cli.md T-2087, OR-B40-15).
//!
//! It reads text, actions `{{ }}` with the trim markers `{{-` and `-}}`,
//! comments `{{/* */}}`, `range PIPELINE` ... `end`, and pipelines of
//! fields (`.`, `.A.B`), string and integer literals, `( )` and the
//! functions of [`FUNCS`]. Each other word of Go's language is refused by
//! name: a template that podvm cannot read never prints a wrong answer.

use super::Bad;

/// The largest template, in bytes.
pub const MAX_TEMPLATE: usize = 64 * 1024;
/// The deepest nesting of `range` and `( )` together.
pub const MAX_DEPTH: usize = 16;

/// One part of a template.
#[derive(Debug, Clone)]
pub enum Node {
    Text(String),
    Action(Pipe),
    /// `{{range PIPE}}` BODY `{{end}}`.
    Range(Pipe, Vec<Node>),
}

/// A pipeline: commands that `|` joins, and the text of its action for a
/// message.
#[derive(Debug, Clone)]
pub struct Pipe {
    pub cmds: Vec<Cmd>,
    pub src: String,
}

/// One command of a pipeline.
#[derive(Debug, Clone)]
pub enum Cmd {
    Call(Func, Vec<Arg>),
    Operand(Arg),
}

/// One operand.
#[derive(Debug, Clone)]
pub enum Arg {
    /// `.` (no part) or `.A.B`.
    Field(Vec<String>),
    Str(String),
    Int(i64),
    Sub(Pipe),
}

/// The functions that podvm takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    Json,
    Index,
    Join,
    Upper,
    Lower,
    Len,
    Println,
}

/// Each function by its name in a template.
pub const FUNCS: &[(&str, Func)] = &[
    ("json", Func::Json),
    ("index", Func::Index),
    ("join", Func::Join),
    ("upper", Func::Upper),
    ("lower", Func::Lower),
    ("len", Func::Len),
    ("println", Func::Println),
];

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Field(Vec<String>),
    Word(String),
    Str(String),
    Int(i64),
    Open,
    Close,
    Bar,
}

/// Read `text` into its nodes.
pub fn parse(text: &str) -> Result<Vec<Node>, Bad> {
    if text.len() > MAX_TEMPLATE {
        return Err(Bad::TooLarge(format!(
            "the template has {} bytes; podvm reads at most {MAX_TEMPLATE}",
            text.len()
        )));
    }
    // Each open `range`: its pipeline and the nodes of its body so far.
    let mut stack: Vec<(Pipe, Vec<Node>)> = Vec::new();
    let mut top: Vec<Node> = Vec::new();
    let mut rest = text;
    let mut trim_next = false;
    loop {
        let (lit, action) = match rest.find("{{") {
            Some(at) => (&rest[..at], Some(&rest[at + 2..])),
            None => (rest, None),
        };
        let mut lit = if trim_next { lit.trim_start() } else { lit };
        let after = match action {
            None => {
                push(&mut stack, &mut top, Node::Text(lit.to_string()));
                break;
            }
            Some(a) => a,
        };
        let trim_left =
            after.starts_with("- ") || after.starts_with("-\t") || after.starts_with("-\n");
        if trim_left {
            lit = lit.trim_end();
        }
        push(&mut stack, &mut top, Node::Text(lit.to_string()));
        let body_at = if trim_left { 1 } else { 0 };
        let end = close_of(&after[body_at..]).ok_or(Bad::Unterminated)? + body_at;
        let mut inner = &after[body_at..end];
        rest = &after[end + 2..];
        trim_next = false;
        if inner.ends_with(" -") || inner.ends_with("\t-") || inner.ends_with("\n-") {
            inner = &inner[..inner.len() - 1];
            trim_next = true;
        }
        let src = format!("{{{{{}}}}}", inner.trim());
        let inner = inner.trim();
        if inner.starts_with("/*") {
            if !inner.ends_with("*/") {
                return Err(parse_bad(&src, "a comment ends with `*/` before `}}`"));
            }
            continue;
        }
        let toks = lex(inner, &src)?;
        match toks.first() {
            Some(Tok::Word(w)) if w == "range" => {
                if stack.len() + 1 > MAX_DEPTH {
                    return Err(Bad::TooDeep(MAX_DEPTH));
                }
                let pipe = pipeline(&toks[1..], &src, stack.len() + 1)?;
                stack.push((pipe, Vec::new()));
            }
            Some(Tok::Word(w)) if w == "end" => {
                if toks.len() > 1 {
                    return Err(parse_bad(&src, "`end` takes nothing after it"));
                }
                let Some((pipe, body)) = stack.pop() else {
                    return Err(parse_bad(&src, "an `end` with no `range` before it"));
                };
                push(&mut stack, &mut top, Node::Range(pipe, body));
            }
            _ => {
                let pipe = pipeline(&toks, &src, stack.len())?;
                push(&mut stack, &mut top, Node::Action(pipe));
            }
        }
    }
    if let Some((pipe, _)) = stack.last() {
        return Err(parse_bad(&pipe.src, "the `range` has no `{{end}}`"));
    }
    Ok(top)
}

fn push(stack: &mut [(Pipe, Vec<Node>)], top: &mut Vec<Node>, n: Node) {
    if matches!(&n, Node::Text(t) if t.is_empty()) {
        return;
    }
    match stack.last_mut() {
        Some((_, body)) => body.push(n),
        None => top.push(n),
    }
}

fn parse_bad(src: &str, why: &str) -> Bad {
    Bad::Parse {
        action: src.to_string(),
        why: why.to_string(),
    }
}

/// The index of the `}}` that ends an action, outside a string literal.
fn close_of(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 0;
    let mut quote: Option<u8> = None;
    while i < b.len() {
        match quote {
            Some(q) => {
                if q == b'"' && b[i] == b'\\' {
                    i += 1;
                } else if b[i] == q {
                    quote = None;
                }
            }
            None => {
                if b[i] == b'"' || b[i] == b'`' {
                    quote = Some(b[i]);
                } else if b[i] == b'}' && b.get(i + 1) == Some(&b'}') {
                    return Some(i);
                }
            }
        }
        i += 1;
    }
    None
}

fn is_name(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn lex(s: &str, src: &str) -> Result<Vec<Tok>, Bad> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch == '(' {
            out.push(Tok::Open);
            i += 1;
        } else if ch == ')' {
            out.push(Tok::Close);
            i += 1;
        } else if ch == '|' {
            out.push(Tok::Bar);
            i += 1;
        } else if ch == '.' && i > 0 && c[i - 1] == ')' {
            return Err(parse_bad(
                src,
                "a field of the value of `( )` is not read by podvm",
            ));
        } else if ch == '.' {
            let mut parts = Vec::new();
            while i < c.len() && c[i] == '.' {
                let start = i + 1;
                let mut j = start;
                while j < c.len() && is_name(c[j]) {
                    j += 1;
                }
                if j == start {
                    if parts.is_empty() && (j == c.len() || !c[j].eq(&'.')) {
                        i = j;
                        break; // `.` alone
                    }
                    return Err(parse_bad(src, "a field name is empty (`..`)"));
                }
                parts.push(c[start..j].iter().collect());
                i = j;
            }
            if i < c.len() && !(c[i].is_whitespace() || c[i] == ')' || c[i] == '|') {
                return Err(parse_bad(src, &format!("{:?} after a field name", c[i])));
            }
            out.push(Tok::Field(parts));
        } else if ch == '"' || ch == '`' {
            let mut text = String::new();
            let mut j = i + 1;
            loop {
                let Some(&d) = c.get(j) else {
                    return Err(parse_bad(src, "a string has no closing quote"));
                };
                if d == ch {
                    break;
                }
                if ch == '"' && d == '\\' {
                    j += 1;
                    match c.get(j) {
                        Some('n') => text.push('\n'),
                        Some('t') => text.push('\t'),
                        Some('\\') => text.push('\\'),
                        Some('"') => text.push('"'),
                        other => {
                            return Err(parse_bad(
                                src,
                                &format!(
                                    "the escape \\{} in a string",
                                    other.copied().unwrap_or(' ')
                                ),
                            ))
                        }
                    }
                } else {
                    text.push(d);
                }
                j += 1;
            }
            out.push(Tok::Str(text));
            i = j + 1;
        } else if ch.is_ascii_digit()
            || (ch == '-' && c.get(i + 1).is_some_and(char::is_ascii_digit))
        {
            let mut j = i + 1;
            while j < c.len() && is_name(c[j]) {
                j += 1;
            }
            let word: String = c[i..j].iter().collect();
            let n = word
                .parse::<i64>()
                .map_err(|_| parse_bad(src, &format!("{word:?} is not an integer podvm reads")))?;
            out.push(Tok::Int(n));
            i = j;
        } else if ch.is_alphabetic() || ch == '_' {
            let mut j = i;
            while j < c.len() && is_name(c[j]) {
                j += 1;
            }
            out.push(Tok::Word(c[i..j].iter().collect()));
            i = j;
        } else if ch == '$' {
            return Err(Bad::Unsupported("$ (a variable)".into()));
        } else {
            return Err(parse_bad(
                src,
                &format!("{ch:?} is not a part of a template podvm reads"),
            ));
        }
    }
    Ok(out)
}

/// The words of Go's language that podvm does not take.
const REFUSED: &[&str] = &[
    "if", "else", "with", "define", "template", "block", "break", "continue", "nil", "true",
    "false",
];

fn pipeline(toks: &[Tok], src: &str, depth: usize) -> Result<Pipe, Bad> {
    if toks.is_empty() {
        return Err(parse_bad(src, "the action is empty"));
    }
    let mut cmds = Vec::new();
    for part in split_bars(toks) {
        cmds.push(command(part, src, depth)?);
    }
    Ok(Pipe {
        cmds,
        src: src.to_string(),
    })
}

/// The commands of a pipeline: split at each `|` outside `( )`.
fn split_bars(toks: &[Tok]) -> Vec<&[Tok]> {
    let mut out = Vec::new();
    let mut level = 0usize;
    let mut start = 0;
    for (i, t) in toks.iter().enumerate() {
        match t {
            Tok::Open => level += 1,
            Tok::Close => level = level.saturating_sub(1),
            Tok::Bar if level == 0 => {
                out.push(&toks[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&toks[start..]);
    out
}

fn command(toks: &[Tok], src: &str, depth: usize) -> Result<Cmd, Bad> {
    let mut args = Vec::new();
    let mut i = 0;
    let func = match toks.first() {
        None => return Err(parse_bad(src, "a `|` has no command on one side")),
        Some(Tok::Word(w)) => {
            i = 1;
            Some(func_of(w)?)
        }
        Some(_) => None,
    };
    while i < toks.len() {
        let (a, next) = operand(toks, i, src, depth)?;
        args.push(a);
        i = next;
    }
    match func {
        Some(f) => Ok(Cmd::Call(f, args)),
        None if args.len() == 1 => Ok(Cmd::Operand(args.remove(0))),
        None => Err(parse_bad(
            src,
            "only a function takes arguments: put `index`, `join` or another function first",
        )),
    }
}

fn func_of(w: &str) -> Result<Func, Bad> {
    if let Some((_, f)) = FUNCS.iter().find(|(n, _)| *n == w) {
        return Ok(*f);
    }
    if REFUSED.contains(&w) || w == "range" || w == "end" {
        return Err(Bad::Unsupported(w.to_string()));
    }
    Err(Bad::Unsupported(format!("{w} (a function)")))
}

fn operand(toks: &[Tok], i: usize, src: &str, depth: usize) -> Result<(Arg, usize), Bad> {
    match &toks[i] {
        Tok::Field(p) => Ok((Arg::Field(p.clone()), i + 1)),
        Tok::Str(s) => Ok((Arg::Str(s.clone()), i + 1)),
        Tok::Int(n) => Ok((Arg::Int(*n), i + 1)),
        Tok::Open => {
            if depth + 1 > MAX_DEPTH {
                return Err(Bad::TooDeep(MAX_DEPTH));
            }
            let mut level = 0usize;
            let mut j = i;
            loop {
                match toks.get(j) {
                    None => return Err(parse_bad(src, "a `(` has no `)`")),
                    Some(Tok::Open) => level += 1,
                    Some(Tok::Close) => {
                        level -= 1;
                        if level == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            let inner = pipeline(&toks[i + 1..j], src, depth + 1)?;
            Ok((Arg::Sub(inner), j + 1))
        }
        Tok::Close => Err(parse_bad(src, "a `)` has no `(`")),
        Tok::Bar => Err(parse_bad(src, "a `|` has no command on one side")),
        Tok::Word(w) => Err(match func_of(w) {
            Ok(_) => parse_bad(
                src,
                &format!("the function {w} is an argument here: put it in `( )`"),
            ),
            Err(e) => e,
        }),
    }
}
