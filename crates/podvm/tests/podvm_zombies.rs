//! TODO/cli.md T-2087 (OR-B40-05, the follow-up of experiment 485): each
//! process that podvm starts and that does not outlive the verb by design
//! is reaped by podvm, not by the init of the host. The test makes itself
//! the child subreaper, so each process of podvm that outlives its parent
//! becomes a child of the test, and a zombie of it is counted here at once
//! (an init that reaps late, as on the cloud host, hides none).
//!
//! By design one process outlives a verb: the launcher of `run -d`, which
//! `rm -f` ends. Its parent is then the subreaper or the init of the caller,
//! as with the shim of docker. The image is an OCI layout with the host's
//! `dash` as `/bin/sh` and `sleep`. A host with no glibc or no `dash` at the
//! x86_64 paths skips the test with a line on stderr.

mod host_image;
use host_image::{podvm, text};

const IMAGE: &str = "localhost/podvmzombies:1";
const NAME: &str = "podvmzombies";

unsafe extern "C" {
    fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
    fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
}

/// The children of this process in state `Z` whose name starts with
/// `podvm`; each one is reaped. The others are reaped too, and not counted.
fn reap_zombies() -> Vec<String> {
    let me = std::process::id().to_string();
    let mut found = Vec::new();
    let Ok(rd) = std::fs::read_dir("/proc") else {
        return found;
    };
    for e in rd.flatten() {
        let Ok(stat) = std::fs::read_to_string(e.path().join("stat")) else {
            continue;
        };
        let (Some(open), Some(close)) = (stat.find('('), stat.rfind(')')) else {
            continue;
        };
        let comm = &stat[open + 1..close];
        let rest: Vec<&str> = stat[close + 1..].split_whitespace().collect();
        if rest.first() != Some(&"Z") || rest.get(1) != Some(&me.as_str()) {
            continue;
        }
        let Ok(pid) = e.file_name().to_string_lossy().parse::<i32>() else {
            continue;
        };
        let mut st = 0;
        // SAFETY: a zombie child of this process; st lives for the call.
        unsafe { waitpid(pid, &mut st, 0) };
        if comm.starts_with("podvm") {
            found.push(format!("{pid} ({comm})"));
        }
    }
    found
}

#[test]
fn each_verb_reaps_each_process_of_podvm_that_ends_with_it() {
    let Some(tar) = host_image::layout(IMAGE, &["sleep"]) else {
        eprintln!("T-2087: this host has no glibc or dash at the x86_64 paths; skipped");
        return;
    };
    // SAFETY: PR_SET_CHILD_SUBREAPER (36) with 1.
    if unsafe { prctl(36, 1, 0, 0, 0) } != 0 {
        eprintln!("T-2087: the kernel refused PR_SET_CHILD_SUBREAPER; skipped");
        return;
    }
    let (dir, store) = host_image::store_with(NAME, tar);
    // (the verb, its arguments, the zombies of podvm that it may leave)
    let steps: &[(&str, &[&str], usize)] = &[
        ("system info", &["system", "info"], 0),
        ("a foreground run", &["run", IMAGE, "sh", "-c", ":"], 0),
        (
            "run -d",
            &["run", "-d", "--name", NAME, IMAGE, "sleep", "60"],
            0,
        ),
        ("exec", &["exec", NAME, "sh", "-c", ":"], 0),
        // The launcher of `run -d` outlives that verb by design.
        ("rm -f", &["rm", "-f", NAME], 1),
    ];
    let mut failures = Vec::new();
    for (what, args, allowed) in steps {
        let r = podvm(&store, args);
        // A process that ends just after the verb: a short wait.
        std::thread::sleep(std::time::Duration::from_millis(300));
        let z = reap_zombies();
        if !r.status.success() {
            failures.push(format!(
                "{what}: exit {:?}, err {:?}",
                r.status.code(),
                text(&r.stderr)
            ));
        }
        if z.len() > *allowed {
            failures.push(format!("{what}: zombies of podvm {z:?}, at most {allowed}"));
        }
    }
    let _ = podvm(&store, &["rm", "-f", NAME]);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
