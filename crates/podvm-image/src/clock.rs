//! ISO 8601 UTC, formatted at the edge.
//!
//! `docs/conventions/code.md`: timestamps stored as ISO 8601 UTC. ⚠ The warning
//! there is the reason this is not a number of seconds in the store file: a
//! format that will never string-compare against an ISO column silently returns
//! false forever, and `podvm images` sorts by this field.
//!
//! The calendar is `pod_core::clock` (the ruling "Shared code",
//! TODO/deps.md T-1955); this module keeps the docker wording of an age.

use std::time::{SystemTime, UNIX_EPOCH};

/// `YYYY-MM-DDTHH:MM:SSZ`; a time before the epoch is the epoch, which sorts
/// before each real entry and says that the reading could not be made.
pub fn iso8601(t: SystemTime) -> String {
    pod_core::clock::rfc3339(t)
}

pub fn now() -> String {
    iso8601(SystemTime::now())
}

/// How long ago, in docker's `CreatedSince` wording. ⛔ Returns `None` where the
/// input does not parse, so the caller prints a dash: `AGENTS.md` absolute
/// 3, a dash where a value is unknown.
pub fn since(iso: &str) -> Option<String> {
    let then = parse_iso8601(iso)?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    let d = now - then;
    if d < 0 {
        return Some("just now".into());
    }
    Some(match d {
        s if s < 60 => format!("{s} second{} ago", plural(s)),
        s if s < 3600 => format!("{} minute{} ago", s / 60, plural(s / 60)),
        s if s < 86_400 => format!("{} hour{} ago", s / 3600, plural(s / 3600)),
        s if s < 2_592_000 => format!("{} day{} ago", s / 86_400, plural(s / 86_400)),
        s if s < 31_536_000 => format!("{} month{} ago", s / 2_592_000, plural(s / 2_592_000)),
        s => format!("{} year{} ago", s / 31_536_000, plural(s / 31_536_000)),
    })
}

fn plural(n: i64) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// `YYYY-MM-DDTHH:MM:SS` with anything after the seconds ignored, into epoch
/// seconds. ⚠ Fractional seconds and a `+00:00` offset both appear in image
/// configs; only `Z` and a truncated offset are read, and anything else returns
/// `None` rather than a number computed from a shape this does not understand.
fn parse_iso8601(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b' ') {
        return None;
    }
    let n = |a: usize, z: usize| -> Option<i64> { s.get(a..z)?.parse::<i64>().ok() };
    let (y, mo, d) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let (h, mi, sec) = (n(11, 13)?, n(14, 16)?, n(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    Some(pod_core::clock::days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + sec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn the_epoch_and_two_known_instants_render_exactly() {
        assert_eq!(iso8601(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        assert_eq!(
            iso8601(UNIX_EPOCH + Duration::from_secs(1_000_000_000)),
            "2001-09-09T01:46:40Z"
        );
        // A leap day, which is where a hand-written calendar goes wrong.
        assert_eq!(
            iso8601(UNIX_EPOCH + Duration::from_secs(1_709_164_800)),
            "2024-02-29T00:00:00Z"
        );
    }

    #[test]
    fn a_config_timestamp_with_fractional_seconds_parses() {
        assert_eq!(
            parse_iso8601("2024-02-29T00:00:00.123456789Z"),
            Some(1_709_164_800)
        );
        assert_eq!(parse_iso8601("2024-02-29T00:00:00Z"), Some(1_709_164_800));
    }

    #[test]
    fn an_unparseable_timestamp_is_none_rather_than_a_number() {
        // ⛔ AGENTS.md absolute 3: a dash where the value is unknown.
        assert!(parse_iso8601("yesterday").is_none());
        assert!(parse_iso8601("2024-13-01T00:00:00Z").is_none());
        assert!(since("not a date").is_none());
    }

    #[test]
    fn an_age_is_worded_the_way_docker_words_it() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let ago = |s: i64| since(&iso8601(UNIX_EPOCH + Duration::from_secs((now - s) as u64)));
        assert_eq!(ago(1).unwrap(), "1 second ago");
        assert_eq!(ago(90).unwrap(), "1 minute ago");
        assert_eq!(ago(7200).unwrap(), "2 hours ago");
        assert_eq!(ago(90_000).unwrap(), "1 day ago");
    }
}
