//! The evaluator of a parsed `--format` template against the fields of one
//! row or one document (TODO/cli.md T-2087, OR-B40-15).
//!
//! ⛔ Bounded: the output and the count of steps have a limit, so a
//! `range` in a `range` over a large list ends with a true line and never
//! fills the memory.

use super::parse::{Arg, Cmd, Func, Node, Pipe};
use super::{go_text, Bad, Data};
use serde_json::Value;

/// The largest output of one template, in bytes.
pub const MAX_OUTPUT: usize = 16 << 20;
/// The most steps (an action or a pass of `range`) of one template.
pub const MAX_STEPS: usize = 1_000_000;

/// A value: its JSON, and the text of `{{.X}}` where that is not the Go text
/// of the JSON.
#[derive(Clone, Debug)]
pub struct Val {
    pub json: Value,
    pub plain: Option<String>,
}

impl Val {
    fn of(json: Value) -> Val {
        Val { json, plain: None }
    }
    fn text(s: String) -> Val {
        Val {
            json: Value::String(s),
            plain: None,
        }
    }
    /// The text that `{{.}}` prints.
    pub fn shown(&self) -> String {
        self.plain.clone().unwrap_or_else(|| go_text(&self.json))
    }
}

/// The dot: the root of the data, or a value that `range` gave.
#[derive(Clone)]
enum Dot {
    Root,
    At(Val),
}

/// The column name of a field at the root, for the header of `table`.
pub type HeaderFn<'a> = &'a dyn Fn(&[String]) -> String;

/// One evaluation.
pub struct Run<'a> {
    data: &'a Data,
    /// The header of a `table`: a field at the root gives its column name.
    header: Option<HeaderFn<'a>>,
    steps: usize,
    pub out: String,
}

impl<'a> Run<'a> {
    pub fn new(data: &'a Data, header: Option<HeaderFn<'a>>) -> Run<'a> {
        Run {
            data,
            header,
            steps: 0,
            out: String::new(),
        }
    }

    pub fn nodes(&mut self, nodes: &[Node]) -> Result<(), Bad> {
        self.walk(nodes, &Dot::Root)
    }

    fn step(&mut self) -> Result<(), Bad> {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            return Err(Bad::TooLarge(format!(
                "the template took more than {MAX_STEPS} steps"
            )));
        }
        if self.out.len() > MAX_OUTPUT {
            return Err(Bad::TooLarge(format!(
                "the output of the template is larger than {MAX_OUTPUT} bytes"
            )));
        }
        Ok(())
    }

    fn walk(&mut self, nodes: &[Node], dot: &Dot) -> Result<(), Bad> {
        for n in nodes {
            self.step()?;
            match n {
                Node::Text(t) => self.out.push_str(t),
                Node::Action(p) => {
                    let v = self.pipe(p, dot)?;
                    self.out.push_str(&v.shown());
                }
                Node::Range(p, body) => {
                    let v = self.pipe(p, dot)?;
                    let items: Vec<Value> = match v.json {
                        Value::Array(a) => a,
                        // Go walks a map in the order of its keys.
                        Value::Object(o) => {
                            let mut keys: Vec<&String> = o.keys().collect();
                            keys.sort();
                            keys.iter().map(|k| o[*k].clone()).collect()
                        }
                        // A list that podvm does not know is empty: Go's nil.
                        Value::Null => Vec::new(),
                        other => {
                            return Err(exec_bad(
                                p,
                                &format!(
                                    "range takes a list or a map, and this is {}",
                                    kind(&other)
                                ),
                            ))
                        }
                    };
                    for item in items {
                        self.step()?;
                        self.walk(body, &Dot::At(Val::of(item)))?;
                    }
                }
            }
        }
        Ok(())
    }

    fn pipe(&mut self, p: &Pipe, dot: &Dot) -> Result<Val, Bad> {
        let mut last: Option<Val> = None;
        for c in &p.cmds {
            self.step()?;
            let v = match c {
                Cmd::Operand(a) => {
                    if last.is_some() {
                        return Err(exec_bad(
                            p,
                            "a `|` gives its value to a function, not to a field",
                        ));
                    }
                    self.arg(a, dot, p)?
                }
                Cmd::Call(f, args) => {
                    let mut vals = Vec::with_capacity(args.len() + 1);
                    for a in args {
                        vals.push(self.arg(a, dot, p)?);
                    }
                    if let Some(l) = last.take() {
                        vals.push(l);
                    }
                    call(*f, vals, p)?
                }
            };
            last = Some(v);
        }
        last.ok_or_else(|| exec_bad(p, "the action is empty"))
    }

    fn arg(&mut self, a: &Arg, dot: &Dot, p: &Pipe) -> Result<Val, Bad> {
        Ok(match a {
            Arg::Str(s) => Val::text(s.clone()),
            Arg::Int(n) => Val::of(Value::from(*n)),
            Arg::Sub(inner) => self.pipe(inner, dot)?,
            Arg::Field(chain) => match dot {
                Dot::Root => match self.header {
                    Some(h) if !chain.is_empty() => Val::text(h(chain)),
                    _ => self.data.resolve(chain).map_err(|e| match e {
                        Bad::Exec { why, .. } => exec_bad(p, &why),
                        other => other,
                    })?,
                },
                Dot::At(v) if chain.is_empty() => v.clone(),
                Dot::At(v) => navigate(&v.json, chain, "the element of range")
                    .map_err(|why| exec_bad(p, &why))?,
            },
        })
    }
}

fn exec_bad(p: &Pipe, why: &str) -> Bad {
    Bad::Exec {
        action: p.src.clone(),
        why: why.to_string(),
    }
}

/// The word of a JSON kind, for a message.
pub fn kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "a null (podvm does not know the value)",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "a list",
        Value::Object(_) => "a map",
    }
}

/// The value at `chain` under `v`, or why there is none. `from` names `v`.
pub fn navigate(v: &Value, chain: &[String], from: &str) -> Result<Val, String> {
    let mut at = v;
    let mut path = if from.is_empty() {
        ".".to_string()
    } else {
        from.to_string()
    };
    for key in chain {
        match at {
            Value::Object(o) => match o.get(key) {
                Some(next) => at = next,
                None => {
                    let mut keys: Vec<&String> = o.keys().collect();
                    keys.sort();
                    let keys: Vec<&str> = keys.iter().map(|k| k.as_str()).collect();
                    return Err(format!(
                        "{path} has no key {key:?}; it has: {}",
                        if keys.is_empty() {
                            "nothing".to_string()
                        } else {
                            keys.join(", ")
                        }
                    ));
                }
            },
            other => {
                return Err(format!(
                    "{path} is {}, so it has no field {key:?}",
                    kind(other)
                ))
            }
        }
        path = if path.starts_with('.') && path != "." {
            format!("{path}.{key}")
        } else {
            format!(".{key}")
        };
    }
    Ok(Val::of(at.clone()))
}

fn want(p: &Pipe, f: &str, args: &[Val], n: std::ops::RangeInclusive<usize>) -> Result<(), Bad> {
    if n.contains(&args.len()) {
        return Ok(());
    }
    Err(exec_bad(
        p,
        &format!(
            "{f} takes {} argument(s), and it has {}",
            if n.start() == n.end() {
                n.start().to_string()
            } else {
                format!("{} or more", n.start())
            },
            args.len()
        ),
    ))
}

fn call(f: Func, mut a: Vec<Val>, p: &Pipe) -> Result<Val, Bad> {
    match f {
        Func::Json => {
            want(p, "json", &a, 1..=1)?;
            Ok(Val::text(a[0].json.to_string()))
        }
        Func::Upper | Func::Lower => {
            want(p, "upper and lower", &a, 1..=1)?;
            let s = a[0].shown();
            Ok(Val::text(if f == Func::Upper {
                s.to_uppercase()
            } else {
                s.to_lowercase()
            }))
        }
        Func::Len => {
            want(p, "len", &a, 1..=1)?;
            let n = match &a[0].json {
                Value::Array(x) => x.len(),
                Value::Object(x) => x.len(),
                Value::String(x) => x.len(),
                other => return Err(exec_bad(p, &format!("len of {}", kind(other)))),
            };
            Ok(Val::of(Value::from(n)))
        }
        Func::Println => {
            let words: Vec<String> = a.iter().map(Val::shown).collect();
            Ok(Val::text(format!("{}\n", words.join(" "))))
        }
        Func::Join => {
            want(p, "join", &a, 2..=2)?;
            let Value::String(sep) = &a[1].json else {
                return Err(exec_bad(
                    p,
                    "the second argument of join is the separator, a string",
                ));
            };
            match &a[0].json {
                Value::Array(items) => Ok(Val::text(
                    items.iter().map(go_text).collect::<Vec<_>>().join(sep),
                )),
                other => Err(exec_bad(
                    p,
                    &format!("join takes a list, and this is {}", kind(other)),
                )),
            }
        }
        Func::Index => {
            want(p, "index", &a, 2..=usize::MAX)?;
            let mut at = a.remove(0).json;
            for k in a {
                at = match (at, &k.json) {
                    (Value::Array(items), Value::Number(n)) => {
                        let len = items.len();
                        match n.as_u64().and_then(|i| items.into_iter().nth(i as usize)) {
                            Some(v) => v,
                            None => {
                                return Err(exec_bad(
                                    p,
                                    &format!("the index {n} is out of range: the list has {len}"),
                                ))
                            }
                        }
                    }
                    // Go gives the zero value for a key that a map has not:
                    // docker's labels are a map of strings, so an empty string.
                    (Value::Object(mut o), Value::String(key)) => {
                        o.remove(key).unwrap_or(Value::String(String::new()))
                    }
                    (other, key) => {
                        return Err(exec_bad(
                            p,
                            &format!("index of {} with {} as the key", kind(&other), kind(key)),
                        ))
                    }
                };
            }
            Ok(Val::of(at))
        }
    }
}
