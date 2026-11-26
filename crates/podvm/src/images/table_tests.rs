//! TODO/image.md T-1840: `images` tells two platform records apart, lists
//! each tag of a repository named with no tag, and `inspect` gives
//! `.Config.Labels`.

use super::table_more::{labels_json, needs_platform, repository_filter};
use podvm_image::{clock, digest::Digest, Record, Store};

fn store(tag: &str) -> Store {
    let d = std::env::temp_dir().join(format!("podvm-table-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    Store::open(d).unwrap()
}

fn record(s: &Store, tag: &str, platform: &str, config: &serde_json::Value) -> Record {
    let bytes = serde_json::to_vec(config).unwrap();
    let want = Digest::of(&bytes);
    s.put_bytes(&bytes, &want, "a config of a test").unwrap();
    let arch = platform.rsplit('/').next().unwrap().to_string();
    Record {
        repository: "docker.io/library/alpine".into(),
        tag: Some(tag.into()),
        digest: format!("sha256:{}", "1".repeat(64)),
        digest_media_type: podvm_image::oci::MEDIA_OCI_INDEX.into(),
        manifest_digest: format!("sha256:{}{}", tag.len(), "2".repeat(63)),
        config_digest: want.to_string(),
        platform: platform.into(),
        layers: vec![],
        stored_bytes: 0,
        unpacked_bytes: None,
        architecture: arch,
        os: "linux".into(),
        created: None,
        pulled_at: clock::now(),
    }
}

#[test]
fn images_table_tells_platforms_apart_and_lists_each_tag() {
    let s = store("tags");
    let none = serde_json::json!({"config": {}});
    let a = record(&s, "3.20", "linux/amd64", &none);
    let b = record(&s, "3.20", "linux/arm64", &none);
    assert!(needs_platform(&[a.clone(), b.clone()]));
    assert!(!needs_platform(std::slice::from_ref(&a)));
    s.put_record(a).unwrap();
    s.put_record(record(&s, "3.18", "linux/amd64", &none))
        .unwrap();
    let got = repository_filter(&s, "alpine").unwrap().unwrap();
    let mut tags: Vec<_> = got.iter().filter_map(|r| r.tag.clone()).collect();
    tags.sort();
    assert_eq!(tags, ["3.18", "3.20"]);
    assert!(repository_filter(&s, "alpine:3.20").is_none());
    let _ = std::fs::remove_dir_all(s.root());
}

#[test]
fn images_table_inspect_gives_config_labels() {
    let s = store("labels");
    let with = record(
        &s,
        "l",
        "linux/amd64",
        &serde_json::json!({"config": {"Labels": {"team": "pod", "tier": "1"}}}),
    );
    assert_eq!(
        labels_json(&s, &with),
        serde_json::json!({"team": "pod", "tier": "1"})
    );
    let without = record(&s, "n", "linux/amd64", &serde_json::json!({"config": {}}));
    assert_eq!(labels_json(&s, &without), serde_json::Value::Null);
    let _ = std::fs::remove_dir_all(s.root());
}

/// TODO/image.md T-1841: `save --platform` parses, and it selects the
/// record of that platform among two of one reference.
#[test]
fn save_platform_selects_that_record() {
    let s = store("save");
    let none = serde_json::json!({"config": {}});
    s.put_record(record(&s, "3.20", "linux/amd64", &none))
        .unwrap();
    s.put_record(record(&s, "3.20", "linux/arm64", &none))
        .unwrap();
    let args: Vec<String> = ["--platform", "linux/arm64", "-o", "x.tar", "alpine:3.20"]
        .iter()
        .map(|a| a.to_string())
        .collect();
    let o = super::save_load::parse_save(&args).unwrap();
    assert_eq!(o.platform.as_deref(), Some("linux/arm64"));
    let p = podvm_image::platform::wanted(o.platform.as_deref()).unwrap();
    let r = s.find_one_for(&o.want, Some(&p)).unwrap();
    assert_eq!(r.platform, "linux/arm64");
    let _ = std::fs::remove_dir_all(s.root());
}
