use super::*;

/// The JSON Schema of layout [`SCHEMA`] (TODO/cli.md T-1752). Each release
/// publishes it as the asset `podvm-run-report.v1.schema.json`; a change
/// of the layout is a new version and a new file.
const SCHEMA_DOC: &str = include_str!("../../../schemas/run-report.v1.schema.json");

fn v(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

#[test]
fn report_has_every_field() {
    let mut r = Report::begin();
    r.labels = v(&["job=7"]);
    r.rung = Some("namespace".into());
    r.ran = true;
    r.end(3);
    let j = r.json();
    for k in [
        "schema",
        "podvm",
        "image",
        "image_digest",
        "rung",
        "measured_by",
        "argv",
        "container",
        "detached",
        "exit_code",
        "signal",
        "refused",
        "started",
        "ended",
        "wall_ms",
        "user_ms",
        "system_ms",
        "max_rss_kib",
        "timed_out",
        "timeout_s",
        "limits",
        "no_new_privs",
        "confine",
        "labels",
        "degradations",
    ] {
        assert!(j.get(k).is_some(), "the report has no {k}: {j}");
    }
    assert_eq!(j["exit_code"], 3);
    assert_eq!(j["signal"], serde_json::Value::Null);
    assert_eq!(j["labels"][0], "job=7");
    assert!(j["wall_ms"].is_u64());
    let mut s = Report::begin();
    s.ran = true;
    s.end(137);
    assert_eq!(s.signal, Some(9));
}

#[test]
fn report_written_on_refusal() {
    let dir = std::env::temp_dir().join(format!("podvm-report-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("r.json");
    let mut r = Report::begin();
    r.end(125);
    r.write(&path).unwrap();
    let j: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(j["refused"], 125);
    assert_eq!(j["exit_code"], serde_json::Value::Null);
    assert_eq!(j["rung"], serde_json::Value::Null);
    let left: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
    assert_eq!(left.len(), 1, "one file, no temporary left: {left:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Whether `value` has a type that the property `prop` of the schema
/// allows: the subset of JSON Schema that the file uses.
fn allows(prop: &serde_json::Value, value: &serde_json::Value) -> bool {
    if let Some(c) = prop.get("const") {
        return c == value;
    }
    let types: Vec<&str> = match &prop["type"] {
        serde_json::Value::String(t) => vec![t.as_str()],
        serde_json::Value::Array(a) => a.iter().filter_map(|t| t.as_str()).collect(),
        _ => return false,
    };
    types.iter().any(|t| match *t {
        "null" => value.is_null(),
        "string" => value.is_string(),
        "boolean" => value.is_boolean(),
        "integer" => value.is_i64() || value.is_u64(),
        "array" => value
            .as_array()
            .is_some_and(|a| a.iter().all(|e| allows(&prop["items"], e))),
        _ => false,
    })
}

/// T-1752 (V-8 of PodUtils/podcrates#124): the schema file describes
/// each record that `json` writes, key for key and type for type.
#[test]
fn each_record_validates_against_the_schema_of_its_version() {
    let schema: serde_json::Value = serde_json::from_str(SCHEMA_DOC).unwrap();
    assert_eq!(schema["properties"]["schema"]["const"], SCHEMA);
    assert_eq!(schema["additionalProperties"], false);
    let props = schema["properties"].as_object().unwrap();
    let required: Vec<&str> = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap())
        .collect();
    let mut ran = Report::begin();
    ran.rung = Some("namespace".into());
    ran.labels = v(&["job=7"]);
    ran.limits = v(&["nofile=64"]);
    ran.ran = true;
    ran.end(0);
    let mut refused = Report::begin();
    refused.end(125);
    let mut detached = Report::begin();
    detached.detached = true;
    detached.container = Some("abc".into());
    detached.ran = true;
    detached.end(0);
    for r in [ran, refused, detached] {
        let j = r.json();
        let obj = j.as_object().unwrap();
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        let mut want = required.clone();
        keys.sort_unstable();
        want.sort_unstable();
        assert_eq!(keys, want, "the record and the required keys differ");
        for (k, value) in obj {
            let prop = props.get(k).unwrap_or_else(|| panic!("no property {k}"));
            assert!(allows(prop, value), "{k} = {value} is not of {prop}");
        }
    }
}

/// TODO/cli.md T-1802: a run refused before `prepare` writes no value
/// of `-e` into its report, and its argv is the payload's command, or
/// empty where the parse did not reach the image.
#[test]
fn report_refusal_holds_no_value_of_e() {
    let dir = std::env::temp_dir().join(format!("podvm-t1802-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("r.json");
    let cases: [(&[&str], &[&str]); 2] = [
        (
            &[
                "--pull",
                "never",
                "-e",
                "API_TOKEN=canary-value",
                "nosuchimage:1",
                "true",
            ],
            &["true"],
        ),
        (
            &[
                "-e",
                "API_TOKEN=canary-value",
                "--no-such-flag",
                "img",
                "true",
            ],
            &[],
        ),
    ];
    for (args, want) in cases {
        let _ = take_parsed();
        let mut r = Report::begin();
        let _ = crate::run::parse::parse("run", &v(args));
        if let Some((image, command)) = take_parsed() {
            r.parsed(image, command);
        }
        r.end(125);
        r.write(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("canary-value"), "{text}");
        let j: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(j["argv"], serde_json::json!(want), "{text}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// TODO/cli.md T-1801: the parser of `run` gives the path. A word after
/// the image is the payload's, and the value of `--name` is a name.
#[test]
fn report_path_is_the_parsers() {
    let path = |a: &[&str]| {
        let _ = take_path();
        let _ = crate::run::parse::parse("run", &v(a));
        take_path()
    };
    assert_eq!(
        path(&["--report", "r.json", "img", "true"]),
        Some("r.json".into())
    );
    assert_eq!(
        path(&["--report=r.json", "img", "true"]),
        Some("r.json".into())
    );
    assert_eq!(path(&["img", "true", "--report", "important.txt"]), None);
    assert_eq!(path(&["--name", "--report", "img", "cmd"]), None);
    assert_eq!(path(&["img", "--", "--report", "x"]), None);
    assert_eq!(path(&["img"]), None);
    // A refusal after the flag keeps the path, so the refusal is reported.
    assert_eq!(
        path(&["--report", "r.json", "--no-such-flag", "img"]),
        Some("r.json".into())
    );
}

/// T-1969: `oom_kills` is an optional field of layout 1: absent where the
/// run has no leaf, and an integer of the schema where it has one.
#[test]
fn the_oom_count_is_an_optional_field_of_the_same_layout() {
    let schema: serde_json::Value = serde_json::from_str(SCHEMA_DOC).unwrap();
    let prop = &schema["properties"]["oom_kills"];
    assert!(!schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r == "oom_kills"));
    let mut r = Report::begin();
    r.ran = true;
    r.end(137);
    r.oom_kills = Some(2);
    let j = r.json();
    assert_eq!(j["oom_kills"], 2);
    assert!(allows(prop, &j["oom_kills"]), "{prop}");
    r.oom_kills = None;
    assert!(r.json().get("oom_kills").is_none());
}
