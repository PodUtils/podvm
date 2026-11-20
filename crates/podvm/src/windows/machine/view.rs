//! TODO/machine.md T-1869, T-1870: the verbs that read a machine: `logs`,
//! the rows of `ps`, and `podvm windows screenshot`.

use std::path::PathBuf;
use std::time::Duration;

use podvm_image::error::{EXIT_FLAG_ERROR, EXIT_RUNTIME_ERROR};
use podvm_windows::machines::{self, Record};

use super::find;

/// `podvm windows screenshot NAME FILE`: a screen dump (PPM) of a running
/// machine, through QMP `screendump` (TODO/machine.md T-1869).
pub(crate) fn screenshot(args: &[String]) -> i32 {
    let (Some(name), Some(file)) = (args.first(), args.get(1)) else {
        eprintln!("usage: podvm windows screenshot NAME FILE.ppm");
        return EXIT_FLAG_ERROR;
    };
    let Some(r) = find(name).filter(Record::running) else {
        eprintln!("podvm windows screenshot: no running machine {name:?} (`podvm ps` lists them)");
        return EXIT_RUNTIME_ERROR;
    };
    let file = std::path::absolute(file).unwrap_or_else(|_| PathBuf::from(file));
    let sock = machines::socket(&machines::root(), &r.name);
    match podvm_windows::qmp::Monitor::connect(&sock, Duration::from_secs(5))
        .and_then(|mut m| m.screendump(&file))
    {
        Ok(()) => {
            println!("{}", file.display());
            0
        }
        Err(e) => {
            eprintln!("podvm windows screenshot: {name}: {e}");
            EXIT_RUNTIME_ERROR
        }
    }
}

/// `podvm logs [-f] NAME` of a machine: the serial line of the guest; with
/// `follow`, the new bytes while the machine runs.
pub(crate) fn logs(name: &str, follow: bool) -> i32 {
    use std::io::{Read, Seek, Write};
    let root = machines::root();
    let path = machines::dir_in(&root, name).join("serial.log");
    let mut out = std::io::stdout().lock();
    let mut at = 0u64;
    loop {
        if let Ok(mut f) = std::fs::File::open(&path)
            && f.seek(std::io::SeekFrom::Start(at)).is_ok()
        {
            let mut buf = Vec::new();
            if f.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
                at += buf.len() as u64;
                if out.write_all(&buf).and_then(|()| out.flush()).is_err() {
                    return 0;
                }
            }
        }
        if !follow || !machines::read(&root, name).is_some_and(|r| r.running()) {
            return 0;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// `podvm port NAME [PORT[/PROTO]]` (TODO/machine.md T-1869): the forwards
/// of a machine, as docker prints them (`3389/tcp -> 127.0.0.1:33890`). A
/// name of no machine keeps the refusal of the parity table: a container
/// shares the network of this machine and publishes nothing.
pub(crate) fn port(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("usage: podvm port NAME [PORT[/PROTO]]");
        return 0;
    }
    let Some(name) = args.first().filter(|a| !a.starts_with('-')) else {
        eprintln!("podvm port: a NAME is required\nusage: podvm port NAME [PORT[/PROTO]]");
        return podvm_image::error::EXIT_CLI_ERROR; // PodUtils/podvm#519
    };
    let Some(r) = find(name) else {
        eprintln!(
            "podvm: port: podvm publishes no ports of a container: the payload shares this \
             machine's network namespace; a detached Windows guest publishes with -p"
        );
        return EXIT_RUNTIME_ERROR;
    };
    let want = args.get(1).map(|p| {
        if p.contains('/') {
            p.clone()
        } else {
            format!("{p}/tcp")
        }
    });
    let mut any = false;
    for f in r
        .forwards
        .split(',')
        .filter_map(crate::run::publish::forward_of)
    {
        let key = format!("{}/{}", f.guest_port, f.proto.word());
        if want.as_ref().is_none_or(|w| *w == key) {
            println!("{key} -> {}:{}", f.host, f.host_port);
            any = true;
        }
    }
    if !any && want.is_some() {
        eprintln!(
            "podvm port: {name} publishes no port {}",
            want.unwrap_or_default()
        );
        return EXIT_RUNTIME_ERROR;
    }
    0
}
