//! The tests of the boot of the init of the image (T-1993, T-2083).

use super::{bound_of, step, units_line, Answer, Next, Step, Waiter, BOOT_BOUND};
use std::ffi::OsStr;
use std::time::{Duration, Instant};

#[test]
fn the_failed_units_give_one_line() {
    for (text, want) in [
        ("", ""),
        ("\n", ""),
        (
            "chrony.service loaded failed failed chrony\nsystemd-remount-fs.service loaded failed failed Remount\n",
            "; failed units: chrony.service, systemd-remount-fs.service",
        ),
    ] {
        assert_eq!(units_line(text), want, "{text:?}");
    }
}

#[test]
fn the_state_of_systemd_gives_the_step() {
    let bus = "Failed to connect to bus: No such file or directory";
    for (state, err, want) in [
        ("running", "", Step::Run),
        ("degraded", "", Step::Run),
        ("starting", "", Step::Again),
        ("initializing", "", Step::Again),
        // PodUtils/podrunner#349: offline is not final.
        ("offline", "", Step::Again),
        ("unknown", "", Step::Again),
        ("", bus, Step::Again),
        (
            "",
            "Failed to connect to system scope bus via local transport: No such file or directory",
            Step::Again,
        ),
        ("", "", Step::Fail),
        ("maintenance", "", Step::Fail),
        ("stopping", "", Step::Fail),
        ("\u{fffd}garbage", "", Step::Fail),
    ] {
        assert_eq!(step(state, err), want, "{state:?} {err:?}");
    }
}

fn out(state: &str) -> Answer {
    Answer::Out {
        state: state.to_string(),
        err: String::new(),
        exit: i32::from(state != "running"),
    }
}

fn refused(running: bool) -> Answer {
    Answer::ExecErr {
        err: "instance is not running".to_string(),
        running,
    }
}

/// PodUtils/podrunner#349 asks 1 to 3: each answer before the bound
/// and at the bound gives one decision.
#[test]
fn each_answer_gives_its_decision_before_and_at_the_bound() {
    #[derive(Debug, PartialEq)]
    enum Want {
        Up(bool),
        Again,
        Fail,
        Bound,
    }
    let t0 = Instant::now();
    let bound = Duration::from_secs(60);
    for (answer, before, at) in [
        (out("running"), Want::Up(false), Want::Up(false)),
        (out("degraded"), Want::Up(true), Want::Up(true)),
        (out("offline"), Want::Again, Want::Bound),
        (out("initializing"), Want::Again, Want::Bound),
        (out("starting"), Want::Again, Want::Bound),
        (out("unknown"), Want::Again, Want::Bound),
        (out("maintenance"), Want::Fail, Want::Fail),
        (out("stopping"), Want::Fail, Want::Fail),
        (out("garbage"), Want::Fail, Want::Fail),
        (refused(true), Want::Again, Want::Bound),
        (refused(false), Want::Fail, Want::Fail),
        (Answer::NoReply, Want::Again, Want::Bound),
    ] {
        let text = answer.text();
        for (now, want) in [(t0, before), (t0 + bound, at)] {
            let a = match &answer {
                Answer::Out { state, err, exit } => Answer::Out {
                    state: state.clone(),
                    err: err.clone(),
                    exit: *exit,
                },
                Answer::ExecErr { err, running } => Answer::ExecErr {
                    err: err.clone(),
                    running: *running,
                },
                Answer::NoReply => Answer::NoReply,
            };
            let got = match Waiter::new(t0, bound).next(a, now) {
                Next::Up { degraded } => Want::Up(degraded),
                Next::Again => Want::Again,
                Next::Fail(line) => {
                    assert!(line.contains(&text), "{line}");
                    Want::Fail
                }
                Next::Bound(line) => {
                    assert!(line.contains("in 60 s") && line.contains(&text), "{line}");
                    Want::Bound
                }
            };
            assert_eq!(got, want, "{text} at {:?}", now - t0);
        }
    }
}

/// The line of the bound names each poll in order, a run of equal
/// answers once with its count, and at most twelve runs.
#[test]
fn the_bound_names_each_poll() {
    let t0 = Instant::now();
    let mut w = Waiter::new(t0, Duration::from_secs(5));
    for _ in 0..3 {
        assert_eq!(w.next(out("offline"), t0), Next::Again);
    }
    assert_eq!(w.next(refused(true), t0), Next::Again);
    let Next::Bound(line) = w.next(out("starting"), t0 + Duration::from_secs(5)) else {
        panic!("no bound");
    };
    assert!(
        line.ends_with(
            "the polls: \"offline\" (exit 1) x3, exec refused (instance is not running), \
             \"starting\" (exit 1)"
        ),
        "{line}"
    );
    let mut w = Waiter::new(t0, Duration::from_secs(5));
    for i in 0..40 {
        let _ = w.next(out(if i % 2 == 0 { "offline" } else { "starting" }), t0);
    }
    assert!(w.polls.line().contains("(28 more)"), "{}", w.polls.line());
}

#[test]
fn the_variable_sets_the_bound() {
    assert_eq!(bound_of(None), Ok(BOOT_BOUND));
    for (v, want) in [
        ("60", Some(60)),
        (" 1 ", Some(1)),
        ("3600", Some(3600)),
        ("0", None),
        ("3601", None),
        ("-5", None),
        ("1.5", None),
        ("", None),
        ("x", None),
    ] {
        let got = bound_of(Some(OsStr::new(v)));
        match want {
            Some(n) => assert_eq!(got, Ok(Duration::from_secs(n)), "{v:?}"),
            None => assert!(
                got.unwrap_err().contains("PODVM_INIT_BOOT_TIMEOUT"),
                "{v:?}"
            ),
        }
    }
}
