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

/// `podvm logs [-f] [--tail N] [--limit-bytes N] NAME` of a machine: the
/// serial line of the guest; with `-f`, the new bytes while the machine
/// runs. One buffer of 64 KiB, as the log of a container (TODO/cli.md
/// T-2046).
pub(crate) fn logs(name: &str, o: &crate::lifecycle::LogsArgs) -> i32 {
    use podvm_supervise::stream::{self, Fault};
    let root = machines::root();
    let path = machines::dir_in(&root, name).join("serial.log");
    let mut out = std::io::stdout().lock();
    // A serial log that is not there yet, or that does not open, is an
    // empty log: the machine writes it when the guest starts.
    let open = || match stream::open(&path) {
        Ok(Some(f)) => {
            let len = f.metadata().map_or(0, |m| m.len());
            Some((f, len))
        }
        _ => None,
    };
    let (mut at, mut written) = (0u64, 0u64);
    if let Some((mut f, len)) = open() {
        match crate::lifecycle::copy_log(&mut f, len, o, &mut out) {
            Ok(n) => (at, written) = (len, n),
            Err(c) => return c,
        }
    }
    loop {
        if o.limit.is_some_and(|l| written >= l)
            || !o.follow
            || !machines::read(&root, name).is_some_and(|r| r.running())
        {
            return 0;
        }
        std::thread::sleep(Duration::from_millis(250));
        let Some((mut f, len)) = open() else {
            continue;
        };
        let left = o.limit.map(|l| l - written);
        match stream::copy_range(&mut f, at, len, left, &mut out) {
            Ok(n) => (at, written) = (at + n, written + n),
            Err(Fault::Write(e)) => return crate::names::stdout::failed(&e),
            Err(Fault::Read(e)) => {
                eprintln!("podvm logs: {name}: reading {}: {e}", path.display());
                return EXIT_RUNTIME_ERROR;
            }
        }
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
