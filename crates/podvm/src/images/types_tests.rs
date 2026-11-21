use super::tests::a_record;
use super::*;

/// TODO/cli.md T-2060 (OQ-06): each field of `inspect --format` of an image
/// has docker's JSON type, and `{{json .X}}` and `{{.X}}` read the one
/// document that plain `inspect` prints. One row for each field, one rule.
#[test]
fn the_fields_of_an_image_have_dockers_types() {
    let store =
        Store::open(std::env::temp_dir().join(format!("podvm-types-{}", std::process::id())))
            .unwrap();
    let r = Record {
        tag: Some("latest".into()),
        layers: vec!["sha256:aa".into(), "sha256:bb".into()],
        stored_bytes: 42,
        ..a_record()
    };
    let name = r.name();
    let fields = inspect_fields(&r, &store);
    let doc: serde_json::Value = serde_json::from_str(&inspect_json(&r, &store)).unwrap();
    // (field, JSON of docker's type, the text of `{{.X}}`)
    let table: [(&str, serde_json::Value, String); 9] = [
        ("Size", serde_json::json!(42), "42".into()),
        ("RepoTags", serde_json::json!([name]), format!("[{name}]")),
        (
            "Layers",
            serde_json::json!(["sha256:aa", "sha256:bb"]),
            "[sha256:aa sha256:bb]".into(),
        ),
        ("Extracted", serde_json::json!(false), "false".into()),
        ("Os", serde_json::json!("linux"), "linux".into()),
        ("Config.Env", serde_json::Value::Null, "-".into()),
        ("Config.Cmd", serde_json::Value::Null, "-".into()),
        ("Config.Labels", serde_json::Value::Null, "-".into()),
        (
            "Exec.Mode",
            serde_json::json!("fresh-chroot"),
            "fresh-chroot".into(),
        ),
    ];
    for (field, json, plain) in &table {
        let at = fields.iter().find(|(k, _)| k == field).unwrap().1.clone();
        assert_eq!(&at, json, "{field}");
        let path = field.split('.').try_fold(&doc, |v, p| v.get(p)).unwrap();
        assert_eq!(path, json, "{field}: the document and the field differ");
        let t = |x: &str| format::render_typed(x, &fields, &[]).unwrap();
        assert_eq!(
            t(&format!("{{{{json .{field}}}}}")),
            json.to_string(),
            "{field}"
        );
        assert_eq!(t(&format!("{{{{.{field}}}}}")), *plain, "{field}");
    }
    // A map and a list as docker prints them with a template.
    let cfg = serde_json::json!({"Cmd": ["python3", "-m"], "Labels": {"b": "2", "a": "1"}});
    assert_eq!(format::go_text(&cfg["Cmd"]), "[python3 -m]");
    assert_eq!(format::go_text(&cfg["Labels"]), "map[a:1 b:2]");
    let _ = std::fs::remove_dir_all(store.root());
}
