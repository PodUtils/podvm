//! TODO/cli.md T-2108 (OR-B42-08), T-2100 (OR-B41-17): the rows of `ps` in
//! the order, the columns and the words of docker. One function gives the
//! fields of a container ([`ps_fields`]); the table and `--format` read
//! the same fields, so the two cannot differ.
//!
//! docker lists the newest container first. Its CREATED column is the age
//! of the container ("5 minutes ago", the field `.RunningFor`), and its
//! STATUS column has the time of the state: "Up 5 minutes", "Exited (0) 2
//! hours ago", "Created". The words of a span are those of
//! `podvm_image::clock::human`. Where a time does not parse, the state has
//! no time: a dash in CREATED, and "Up" alone.

use podvm_image::clock;
use podvm_supervise::table::{Container, State};

/// The columns of the table: the field and its header, in docker's order.
pub(super) const TABLE: &[(&str, &str)] = &[
    ("ID", "CONTAINER ID"),
    ("Image", "IMAGE"),
    ("Command", "COMMAND"),
    ("RunningFor", "CREATED"),
    ("Status", "STATUS"),
    ("Ports", "PORTS"),
    ("Names", "NAMES"),
];

/// The epoch second of now; 0 where the clock is before the epoch.
pub(super) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// docker's STATUS of `c` at the epoch second `now`.
pub(super) fn status_at(c: &Container, now: i64) -> String {
    let span = |t: &Option<String>| t.as_deref().and_then(|t| clock::span_at(t, now));
    match c.state {
        State::Running => match span(&c.started_at) {
            Some(s) => format!("Up {s}"),
            None => c.status(),
        },
        State::Exited => match span(&c.finished_at) {
            Some(s) => format!("{} {s} ago", c.status()),
            None => c.status(),
        },
        State::Created | State::Dead => c.status(),
    }
}

/// The fields of `ps --format` and of the table for `c` at `now`.
pub(super) fn ps_fields(c: &Container, no_trunc: bool, now: i64) -> Vec<(&'static str, String)> {
    vec![
        ("ID", if no_trunc { c.id.clone() } else { c.short_id() }),
        ("Names", c.name.clone()),
        ("Image", c.image.clone()),
        ("Command", c.argv.join(" ")),
        ("CreatedAt", c.created_at.clone()),
        (
            "RunningFor",
            clock::span_at(&c.created_at, now).map_or_else(|| "-".into(), |s| format!("{s} ago")),
        ),
        ("Status", status_at(c, now)),
        ("State", c.state.word().to_string()),
        // ⛔ Always empty, and the usage says why rather than leaving a reader
        // to wonder whether podvm forgot.
        ("Ports", String::new()),
        (
            "Pid",
            c.pid.map(|p| p.to_string()).unwrap_or_else(|| "0".into()),
        ),
    ]
}

/// The row of the table for `c`: the [`TABLE`] columns of its fields, with
/// the command cut to the width of the column.
pub(super) fn table_row(c: &Container, no_trunc: bool, now: i64) -> Vec<String> {
    let fields = ps_fields(c, no_trunc, now);
    TABLE
        .iter()
        .map(|(key, _)| match *key {
            "Command" => super::command_of(c),
            k => fields
                .iter()
                .find(|(f, _)| *f == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default(),
        })
        .collect()
}

/// The newest container first, as docker lists them. Of two in one second
/// (the time has seconds only), the later in the table comes first. A time
/// that does not parse sorts last.
pub(super) fn newest_first(list: &mut [Container]) {
    list.reverse();
    let key = |c: &Container| {
        (
            clock::span_at(&c.created_at, 0).is_some(),
            c.created_at.clone(),
        )
    };
    list.sort_by_key(|c| std::cmp::Reverse(key(c)));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (state, created, started, finished, exit code, CREATED, STATUS): the
    /// times are seconds before now.
    type Row = (
        State,
        i64,
        Option<i64>,
        Option<i64>,
        Option<i32>,
        &'static str,
        &'static str,
    );

    fn at(secs: i64) -> String {
        clock::iso8601(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64))
    }

    /// One table: the CREATED and STATUS columns of each state against the
    /// times of its record, and the order and headers of the table.
    #[test]
    fn the_rows_of_ps_have_the_order_columns_and_words_of_docker() {
        let now = 1_800_000_000;
        let base = Container {
            id: String::new(),
            name: "n".into(),
            argv: vec!["sleep".into(), "9".into()],
            env: Vec::new(),
            working_dir: "/".into(),
            created_at: String::new(),
            started_at: None,
            finished_at: None,
            state: State::Created,
            pid: None,
            launcher_pid: None,
            exit_code: None,
            noticed: None,
            wall_ms: None,
            end: None,
            rm: false,
            of: podvm_supervise::table::Detail {
                image: "img".into(),
                ..Default::default()
            },
        };
        // (state, created, started, finished, exit code, CREATED, STATUS)
        let rows: &[Row] = &[
            (
                State::Created,
                30,
                None,
                None,
                None,
                "30 seconds ago",
                "Created",
            ),
            (
                State::Running,
                600,
                Some(300),
                None,
                None,
                "10 minutes ago",
                "Up 5 minutes",
            ),
            (
                State::Running,
                5,
                Some(0),
                None,
                None,
                "5 seconds ago",
                "Up Less than a second",
            ),
            (
                State::Exited,
                7200,
                Some(7000),
                Some(3600),
                Some(3),
                "2 hours ago",
                "Exited (3) About an hour ago",
            ),
            (
                State::Exited,
                90_000,
                None,
                None,
                None,
                "25 hours ago",
                "Exited",
            ),
        ];
        let mut list = Vec::new();
        for (state, created, started, finished, code, want_created, want_status) in rows {
            let mut c = base.clone();
            c.state = *state;
            c.created_at = at(now - created);
            c.started_at = started.map(|s| at(now - s));
            c.finished_at = finished.map(|s| at(now - s));
            c.exit_code = *code;
            c.id = format!("{created:064}");
            let row = table_row(&c, false, now);
            assert_eq!(row.len(), TABLE.len());
            assert_eq!(
                (row[3].as_str(), row[4].as_str()),
                (*want_created, *want_status)
            );
            assert_eq!(row[5], "", "PORTS");
            list.push(c);
        }
        let mut odd = base.clone();
        odd.created_at = "yesterday".into();
        odd.state = State::Running;
        odd.started_at = Some("yesterday".into());
        assert_eq!(
            table_row(&odd, false, now)[3..5],
            ["-".to_string(), "Up".into()]
        );
        list.push(odd);
        let mut twin = list[0].clone();
        twin.name = "later".into();
        list.push(twin);
        newest_first(&mut list);
        let names: Vec<&str> = list.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names[1..3], ["later", "n"], "the later of one second first");
        list.retain(|c| c.name != "later");
        let order: Vec<&str> = list.iter().map(|c| c.created_at.as_str()).collect();
        let mut times: Vec<i64> = rows.iter().map(|r| r.1).collect();
        times.sort();
        let mut want: Vec<String> = times.iter().map(|s| at(now - s)).collect();
        want.push("yesterday".into());
        assert_eq!(
            order, want,
            "the newest first, a time that does not parse last"
        );
        let headers: Vec<&str> = TABLE.iter().map(|(_, h)| *h).collect();
        assert_eq!(
            headers,
            [
                "CONTAINER ID",
                "IMAGE",
                "COMMAND",
                "CREATED",
                "STATUS",
                "PORTS",
                "NAMES"
            ]
        );
    }
}
