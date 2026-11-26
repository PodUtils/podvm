//! The job of the spare-tree helper on its pipe (TODO/enter.md T-2054
//! item 5): one table of jobs and the rule of each.

use super::{job_bytes, parse_job, MAX_JOB};
use std::path::{Path, PathBuf};

/// The store and digest of a job, or the refusal.
type Want = Result<(&'static str, Option<&'static str>), &'static str>;

#[test]
fn a_job_goes_on_the_pipe_and_back_or_is_refused_by_name() {
    let long = vec![b'a'; MAX_JOB];
    let mut too_long = b"/s\0".to_vec();
    too_long.extend_from_slice(&long);
    // (the bytes on the pipe, the store and digest, or the refusal)
    let cases: [(&[u8], Want); 9] = [
        (b"/s\0sha256:ab", Ok(("/s", Some("sha256:ab")))),
        (b"/s\0", Ok(("/s", None))),
        (b"/s", Err("the job has no NUL after the store")),
        (b"", Err("the job has no NUL after the store")),
        (b"s\0d", Err("the store is not an absolute path")),
        (b"\0d", Err("the store is not an absolute path")),
        (b"/s\0d\0", Err("the digest holds a NUL")),
        (b"/s\0\xff", Err("the digest is not UTF-8")),
        (&too_long, Err("the job is too long")),
    ];
    for (job, want) in cases {
        let got = parse_job(job);
        let want = want.map(|(r, d)| (PathBuf::from(r), d.map(str::to_string)));
        assert_eq!(got, want, "job {:?}", String::from_utf8_lossy(job));
    }
}

#[test]
fn a_store_path_that_is_not_utf8_goes_on_the_pipe_and_back() {
    use std::os::unix::ffi::OsStrExt;
    let root = Path::new(std::ffi::OsStr::from_bytes(b"/var/tmp/st\xffore"));
    for digest in ["", "sha256:00"] {
        let job = job_bytes(root, digest).expect("a job");
        let (r, d) = parse_job(&job).expect("the job parses");
        assert_eq!(r, root);
        assert_eq!(d.as_deref(), (!digest.is_empty()).then_some(digest));
    }
    assert_eq!(job_bytes(Path::new(""), "d"), None, "an empty store");
    assert_eq!(
        job_bytes(Path::new("/s"), "d\0"),
        None,
        "a NUL in the digest"
    );
    let huge = "/".repeat(MAX_JOB);
    assert_eq!(job_bytes(Path::new(&huge), "d"), None, "over MAX_JOB");
}
