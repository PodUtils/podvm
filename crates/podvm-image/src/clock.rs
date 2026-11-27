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

/// How long ago, in docker's `CreatedSince` wording: [`human`] and "ago".
/// ⛔ Returns `None` where the input does not parse, so the caller prints a
/// dash: `AGENTS.md` absolute 3, a dash where a value is unknown.
pub fn since(iso: &str) -> Option<String> {
    span(iso).map(|s| format!("{s} ago"))
}

/// The time from `iso` to now, in docker's words ([`human`]); `None` where
/// `iso` does not parse.
pub fn span(iso: &str) -> Option<String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    span_at(iso, i64::try_from(now).ok()?)
}

/// [`span`] at the epoch second `now`.
pub fn span_at(iso: &str, now: i64) -> Option<String> {
    Some(human(now.saturating_sub(parse_iso8601(iso)?)))
}

/// A span of `secs` seconds in the words of docker (`HumanDuration` of
/// go-units): the CREATED and STATUS columns of `ps` and the CREATED column
/// of `images` (TODO/cli.md T-2108, OR-B42-08). A span below zero (a clock
/// that went back) is "Less than a second".
pub fn human(secs: i64) -> String {
    let minutes = secs / 60;
    let hours = (secs + 1800) / 3600;
    match secs {
        s if s < 1 => "Less than a second".into(),
        1 => "1 second".into(),
        s if s < 60 => format!("{s} seconds"),
        _ if minutes == 1 => "About a minute".into(),
        _ if minutes < 60 => format!("{minutes} minutes"),
        _ if hours == 1 => "About an hour".into(),
        _ if hours < 48 => format!("{hours} hours"),
        _ if hours < 24 * 7 * 2 => format!("{} days", hours / 24),
        _ if hours < 24 * 30 * 2 => format!("{} weeks", hours / 24 / 7),
        _ if hours < 24 * 365 * 2 => format!("{} months", hours / 24 / 30),
        s => format!("{} years", s / 3600 / 24 / 365),
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

    /// One table: the words of a span, at each edge of each unit of
    /// go-units' `HumanDuration`, and the age of a time before now.
    #[test]
    fn an_age_is_worded_the_way_docker_words_it() {
        let (m, h, d) = (60, 3600, 86_400);
        for (secs, want) in [
            (-5, "Less than a second"),
            (0, "Less than a second"),
            (1, "1 second"),
            (59, "59 seconds"),
            (60, "About a minute"),
            (119, "About a minute"),
            (120, "2 minutes"),
            (59 * m + 59, "59 minutes"),
            (h, "About an hour"),
            (h + 29 * m, "About an hour"),
            (h + 30 * m, "2 hours"),
            (47 * h, "47 hours"),
            (2 * d, "2 days"),
            (13 * d, "13 days"),
            (14 * d, "2 weeks"),
            (59 * d, "8 weeks"),
            (60 * d, "2 months"),
            (729 * d, "24 months"),
            (730 * d, "2 years"),
        ] {
            assert_eq!(human(secs), want, "{secs}");
        }
        let at = 1_709_164_800 + 90;
        assert_eq!(
            span_at("2024-02-29T00:00:00Z", at).as_deref(),
            Some("About a minute")
        );
        assert!(span_at("yesterday", at).is_none());
        assert!(since(&now()).unwrap().ends_with(" ago"));
    }
}
