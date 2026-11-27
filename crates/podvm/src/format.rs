//! `--format`: the part of docker's Go templates that podvm reads.
//!
//! TODO/cli.md T-2087 (OR-B40-15): fields (`.`, `.A.B`), string and integer
//! literals, `( )`, pipelines with `|`, `range` ... `end`, and the functions
//! `json`, `index`, `join`, `upper`, `lower`, `len` and `println`
//! ([`parse`]); the `table` prefix of a list verb ([`table`]); `json` alone
//! as `{{json .}}`. Each other word of Go's language (`if`, `with`, a
//! variable, `printf`) is refused by name with one line: a template that
//! podvm cannot read never prints a wrong answer.
//!
//! ⛔ **Selected by name, never by position.** `docs/conventions/code.md`. An
//! unknown field is refused and the known ones are listed, rather than
//! rendering as empty: a template that silently produces a blank column is a
//! wrong answer that looks like a right one.
//!
//! ⛔ **Bounded.** The template ([`parse::MAX_TEMPLATE`]), its nesting
//! ([`parse::MAX_DEPTH`]), its output and its steps ([`eval::MAX_OUTPUT`],
//! [`eval::MAX_STEPS`]) have limits, and each limit ends with a true line.

mod eval;
mod parse;
mod table;
#[cfg(test)]
mod tests;

use parse::{Arg, Cmd, Node, Pipe};
use serde_json::Value;

/// What went wrong with a template, for a message the caller can act on.
#[derive(Debug)]
pub enum Bad {
    Unterminated,
    Unsupported(String),
    UnknownField {
        got: String,
        known: Vec<String>,
    },
    /// The template does not parse: the action, and why.
    Parse {
        action: String,
        why: String,
    },
    /// The template parsed and an action cannot give a value.
    Exec {
        action: String,
        why: String,
    },
    TooLarge(String),
    TooDeep(usize),
    /// `table` for a verb that prints one document for each name.
    Table,
}

impl std::fmt::Display for Bad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Bad::Unterminated => write!(f, "the template has a `{{{{` that is never closed"),
            Bad::Unsupported(what) => write!(
                f,
                "`{what}` is not a part of Go's templates that podvm's --format reads. It \
                 reads fields (`.`, `.A.B`), strings, integers, `( )`, `|`, `range` ... \
                 `end`, and the functions json, index, join, upper, lower, len and println"
            ),
            Bad::UnknownField { got, known } => write!(
                f,
                "there is no field {got:?}. This verb has: {}",
                known.join(", ")
            ),
            Bad::Parse { action, why } | Bad::Exec { action, why } => {
                write!(f, "in {action}: {why}")
            }
            Bad::TooLarge(why) => write!(f, "{why}"),
            Bad::TooDeep(n) => write!(
                f,
                "the template nests `range` and `( )` deeper than {n} levels, the limit of podvm"
            ),
            Bad::Table => write!(
                f,
                "the `table` prefix lays out the rows of a list verb (`ps`, `images`); this \
                 verb prints one document for each name, so use the fields and `\\t` without it"
            ),
        }
    }
}

impl Bad {
    /// The line with no list of the known fields, for a message that names
    /// two kinds (`inspect` of an image and of a container).
    pub fn short(&self) -> String {
        match self {
            Bad::UnknownField { got, .. } => format!("there is no field {got:?}"),
            other => other.to_string(),
        }
    }
}

/// A field with its JSON type, and the text for `{{.X}}` where it is not the
/// Go text of the value (`{{.State}}` is the word, and `{{json .State}}` is
/// docker's object).
#[derive(Clone, Debug)]
pub struct Typed {
    pub json: Value,
    pub plain: Option<String>,
}

/// The fields of one row or one document. A registered name can hold dots
/// (`Exec.Shares`); `.` is the whole object.
pub struct Data {
    fields: Vec<(String, Typed)>,
    root: Value,
}

impl Data {
    fn new(fields: Vec<(String, Typed)>) -> Data {
        let root = nest(&fields);
        Data { fields, root }
    }

    /// Fields that are text.
    pub fn text(fields: &[(&str, String)]) -> Data {
        Data::documents(fields, &[])
    }

    /// Fields that are text, and the names of those whose text is a JSON
    /// document (`{{json .Parity}}` prints it as one).
    pub fn documents(fields: &[(&str, String)], documents: &[&str]) -> Data {
        Data::new(
            fields
                .iter()
                .map(|(k, v)| {
                    let t = if documents.contains(k) {
                        Typed {
                            json: serde_json::from_str(v).unwrap_or_else(|_| v.clone().into()),
                            plain: Some(v.clone()),
                        }
                    } else {
                        Typed {
                            json: v.clone().into(),
                            plain: None,
                        }
                    };
                    (k.to_string(), t)
                })
                .collect(),
        )
    }

    /// Fields with JSON types; `plain` names the text of a field for
    /// `{{.X}}` where it is not [`go_text`] of its value.
    pub fn typed(fields: &[(&str, Value)], plain: &[(&str, String)]) -> Data {
        Data::new(
            fields
                .iter()
                .map(|(k, v)| {
                    let plain = plain.iter().find(|(p, _)| p == k).map(|(_, t)| t.clone());
                    (
                        k.to_string(),
                        Typed {
                            json: v.clone(),
                            plain,
                        },
                    )
                })
                .collect(),
        )
    }

    /// The object of `.`: the verb's own document (`inspect` prints it),
    /// where the verb has one.
    pub fn with_root(mut self, root: Value) -> Data {
        self.root = root;
        self
    }

    fn known(&self) -> Vec<&str> {
        self.fields.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// The value of `.A.B` at the root: a registered name first (the longest
    /// that starts the chain), then a key of the object of `.`.
    fn resolve(&self, chain: &[String]) -> Result<eval::Val, Bad> {
        let shown = format!(".{}", chain.join("."));
        if chain.is_empty() {
            return Ok(eval::Val {
                json: self.root.clone(),
                plain: None,
            });
        }
        for k in (1..=chain.len()).rev() {
            let name = chain[..k].join(".");
            if let Some((_, t)) = self.fields.iter().find(|(n, _)| *n == name) {
                if k == chain.len() {
                    return Ok(eval::Val {
                        json: t.json.clone(),
                        plain: t.plain.clone(),
                    });
                }
                return eval::navigate(&t.json, &chain[k..], &format!(".{name}"))
                    .map_err(|why| exec(&shown, why));
            }
        }
        if self.root.get(&chain[0]).is_some() {
            return eval::navigate(&self.root, chain, "").map_err(|why| exec(&shown, why));
        }
        Err(unknown(&shown, &self.known()))
    }
}

fn exec(action: &str, why: String) -> Bad {
    Bad::Exec {
        action: action.to_string(),
        why,
    }
}

fn unknown(got: &str, known: &[&str]) -> Bad {
    Bad::UnknownField {
        got: got.to_string(),
        known: known.iter().map(|k| format!(".{k}")).collect(),
    }
}

/// The object of `.` from the fields: `A.B` is the key `B` of the object
/// `A`. A name under a field that is not an object stays out of it.
fn nest(fields: &[(String, Typed)]) -> Value {
    let mut sorted: Vec<&(String, Typed)> = fields.iter().collect();
    sorted.sort_by_key(|(k, _)| k.matches('.').count());
    let mut root = serde_json::Map::new();
    'each: for (k, t) in sorted {
        let parts: Vec<&str> = k.split('.').collect();
        let mut at = &mut root;
        for p in &parts[..parts.len() - 1] {
            let next = at
                .entry(p.to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            match next {
                Value::Object(o) => at = o,
                _ => continue 'each,
            }
        }
        at.insert(parts[parts.len() - 1].to_string(), t.json.clone());
    }
    Value::Object(root)
}

/// A parsed template.
pub struct Template {
    nodes: Vec<Node>,
    table: bool,
    /// `table` alone: the verb's own table.
    default_table: bool,
}

impl Template {
    /// Read `text` as docker's CLI reads it: `json` alone is `{{json .}}`,
    /// a `table` prefix lays out the rows, and `\t` and `\n` are a tab and a
    /// new line.
    pub fn parse(text: &str) -> Result<Template, Bad> {
        let trimmed = text.trim_matches(' ');
        if trimmed == "json" {
            return Ok(Template {
                nodes: parse::parse("{{json .}}")?,
                table: false,
                default_table: false,
            });
        }
        let rest = trimmed.strip_prefix("table");
        let table = rest.is_some_and(|r| r.is_empty() || r.starts_with(char::is_whitespace));
        let body = if table {
            rest.unwrap_or_default().trim_matches(' ')
        } else {
            text
        };
        let body = body.replace("\\t", "\t").replace("\\n", "\n");
        Ok(Template {
            nodes: parse::parse(&body)?,
            table,
            default_table: table && body.trim().is_empty(),
        })
    }

    /// `table` alone: the caller prints its own table.
    pub fn is_default_table(&self) -> bool {
        self.default_table
    }

    /// Check each field at the root against the verb's names, with no
    /// record in hand. A field in the body of `range` reads an element and
    /// is checked when it runs.
    ///
    /// ⛔ **Called BEFORE the loop over records.** Measured on 2026-09-08:
    /// with the template checked only inside the loop, `podvm images --format
    /// '{{.Nope}}'` against an EMPTY store printed nothing and exited **0**.
    pub fn check(&self, known: &[&str]) -> Result<(), Bad> {
        check_nodes(&self.nodes, known)
    }

    /// One document. `table` is refused here ([`Bad::Table`]).
    pub fn render(&self, data: &Data) -> Result<String, Bad> {
        if self.table {
            return Err(Bad::Table);
        }
        let mut run = eval::Run::new(data, None);
        run.nodes(&self.nodes)?;
        Ok(run.out)
    }

    /// Each row on its own line; with `table`, a header first and the
    /// columns aligned. `headers` gives the column name of a field where it
    /// is not the words of its name (`ID` of `ps` is `CONTAINER ID`).
    pub fn render_list(&self, rows: &[Data], headers: &[(&str, &str)]) -> Result<String, Bad> {
        let mut text = String::new();
        if self.table {
            let empty = Data::new(Vec::new());
            let name = |chain: &[String]| table::header(chain, headers);
            let mut run = eval::Run::new(&empty, Some(&name));
            run.nodes(&self.nodes)?;
            text.push_str(&run.out);
            text.push('\n');
        }
        for d in rows {
            let mut run = eval::Run::new(d, None);
            run.nodes(&self.nodes)?;
            text.push_str(&run.out);
            text.push('\n');
            if text.len() > eval::MAX_OUTPUT {
                return Err(Bad::TooLarge(format!(
                    "the output of the template is larger than {} bytes",
                    eval::MAX_OUTPUT
                )));
            }
        }
        Ok(if self.table {
            table::align(&text)
        } else {
            text
        })
    }
}

fn check_nodes(nodes: &[Node], known: &[&str]) -> Result<(), Bad> {
    for n in nodes {
        match n {
            Node::Text(_) => {}
            Node::Action(p) => check_pipe(p, known)?,
            // The body reads the elements: only the pipeline is at the root.
            Node::Range(p, _) => check_pipe(p, known)?,
        }
    }
    Ok(())
}

fn check_pipe(p: &Pipe, known: &[&str]) -> Result<(), Bad> {
    for c in &p.cmds {
        let args: &[Arg] = match c {
            Cmd::Call(_, a) => a,
            Cmd::Operand(a) => std::slice::from_ref(a),
        };
        for a in args {
            match a {
                Arg::Field(chain) if !chain.is_empty() => {
                    let full = chain.join(".");
                    let under =
                        (1..=chain.len()).any(|k| known.contains(&chain[..k].join(".").as_str()));
                    let above = known.iter().any(|k| k.starts_with(&format!("{full}.")));
                    if !under && !above {
                        return Err(unknown(&format!(".{full}"), known));
                    }
                }
                Arg::Sub(inner) => check_pipe(inner, known)?,
                _ => {}
            }
        }
    }
    Ok(())
}

/// Parse and check the template of a list verb (`ps`, `images`): `None`
/// where there is none, and for `table` alone, which is the verb's own table.
pub fn list_template(template: Option<&str>, known: &[&str]) -> Result<Option<Template>, Bad> {
    let Some(text) = template else {
        return Ok(None);
    };
    let t = Template::parse(text)?;
    t.check(known)?;
    Ok((!t.is_default_table()).then_some(t))
}

/// Parse `template` and check it for a verb that prints one document.
pub fn check(template: &str, known: &[&str]) -> Result<(), Bad> {
    let t = Template::parse(template)?;
    if t.table {
        return Err(Bad::Table);
    }
    t.check(known)
}

/// The same; `documents` names the fields whose text is a JSON document,
/// a subset of `known`.
pub fn check_with_documents(
    template: &str,
    known: &[&str],
    _documents: &[&str],
) -> Result<(), Bad> {
    check(template, known)
}

/// Render `template` against fields that are text.
#[cfg(test)]
pub fn render(template: &str, fields: &[(&str, String)]) -> Result<String, Bad> {
    Template::parse(template)?.render(&Data::text(fields))
}

/// The same, naming the fields whose values are already JSON documents.
pub fn render_with_documents(
    template: &str,
    fields: &[(&str, String)],
    documents: &[&str],
) -> Result<String, Bad> {
    Template::parse(template)?.render(&Data::documents(fields, documents))
}

/// The same, for a verb whose fields have JSON types.
#[cfg(test)]
pub fn render_typed(
    template: &str,
    fields: &[(&str, Value)],
    plain: &[(&str, String)],
) -> Result<String, Bad> {
    Template::parse(template)?.render(&Data::typed(fields, plain))
}

/// The text that Go prints for a value with `{{.X}}`: `[a b]` for a list,
/// `map[k:v k2:v2]` for a map (keys in order), the number or the word as it
/// is. A null is a dash (AGENTS.md section 5: an unknown value is a dash).
pub fn go_text(v: &Value) -> String {
    match v {
        Value::Null => "-".to_string(),
        Value::String(s) => s.clone(),
        Value::Array(a) => format!("[{}]", a.iter().map(go_text).collect::<Vec<_>>().join(" ")),
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            let items: Vec<String> = keys
                .iter()
                .map(|k| format!("{k}:{}", go_text(&o[*k])))
                .collect();
            format!("map[{}]", items.join(" "))
        }
        other => other.to_string(),
    }
}
