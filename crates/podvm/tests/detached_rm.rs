//! TODO/cli.md T-2074 (OR-B38-04 of the seventh outside pass): `--rm` of
//! `run -d` and of `create` removes the container, its record and its tree
//! after the end of its run, as docker does; `wait` still gets the code.
//! Before, podvm accepted the flag and kept the container `Exited`.

mod host_image;
use host_image::{podvm, text};
use std::time::{Duration, Instant};

const IMAGE: &str = "localhost/t2074:1";

#[test]
fn rm_of_a_detached_run_removes_the_container_at_its_end() {
    let Some(tar) = host_image::layout(IMAGE, &["sleep"]) else {
        eprintln!("T-2074: this host has no glibc or dash at the x86_64 paths; skipped");
        return;
    };
    let (dir, store) = host_image::store_with("t2074", tar);
    let trees = || {
        std::fs::read_dir(store.join("trees"))
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                    .count()
            })
            .unwrap_or(0)
    };
    // The first run extracts the tree of the image, which stays.
    let warm = podvm(&store, &["run", "--rm", IMAGE, "sh", "-c", "exit 0"]);
    assert!(warm.status.success(), "{}", text(&warm.stderr));
    let before = trees();
    let listed = |name: &str| text(&podvm(&store, &["ps", "-a"]).stdout).contains(name);
    // (the verbs that start the container, its name, the code of `wait`)
    type Case<'a> = (&'a [&'a [&'a str]], &'a str, Option<&'a str>);
    let cases: &[Case] = &[
        (
            &[&[
                "run", "-d", "--rm", "--name", "rm-a", IMAGE, "sh", "-c", "exit 0",
            ]],
            "rm-a",
            None,
        ),
        (
            &[&[
                "run",
                "-d",
                "--rm",
                "--name",
                "rm-b",
                IMAGE,
                "sh",
                "-c",
                "sleep 1; exit 3",
            ]],
            "rm-b",
            Some("3"),
        ),
        (
            &[
                &[
                    "create", "--rm", "--name", "rm-c", IMAGE, "sh", "-c", "exit 0",
                ],
                &["start", "rm-c"],
            ],
            "rm-c",
            None,
        ),
    ];
    let mut failures = Vec::new();
    for (verbs, name, code) in cases {
        for v in *verbs {
            let r = podvm(&store, v);
            assert!(r.status.success(), "{v:?}: {}", text(&r.stderr));
        }
        if let Some(want) = code {
            let w = podvm(&store, &["wait", name]);
            if text(&w.stdout).trim() != *want {
                failures.push(format!("{name}: wait gave {:?}", text(&w.stdout)));
            }
        }
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end && listed(name) {
            std::thread::sleep(Duration::from_millis(200));
        }
        if listed(name) {
            failures.push(format!("{name}: ps -a lists it after its end"));
        }
    }
    if trees() != before {
        failures.push(format!(
            "trees of containers: {before} before, {} after",
            trees()
        ));
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
