//! Does this mirror speak HTTPS? `pod-rootfs` asks it once per host
//! (`pod_rootfs::reach::Probe`), and this module answers it over podvm's
//! transport ([`podvm_image::net`]: `pod-proxy`, the proxy of the
//! environment, podvm's roots).
//!
//! ⛔ Bounded, always. [`TODO/RULES.md`](../../../TODO/RULES.md) section 8: a
//! runtime whose audience is automated may not wait unbounded, and this probe
//! runs on the path to starting a container.
//!
//! ⚠ **Any HTTP response is a yes**, 404 included. The question is whether the
//! host terminates TLS on 443, not whether it serves `/`. A connect failure, a
//! TLS failure or a timeout is a no, and the reason travels with it so the
//! banner can say which.

use std::time::Duration;

use podvm_image::net::{Limits, Net, Refusal};

pub use pod_rootfs::reach::Answer;

/// ⚠ Two seconds to connect and two to read: the alternative -- a
/// black-holed tcp/80 inside the payload -- costs the package manager's own
/// timeout, which is minutes.
const CONNECT: Duration = Duration::from_secs(2);
const READ: Duration = Duration::from_secs(2);

/// One bounded `HEAD https://HOST/`.
pub fn probe(host: &str) -> Answer {
    let (config, _roots) = podvm_image::tls::client_config();
    // ⚠ The environment's proxy where it sets one: an intercepting proxy is
    // the normal case in the environments podvm is built for.
    match Net::from_environment(config) {
        Ok(mut net) => answer(&mut net, &format!("https://{host}/")),
        Err(why) => Answer::No(why),
    }
}

/// The answer of the first reply of `url`.
///
/// ⛔ A redirect is not followed (TODO/enter.md T-1953, PodUtils/podvm#560):
/// the question is about this host. A 301 to another host is this host's
/// answer over HTTPS, and a follow would turn a timeout of that other host
/// into a refusal that names a host that no source line names.
fn answer(net: &mut Net, url: &str) -> Answer {
    let limits = Limits {
        head: CONNECT.saturating_add(READ),
        idle: READ,
    };
    match net.first_status("HEAD", url, true, limits) {
        // ⭐ Any status is an answer: the host terminated TLS and replied.
        Ok(_) => Answer::Https,
        Err(e @ (Refusal::Transport(_) | Refusal::Refused(_))) => Answer::No(e.to_string()),
        Err(Refusal::Status(_)) => Answer::Https,
    }
}

/// TODO/image.md T-1762: how long a verdict holds. A yes holds a day; a
/// no holds ten minutes, so a mirror that comes back is asked again soon.
const FRESH_YES: u64 = 24 * 3600;
const FRESH_NO: u64 = 600;

/// One verdict of the cache: `HOST TAB https|no TAB SECONDS TAB REASON`.
fn parse_line(line: &str) -> Option<(&str, Answer, u64)> {
    let mut f = line.splitn(4, '\t');
    let host = f.next()?;
    let word = f.next()?;
    let at: u64 = f.next()?.parse().ok()?;
    let answer = match word {
        "https" => Answer::Https,
        "no" => Answer::No(format!("{} (cached)", f.next().unwrap_or(""))),
        _ => return None,
    };
    Some((host, answer, at))
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The cached verdict of `host` in `text`, where it is fresh at `now`.
pub fn lookup(text: &str, host: &str, now: u64) -> Option<Answer> {
    text.lines().filter_map(parse_line).find_map(|(h, a, at)| {
        let fresh = if a.ok() { FRESH_YES } else { FRESH_NO };
        (h == host && now.saturating_sub(at) < fresh).then_some(a)
    })
}

/// `text` with the verdict of `host` at `now` in place of its old line.
pub fn record(text: &str, host: &str, answer: &Answer, now: u64) -> String {
    let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
    let mut out: String = text
        .lines()
        .filter(|l| parse_line(l).is_some_and(|(h, _, _)| h != host))
        .map(|l| format!("{l}\n"))
        .collect();
    let (word, why) = match answer {
        Answer::Https => ("https", String::new()),
        Answer::No(why) => ("no", clean(why)),
    };
    out.push_str(&format!("{}\t{word}\t{now}\t{why}\n", clean(host)));
    out
}

/// [`probe`] behind a cache file of the store: a fresh verdict of an
/// earlier run answers with no request (TODO/image.md T-1762). Before,
/// each `run` of ubuntu paid about 2 s for four probes. With no path, or a
/// file that does not read or write, each host is probed as before.
pub fn cached(path: Option<std::path::PathBuf>) -> impl FnMut(&str) -> Answer {
    move |host| {
        let Some(path) = &path else {
            return probe(host);
        };
        let text = std::fs::read_to_string(path).unwrap_or_default();
        if let Some(a) = lookup(&text, host, now()) {
            return a;
        }
        let a = probe(host);
        let next = record(&text, host, &a, now());
        // The one atomic replace of a file (TODO/deps.md T-2066): a reader
        // never sees half a file, and a failed write leaves no file beside
        // it. A cache that is not written costs a probe at the next run.
        let _ =
            pod_core::files::replace(path, next.as_bytes(), pod_core::files::Mode::Exact(0o644));
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn net() -> Net {
        let (config, _) = podvm_image::tls::client_config();
        Net::with_environment(config, podvm_image::net::environment(&|_| None))
            .expect("the transport starts")
    }

    /// A loopback origin that answers its first connection with `reply`.
    fn origin(reply: String) -> (u16, TcpListener) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback binds");
        let port = listener.local_addr().unwrap().port();
        let l = listener.try_clone().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = l.accept() {
                let mut head = [0u8; 1024];
                let _ = s.read(&mut head);
                let _ = s.write_all(reply.as_bytes());
            }
        });
        (port, listener)
    }

    /// TODO/enter.md T-1953 (PodUtils/podvm#560): a reach check decides by
    /// the first answer of the host that it asks. A redirect to another
    /// host is a yes, and the other host is not asked: it can time out, or
    /// be down, with no effect on the verdict and no name in the error.
    #[test]
    fn a_redirect_is_the_answer_of_the_first_host_and_is_not_followed() {
        let (far, far_listener) = origin("HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".into());
        far_listener.set_nonblocking(true).unwrap();
        for status in [
            "301 Moved Permanently",
            "302 Found",
            "308 Permanent Redirect",
        ] {
            let reply = format!(
                "HTTP/1.1 {status}\r\nLocation: http://localhost:{far}/x\r\nContent-Length: 0\r\n\r\n"
            );
            let (near, _keep) = origin(reply);
            let got = answer(&mut net(), &format!("http://127.0.0.1:{near}/"));
            assert_eq!(got, Answer::Https, "{status}");
        }
        // The redirect target was never dialled.
        let dialled = far_listener.accept().map(|_| true).unwrap_or(false);
        assert!(!dialled, "the host of the redirect was asked");
        // A status of 400 or more is an answer too; a host that answers
        // nothing is a no whose reason names the host that was asked.
        let (near, _keep) = origin("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".into());
        assert_eq!(
            answer(&mut net(), &format!("http://127.0.0.1:{near}/")),
            Answer::Https
        );
        let closed = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = closed.local_addr().unwrap().port();
        drop(closed);
        match answer(&mut net(), &format!("http://127.0.0.1:{port}/")) {
            Answer::No(why) => assert!(why.contains(&format!("127.0.0.1:{port}")), "{why}"),
            Answer::Https => panic!("a closed port answered"),
        }
    }

    #[test]
    fn a_verdict_is_cached_with_its_age_bound() {
        let t = record("", "a.example", &Answer::Https, 1000);
        let t = record(
            &t,
            "b.example",
            &Answer::No("connect\ttimed out".into()),
            1000,
        );
        assert_eq!(
            lookup(&t, "a.example", 1000 + FRESH_YES - 1),
            Some(Answer::Https)
        );
        assert_eq!(lookup(&t, "a.example", 1000 + FRESH_YES), None);
        let no = lookup(&t, "b.example", 1001).expect("a fresh no");
        assert_eq!(no.why(), "connect timed out (cached)");
        assert_eq!(lookup(&t, "b.example", 1000 + FRESH_NO), None);
        // A new verdict replaces the old line of its host.
        let t = record(&t, "b.example", &Answer::Https, 2000);
        assert_eq!(t.lines().count(), 2);
        assert_eq!(lookup(&t, "b.example", 2001), Some(Answer::Https));
        assert_eq!(lookup("garbage\nx\ty\n", "x", 0), None);
    }

    /// ⛔ The three states stay three. A `No` carries its reason so the banner
    /// can say why a file was left alone.
    #[test]
    fn a_refusal_carries_its_reason() {
        let a = Answer::No("connect timed out".into());
        assert!(!a.ok());
        assert_eq!(a.why(), "connect timed out");
    }
}
