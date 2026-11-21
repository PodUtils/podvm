use super::*;

fn v(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}

#[test]
fn only_a_foreground_run_of_a_caller_that_asks_enters() {
    let run = v(&["podvm", "run", "--rm", "img", "true"]);
    assert!(wanted(&run, 1000, Some("1")));
    assert!(!wanted(&run, 0, Some("1")), "uid 0 has the rights");
    assert!(wanted(&run, 1000, None), "the default");
    assert!(!wanted(&run, 1000, Some("0")), "turned off");
    for d in ["-d", "--detach", "-itd", "--podvm-tier=machine"] {
        let a = v(&["podvm", "run", d, "img", "true"]);
        assert!(!wanted(&a, 1000, Some("1")), "{d}");
    }
    assert!(wanted(&v(&["podvm", "run", "-it", "img"]), 1000, Some("1")));
    assert!(!wanted(
        &v(&["podvm", "exec", "c", "true"]),
        1000,
        Some("1")
    ));
    let m = maps(1000, 100);
    assert_eq!(m[1].1, "0 1000 1");
    assert_eq!(m[2].1, "0 100 1");
}
