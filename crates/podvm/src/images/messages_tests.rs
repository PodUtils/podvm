//! TODO/image.md T-1836: the texts of `save` and `image prune`.

use super::*;

/// TODO/image.md T-1836: the size of a save shows once, and the total of
/// `image prune` has its blank line only after a list.
#[test]
fn image_messages_say_each_thing_once() {
    let done = podvm_image::layout::Saved {
        record: podvm_image::Record {
            repository: "r".into(),
            tag: None,
            digest: String::new(),
            digest_media_type: String::new(),
            manifest_digest: "sha256:0".into(),
            config_digest: String::new(),
            platform: "linux/amd64".into(),
            layers: Vec::new(),
            stored_bytes: 0,
            unpacked_bytes: None,
            architecture: "amd64".into(),
            os: "linux".into(),
            created: None,
            pulled_at: String::new(),
        },
        blobs: 3,
        bytes: 3 << 20,
    };
    let line = saved_line("r:1", &done);
    assert!(
        line.contains("(3.0 MiB)") && !line.contains("bytes"),
        "{line}"
    );
    assert_eq!(prune_total(false, 0), "Total reclaimed space: 0 B\n");
    assert!(prune_total(true, 0).starts_with("\nTotal"));
}
