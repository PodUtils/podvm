//! TODO/cli.md T-2100 (OR-B41-06 of the tenth outside pass): a foreground
//! `run --rm` whose reader of stdout and stderr goes away removes its
//! record. Before, the payload died of SIGPIPE, the pump of stderr closed
//! its pipe, and the next line of podvm on stderr (the note of the exit)
//! ended podvm by SIGPIPE: the record stayed (`ps -a`: Dead), 3 of 3 runs.

mod host_image;
use host_image::{podvm, text};

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

const IMAGE: &str = "localhost/t2100:1";

#[test]
fn a_run_rm_whose_reader_left_removes_its_record() {
    let Some(tar) = host_image::layout(IMAGE, &[]) else {
        eprintln!("T-2100: this host has no glibc or dash at the x86_64 paths; skipped");
        return;
    };
    let (dir, store) = host_image::store_with("t2100", tar);
    // (the stream that the payload writes without end)
    for to in [">&2", ""] {
        let script = format!("while :; do echo x {to}; done");
        let (reader, writer) = std::io::pipe().unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_podvm"))
            .env("PODVM_SPARE_TREE", "0")
            .env("PODVM_STORE", &store)
            .args(["run", "--rm", IMAGE, "sh", "-c", &script])
            .stdin(Stdio::null())
            .stdout(writer.try_clone().unwrap())
            .stderr(writer)
            .spawn()
            .unwrap();
        // The reader takes the first line of the payload and goes, as
        // `| head -1` does.
        let mut lines = BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            if lines.read_line(&mut line).unwrap_or(0) == 0 || line == "x\n" {
                break;
            }
        }
        drop(lines);
        let start = std::time::Instant::now();
        let status = loop {
            if let Some(s) = child.try_wait().unwrap() {
                break s;
            }
            if start.elapsed() > std::time::Duration::from_secs(60) {
                let _ = child.kill();
                panic!("{to:?}: podvm did not end in 60 s");
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        // The payload's code: 141, its SIGPIPE, as with no podvm.
        assert_eq!(status.code(), Some(141), "{to:?}: {status:?}");
        let ps = podvm(&store, &["ps", "-a", "-q"]);
        assert_eq!(text(&ps.stdout).trim(), "", "{to:?}: a record stays");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
