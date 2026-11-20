//! TODO/machine.md T-2008: the job of one run for a guest that booted
//! before the run was known.
//!
//! A guest whose state is saved ([`super::state`]) boots the init of the
//! image, then waits for one job on its first serial port. The host sends
//! the job after the load of the state: the rest of the init script of
//! this run (the markers with the nonce of the run, the environment, the
//! working directory, the volumes, the command), the bytes of stdin, the
//! time of the host, and 32 bytes for the random pool of the guest. Each
//! run of one state starts from the same memory: without the last two, two
//! runs have the clock of the save and the same random numbers.
//!
//! The frame is text, so that no byte has a meaning for a terminal:
//!
//! ```text
//! PODVM-JOB-1 LENGTH SHA256\n
//! HEX\n
//! ```
//!
//! `HEX` is `LENGTH` hex digits of one JSON object, and `SHA256` is the
//! digest of those digits. The guest takes a frame only whole: a length
//! out of range, a digit that is no hex digit, a wrong digest or a member
//! of the wrong kind gives the marker [`BAD`] and no command, and the host
//! then boots a guest with no saved state.

use std::io::{Read, Write};

use sha2::{Digest, Sha256};

/// The first word of a frame.
pub const MAGIC: &str = "PODVM-JOB-1";
/// The most hex digits of one frame: 4 MiB.
pub const MAX_HEX: usize = 4 << 20;
/// The most bytes of stdin that a job carries; a run with more boots.
pub const MAX_STDIN: usize = 64 << 10;
/// The newlines before the header of a frame.
const LEAD: usize = 64;
/// The bytes before a frame that the reader skips: the echo of nothing,
/// and a line that the console had before the terminal was raw.
const MAX_NOISE: usize = 64 << 10;
/// The marker of the guest that waits for its job, with the nonce of the
/// state and `up` or `failed` (the boot of the init).
pub const HELD: &str = "PODVM-HELD-";
/// The marker of a job that the guest did not take, with the nonce of the
/// state and the reason.
pub const BAD: &str = "PODVM-JOB-BAD-";

/// One job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// The rest of the init script of the run ([`super::init::job`]).
    pub script: String,
    /// The bytes of the stdin of the command, where podvm read some.
    pub stdin: Option<Vec<u8>>,
    /// The bytes for the random pool of the guest.
    pub seed: [u8; 32],
    /// The time of the host, in nanoseconds since the epoch.
    pub time_ns: u64,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &[u8]) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err(format!("{} hex digits are no whole bytes", text.len()));
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        _ => Err(format!("the byte {c:#04x} is no hex digit")),
    };
    text.chunks(2)
        .map(|p| Ok(digit(p[0])? << 4 | digit(p[1])?))
        .collect()
}

fn sum(digits: &[u8]) -> String {
    hex(&Sha256::digest(digits))
}

impl Job {
    /// The frame of this job. An error where the job is larger than a
    /// frame can be: the caller then boots a guest with no saved state.
    pub fn frame(&self) -> Result<Vec<u8>, String> {
        let body = serde_json::json!({
            "script": self.script,
            "stdin": self.stdin.as_deref().map(hex),
            "seed": hex(&self.seed),
            "time_ns": self.time_ns,
        })
        .to_string();
        let digits = hex(body.as_bytes());
        if digits.len() > MAX_HEX {
            return Err(format!(
                "the job of the run is {} bytes, and a frame holds {}",
                body.len(),
                MAX_HEX / 2
            ));
        }
        // The serial port of an emulator that loads a state takes a few
        // bytes before the load, and the load then drops them (16 at most:
        // the FIFO of the port; measured with QEMU 8.2.2, where a frame
        // with no lead never came). The lead is for those bytes.
        let lead = "\n".repeat(LEAD);
        Ok(format!(
            "{lead}{MAGIC} {} {}\n{digits}\n",
            digits.len(),
            sum(digits.as_bytes())
        )
        .into_bytes())
    }

    /// Read the header line (without its newline): the count of digits and
    /// the digest.
    pub fn header(line: &str) -> Result<(usize, String), String> {
        let f: Vec<&str> = line.split(' ').collect();
        let [magic, len, digest] = f.as_slice() else {
            return Err(format!("the header {line:?} has not three words"));
        };
        if *magic != MAGIC {
            return Err(format!("the header starts with {magic:?}, not {MAGIC}"));
        }
        let len = len
            .parse::<usize>()
            .map_err(|_| format!("the length {len:?} is no number"))?;
        if len == 0 || len > MAX_HEX || !len.is_multiple_of(2) {
            return Err(format!(
                "the length {len} is not an even count from 2 to {MAX_HEX}"
            ));
        }
        if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("the digest is not 64 hex digits".to_string());
        }
        Ok((len, digest.to_string()))
    }

    /// Read the digits of a frame against the digest of its header.
    pub fn body(digits: &[u8], digest: &str) -> Result<Job, String> {
        if sum(digits) != digest {
            return Err("the digest of the digits is not the one of the header".to_string());
        }
        let v: serde_json::Value = serde_json::from_slice(&unhex(digits)?)
            .map_err(|e| format!("the job is no JSON: {e}"))?;
        let script = v["script"]
            .as_str()
            .ok_or("the job has no script")?
            .to_string();
        let stdin = match &v["stdin"] {
            serde_json::Value::Null => None,
            serde_json::Value::String(s) => Some(unhex(s.as_bytes())?),
            _ => return Err("the stdin of the job is no text".to_string()),
        };
        let seed = v["seed"].as_str().ok_or("the job has no seed")?;
        let seed: [u8; 32] = unhex(seed.as_bytes())?
            .try_into()
            .map_err(|_| "the seed of the job is not 32 bytes".to_string())?;
        let time_ns = v["time_ns"].as_u64().ok_or("the job has no time")?;
        Ok(Job {
            script,
            stdin,
            seed,
            time_ns,
        })
    }

    /// Read one frame from `r`. Bytes before the header line are skipped,
    /// up to [`MAX_NOISE`].
    pub fn read(r: &mut impl Read) -> Result<Job, String> {
        let mut line = Vec::new();
        let mut skipped = 0usize;
        let mut b = [0u8; 1];
        let header = loop {
            match r.read(&mut b) {
                Ok(0) => return Err("the serial line ended before a job".to_string()),
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(format!("the read of the serial line: {e}")),
            }
            if b[0] != b'\n' {
                line.push(b[0]);
                if line.len() > 256 {
                    skipped += line.len();
                    line.clear();
                }
            } else {
                let text = String::from_utf8_lossy(&line);
                let text = text.trim_end_matches('\r');
                if let Some(at) = text.find(MAGIC) {
                    break Job::header(&text[at..])?;
                }
                skipped += line.len() + 1;
                line.clear();
            }
            if skipped > MAX_NOISE {
                return Err(format!(
                    "no job in the first {MAX_NOISE} bytes of the serial line"
                ));
            }
        };
        let mut digits = vec![0u8; header.0];
        r.read_exact(&mut digits)
            .map_err(|e| format!("the serial line ended in the job: {e}"))?;
        Job::body(&digits, &header.1)
    }
}

unsafe extern "C" {
    fn clock_settime(clock: i32, ts: *const [i64; 2]) -> i32;
    fn ioctl(fd: i32, req: u64, ...) -> i32;
}

/// `RNDADDENTROPY`: mix bytes into the pool and count them.
const RNDADDENTROPY: u64 = 0x4008_5203;
/// `RNDRESEEDCRNG`: take the pool into the generator now.
const RNDRESEEDCRNG: u64 = 0x5207;

/// Give the kernel of this guest the seed of the run, and make its
/// generator take it now: each run of one saved state has the same pool.
fn reseed(seed: &[u8; 32]) -> Result<(), String> {
    use std::os::fd::AsRawFd;
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/urandom")
        .map_err(|e| format!("/dev/urandom: {e}"))?;
    // struct rand_pool_info: the count of bits, the count of bytes, the bytes.
    #[repr(C)]
    struct Pool {
        bits: i32,
        bytes: i32,
        buf: [u8; 32],
    }
    let pool = Pool {
        bits: 256,
        bytes: 32,
        buf: *seed,
    };
    // SAFETY: each ioctl takes the open descriptor; the first reads `pool`,
    // which lives for the call.
    let done = unsafe {
        ioctl(f.as_raw_fd(), RNDADDENTROPY, &pool as *const Pool) == 0
            && ioctl(f.as_raw_fd(), RNDRESEEDCRNG) == 0
    };
    if done {
        Ok(())
    } else {
        Err(format!(
            "the seed of the random pool: {}",
            std::io::Error::last_os_error()
        ))
    }
}

/// Set the clock of this guest to the time of the host.
fn set_clock(time_ns: u64) -> Result<(), String> {
    let ts = [
        (time_ns / 1_000_000_000) as i64,
        (time_ns % 1_000_000_000) as i64,
    ];
    // SAFETY: CLOCK_REALTIME (0) and a timespec of two 64-bit integers
    // (each 64-bit Linux), which lives for the call.
    if unsafe { clock_settime(0, &ts) } != 0 {
        return Err(format!(
            "the clock of the guest: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// Take the job: the clock, the seed, the stdin file and the script file.
fn apply(job: &Job, out: &std::path::Path) -> Result<(), String> {
    set_clock(job.time_ns)?;
    reseed(&job.seed)?;
    let stdin = std::path::Path::new("/podvm/stdin");
    match &job.stdin {
        Some(bytes) => {
            std::fs::write(stdin, bytes).map_err(|e| format!("{}: {e}", stdin.display()))?
        }
        None => {
            let _ = std::fs::remove_file(stdin);
        }
    }
    std::fs::write(out, &job.script).map_err(|e| format!("{}: {e}", out.display()))
}

/// `podvm guest-job NONCE STATUS FILE`, in a guest: print the marker of a
/// held guest (`STATUS` is the exit code of `guest-init --hold`), read one
/// job from the serial line, and write its script to `FILE`. Exit 0 where
/// the job is taken; 1 with the marker [`BAD`] where it is not.
pub fn guest_main(args: &[String]) -> i32 {
    let [nonce, status, out] = args else {
        eprintln!("podvm guest-job: the arguments are NONCE STATUS FILE");
        return 125;
    };
    // The terminal is raw before the marker: a line of more than 4096 bytes
    // is cut in the line mode of a terminal, and the saved state has the
    // terminal as it is at the marker.
    let raw = super::tty::Raw::new();
    let say = |line: String| {
        let mut o = std::io::stdout().lock();
        // A marker that does not reach the console has no second place:
        // the host then sees no marker, and its bound ends the guest.
        o.write_all(line.as_bytes())
            .and_then(|()| o.flush())
            .unwrap_or(());
    };
    let boot = if status == "0" { "up" } else { "failed" };
    say(format!("\n{HELD}{nonce} {boot}\n"));
    let got = Job::read(&mut std::io::stdin().lock())
        .and_then(|job| apply(&job, std::path::Path::new(out)));
    drop(raw);
    match got {
        Ok(()) => 0,
        Err(why) => {
            say(format!("\n{BAD}{nonce} {}\n", why.replace('\n', " ")));
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> Job {
        Job {
            script: "echo PODVM-READY-x\nset -- 'a b' '\"$@\"'\n".to_string(),
            stdin: Some(vec![0, 255, b'\n', b'\r', 3]),
            seed: [7; 32],
            time_ns: 1_795_000_000_123_456_789,
        }
    }

    /// A frame comes back as it went, also after noise on the line, and
    /// with no stdin.
    #[test]
    fn a_job_comes_back_as_it_went() {
        let mut none = job();
        none.stdin = None;
        for j in [job(), none] {
            let frame = j.frame().unwrap();
            assert!(frame.is_ascii(), "a frame is text");
            assert_eq!(Job::read(&mut &frame[..]), Ok(j.clone()));
            let mut noisy = b"[  1.0] a line of the kernel\r\nhalf a line PODVM".to_vec();
            noisy.extend(b"\n");
            noisy.extend(&frame);
            assert_eq!(Job::read(&mut &noisy[..]), Ok(j));
        }
    }

    /// Each frame that is not whole is refused with its reason: the guest
    /// runs no command from it.
    #[test]
    fn a_frame_that_is_not_whole_is_refused() {
        let good = job().frame().unwrap();
        let text = String::from_utf8(good.clone()).unwrap();
        let text = text.trim_start_matches('\n');
        assert_eq!(good.len() - text.len(), LEAD, "the lead of a frame");
        let (head, digits) = text.split_once('\n').unwrap();
        let digits = digits.trim_end();
        let words: Vec<&str> = head.split(' ').collect();
        let with = |len: &str, sum: &str, body: &str| format!("{MAGIC} {len} {sum}\n{body}\n");
        let other = hex(b"{\"script\":1}");
        let cases: Vec<(String, &str)> = vec![
            (String::new(), "ended before a job"),
            ("no frame here\n".repeat(8000), "no job in the first"),
            (text[..text.len() / 2].to_string(), "ended in the job"),
            (with("x", words[2], digits), "is no number"),
            (with("0", words[2], digits), "not an even count"),
            (with("7", words[2], digits), "not an even count"),
            (
                with(&(MAX_HEX + 2).to_string(), words[2], digits),
                "not an even count",
            ),
            (with(words[1], "abc", digits), "not 64 hex digits"),
            (
                with(words[1], &"0".repeat(64), digits),
                "is not the one of the header",
            ),
            (format!("{MAGIC} {}\n", words[1]), "has not three words"),
            (
                with(words[1], words[2], &digits.replacen(&digits[..2], "zz", 1)),
                "is not the one of the header",
            ),
            (
                with(&other.len().to_string(), &sum(other.as_bytes()), &other),
                "has no script",
            ),
            (with("4", &sum(b"zzzz"), "zzzz"), "is no hex digit"),
            (with("4", &sum(b"ABCD"), "ABCD"), "is no hex digit"),
        ];
        for (frame, want) in cases {
            let e = Job::read(&mut frame.as_bytes()).unwrap_err();
            assert!(e.contains(want), "{want:?} is not in {e:?}");
        }
        // A member of the wrong kind, with a right digest.
        for (member, value, want) in [
            ("stdin", serde_json::json!(5), "is no text"),
            ("stdin", serde_json::json!("0g"), "is no hex digit"),
            ("seed", serde_json::json!("00"), "not 32 bytes"),
            ("seed", serde_json::Value::Null, "has no seed"),
            ("time_ns", serde_json::json!(-1), "has no time"),
            ("time_ns", serde_json::json!("now"), "has no time"),
        ] {
            let mut v = serde_json::json!({
                "script": "true", "stdin": null, "seed": hex(&[1; 32]), "time_ns": 1,
            });
            v[member] = value;
            let d = hex(v.to_string().as_bytes());
            let e = Job::body(d.as_bytes(), &sum(d.as_bytes())).unwrap_err();
            assert!(e.contains(want), "{member}: {want:?} is not in {e:?}");
        }
    }

    /// A job larger than a frame is an error of the host, before a guest
    /// starts.
    #[test]
    fn a_job_larger_than_a_frame_is_refused_by_the_host() {
        let mut j = job();
        j.script = "x".repeat(MAX_HEX / 2 + 1);
        assert!(j.frame().unwrap_err().contains("a frame holds"));
    }
}
