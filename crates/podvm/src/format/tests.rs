use super::*;
use serde_json::json;

/// The fields of a row: text, a document and typed values, as the verbs
/// give them.
fn data() -> Data {
    Data::typed(
        &[
            ("Repository", json!("alpine")),
            ("Tag", json!("latest")),
            ("Digest", json!("sha256:abc")),
            ("CreatedAt", json!("2026-11-27")),
            ("State", json!({"Status": "running", "Running": true})),
            ("Config.Cmd", json!(["sh", "-c", "true"])),
            ("Config.Labels", json!({"team": "ci", "com.x.k": "v"})),
            ("Config.Env", Value::Null),
            ("Exec.Shares", json!("filesystem")),
            (
                "Mounts",
                json!([{"Source": "/a", "Destination": "/b"}, {"Source": "/c", "Destination": "/d"}]),
            ),
        ],
        &[("State", "running".to_string())],
    )
}

/// TODO/cli.md T-2087 (OR-B40-15): each template against the one row, with
/// its output or the start of its refusal. One rule: a template that podvm
/// reads gives docker's text, and each other one is one line that names the
/// part that fails.
#[test]
fn each_template_gives_its_text_or_one_true_refusal() {
    let deep = format!(
        "{}.Tag{}",
        "{{".to_string() + &"(".repeat(20),
        ")".repeat(20) + "}}"
    );
    let cases: Vec<(&str, Result<&str, &str>)> = vec![
        ("{{.Digest}}", Ok("sha256:abc")),
        ("{{.Repository}}:{{.Tag}} -> {{.Digest}}", Ok("alpine:latest -> sha256:abc")),
        ("no fields here", Ok("no fields here")),
        ("{{.Repository}}\\t{{.Tag}}", Ok("alpine\tlatest")),
        ("{{ .Tag }}", Ok("latest")),
        ("{{.State}}", Ok("running")),
        ("{{.State.Status}}", Ok("running")),
        ("{{json .State}}", Ok(r#"{"Running":true,"Status":"running"}"#)),
        ("{{json .Tag}}", Ok(r#""latest""#)),
        ("{{.Config.Cmd}}", Ok("[sh -c true]")),
        ("{{json .Config.Cmd}}", Ok(r#"["sh","-c","true"]"#)),
        ("{{.Config.Env}}", Ok("-")),
        ("{{.Exec.Shares}}", Ok("filesystem")),
        ("{{json .Exec}}", Ok(r#"{"Shares":"filesystem"}"#)),
        ("{{index .Config.Cmd 0}}", Ok("sh")),
        (r#"{{index .Config.Labels "com.x.k"}}"#, Ok("v")),
        (r#"{{index .Config.Labels "nope"}}"#, Ok("")),
        ("{{.Config.Labels.team}}", Ok("ci")),
        ("{{range .Config.Cmd}}[{{.}}]{{end}}", Ok("[sh][-c][true]")),
        ("{{range .Mounts}}{{.Source}}:{{.Destination}} {{end}}", Ok("/a:/b /c:/d ")),
        ("{{range .Config.Env}}x{{end}}", Ok("")),
        ("{{range .Config.Labels}}{{.}},{{end}}", Ok("v,ci,")),
        (r#"{{join .Config.Cmd ","}}"#, Ok("sh,-c,true")),
        ("{{upper .Tag}}", Ok("LATEST")),
        ("{{.Repository | upper}}", Ok("ALPINE")),
        ("{{lower (upper .Tag)}}", Ok("latest")),
        ("{{len .Config.Cmd}}", Ok("3")),
        ("{{println .Tag .Repository}}", Ok("latest alpine\n")),
        ("{{json (index .Mounts 1)}}", Ok(r#"{"Destination":"/d","Source":"/c"}"#)),
        ("a {{- \" b \" -}} c", Ok("a b c")),
        ("{{/* a comment */}}x", Ok("x")),
        ("json", Ok("~\"CreatedAt\":\"2026-11-27\"")),
        ("{{json .}}", Ok("~\"Exec\":{\"Shares\":\"filesystem\"}")),
        // The refusals.
        ("{{.Nope}}", Err("there is no field \".Nope\". This verb has: .Repository")),
        ("{{json .Nope}}", Err("there is no field \".Nope\"")),
        ("{{.Exec.Nope}}", Err("in {{.Exec.Nope}}: .Exec has no key \"Nope\"; it has: Shares")),
        ("{{.Tag", Err("the template has a `{{` that is never closed")),
        ("{{if .Tag}}x{{end}}", Err("`if` is not a part of Go's templates")),
        ("{{printf \"%s\" .Tag}}", Err("`printf (a function)` is not a part")),
        ("{{range $i, $e := .Mounts}}{{end}}", Err("`$ (a variable)` is not a part")),
        ("{{range .Config.Cmd}}x", Err("in {{range .Config.Cmd}}: the `range` has no `{{end}}`")),
        ("x{{end}}", Err("in {{end}}: an `end` with no `range` before it")),
        ("{{index .Config.Cmd 5}}", Err("in {{index .Config.Cmd 5}}: the index 5 is out of range: the list has 3")),
        ("{{index .Tag 0}}", Err("in {{index .Tag 0}}: index of a string")),
        ("{{index .Config.Env 0}}", Err("index of a null")),
        ("{{index .Config.Cmd}}", Err("index takes 2 or more argument(s), and it has 1")),
        ("{{join .Tag \",\"}}", Err("join takes a list, and this is a string")),
        ("{{range .Tag}}x{{end}}", Err("range takes a list or a map, and this is a string")),
        ("{{range .Mounts}}{{.Nope}}{{end}}", Err("in {{.Nope}}: the element of range has no key \"Nope\"; it has: Destination, Source")),
        ("{{.Config.Labels.nope}}", Err(".Config.Labels has no key \"nope\"; it has: com.x.k, team")),
        ("{{.Tag .Digest}}", Err("only a function takes arguments")),
        ("{{.Tag | .Digest}}", Err("a `|` gives its value to a function")),
        ("{{(index .Mounts 0).Source}}", Err("a field of the value of `( )`")),
        ("{{}}", Err("the action is empty")),
        ("table {{.Tag}}", Err("the `table` prefix lays out the rows of a list verb")),
        (deep.as_str(), Err("deeper than 16 levels")),
    ];
    let d = data();
    let mut bad = Vec::new();
    for (template, want) in &cases {
        let got = Template::parse(template).and_then(|t| t.render(&d));
        let ok = match (&got, want) {
            (Ok(text), Ok(w)) => match w.strip_prefix('~') {
                Some(part) => text.starts_with('{') && text.contains(part),
                None => text == w,
            },
            (Err(e), Err(w)) => {
                let line = e.to_string();
                line.contains(w) && !line.contains('\n')
            }
            _ => false,
        };
        if !ok {
            bad.push(format!(
                "{template:?}: {:?}",
                got.map_err(|e| e.to_string())
            ));
        }
    }
    // The bounds: a template too large and an output too large.
    let big = "x".repeat(parse::MAX_TEMPLATE + 1);
    if !matches!(Template::parse(&big), Err(Bad::TooLarge(_))) {
        bad.push("a template over the limit parsed".into());
    }
    let wide = Data::typed(&[("L", json!(vec![1; 4096]))], &[]);
    let t = Template::parse(&format!("{{{{range .L}}}}{}{{{{end}}}}", "y".repeat(8192))).unwrap();
    if !matches!(t.render(&wide), Err(Bad::TooLarge(_))) {
        bad.push("an output over the limit was given".into());
    }
    // The check with no record: a field at the root against the names.
    let known = ["Tag", "Exec.Shares", "Config"];
    for (template, ok) in [
        ("{{.Tag}}", true),
        ("{{.Exec}}", true),
        ("{{.Config.Labels.x}}", true),
        ("{{range .Config.Cmd}}{{.Anything}}{{end}}", true),
        ("{{.Nope}}", false),
        ("{{upper (index .Nope 0)}}", false),
        ("{{range .Nope}}{{end}}", false),
    ] {
        if check(template, &known).is_ok() != ok {
            bad.push(format!("check {template:?} is not {ok}"));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// The rows of a list verb with `table`: a header from the names, and the
/// columns aligned as docker's tabwriter aligns them (at least 10 wide, 3
/// spaces after the widest cell).
#[test]
fn a_table_has_a_header_and_aligned_columns() {
    let rows = vec![
        Data::text(&[
            ("ID", "abc".into()),
            ("CreatedAt", "now".into()),
            ("Names", "web".into()),
        ]),
        Data::text(&[
            ("ID", "abcdefabcdef".into()),
            ("CreatedAt", "then".into()),
            ("Names", "db".into()),
        ]),
    ];
    let headers = [("ID", "CONTAINER ID")];
    let cases: Vec<(&str, &str)> = vec![
        (
            "table {{.ID}}\\t{{.CreatedAt}}\\t{{.Names}}",
            "CONTAINER ID   CREATED AT   NAMES\nabc            now          web\nabcdefabcdef   then         db\n",
        ),
        ("table {{.Names}}", "NAMES\nweb\ndb\n"),
        ("{{.Names}}", "web\ndb\n"),
        ("table {{lower .Names}}:{{.ID}}", "names:CONTAINER ID\nweb:abc\ndb:abcdefabcdef\n"),
    ];
    for (template, want) in cases {
        let t = Template::parse(template).unwrap();
        assert_eq!(t.render_list(&rows, &headers).unwrap(), want, "{template}");
    }
    assert!(Template::parse("table").unwrap().is_default_table());
    assert!(!Template::parse("tablet {{.ID}}")
        .unwrap()
        .is_default_table());
}
