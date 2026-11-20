//! TODO/cli.md T-2046: the stream of a log against the whole read.

use std::io::{Cursor, Write};

use super::*;

/// The texts of the tail: no line end, one at the end, empty lines, a
/// line longer than a piece, and lines that cross the edge of a piece.
const TEXTS: &[&[u8]] = &[
    b"",
    b"\n",
    b"\n\n\n",
    b"a",
    b"a\n",
    b"a\nb",
    b"a\nb\n",
    b"one\ntwo\nthree\n",
    b"one\n\ntwo\n\n",
    b"a-long-line-past-the-piece\nb\nc-also-long-past-it\n",
    b"x\ny\nz",
];

/// The rule of each count and each size of piece is the rule of the whole
/// read (`pod_supervise::tail_offset`).
#[test]
fn the_tail_from_the_end_is_the_tail_of_the_whole_read() {
    for text in TEXTS {
        for n in 0..6u64 {
            let want = pod_supervise::tail_offset(text, n);
            for chunk in [1usize, 2, 3, 5, 64, CHUNK] {
                let mut f = Cursor::new(text.to_vec());
                let got = tail_start_by(&mut f, text.len() as u64, n, chunk).unwrap();
                assert_eq!(
                    got,
                    want,
                    "{:?} n={n} chunk={chunk}",
                    String::from_utf8_lossy(text)
                );
            }
        }
    }
}

/// A file shorter than the length of the open gives its start, never an
/// error and never a panic.
#[test]
fn a_file_that_became_shorter_gives_the_start() {
    let mut f = Cursor::new(b"a\nb\n".to_vec());
    assert_eq!(tail_start_by(&mut f, 100, 1, 3).unwrap(), 0);
}

/// The copy: each start, end and bound gives the same bytes as a slice of
/// the whole read.
#[test]
fn the_copy_is_the_slice_of_the_whole_read() {
    let text: Vec<u8> = (0..(CHUNK * 2 + 17)).map(|i| (i % 251) as u8).collect();
    let len = text.len() as u64;
    let cases: &[(u64, u64, Option<u64>)] = &[
        (0, len, None),
        (0, len, Some(1)),
        (0, len, Some(CHUNK as u64)),
        (0, len, Some(CHUNK as u64 + 1)),
        (0, len, Some(len + 5)),
        (5, len, Some(10)),
        (len, len, None),
        (0, len + 100, None), // the file ends first
        (len + 3, len + 9, None),
    ];
    for &(start, end, limit) in cases {
        let mut out = Vec::new();
        let n = copy_range(&mut Cursor::new(text.clone()), start, end, limit, &mut out).unwrap();
        let lo = start.min(len) as usize;
        let mut hi = end.min(len) as usize;
        if let Some(l) = limit {
            hi = hi.min(lo + l as usize);
        }
        assert_eq!(out, text[lo..hi.max(lo)], "{start} {end} {limit:?}");
        assert_eq!(n, out.len() as u64);
    }
}

/// A writer that refuses each write, as a closed pipe does.
struct Closed;

impl Write for Closed {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::BrokenPipe.into())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A closed output is a fault of the write, so the caller names it as one.
#[test]
fn a_closed_output_is_a_fault_of_the_write() {
    let r = copy_range(&mut Cursor::new(b"abc".to_vec()), 0, 3, None, &mut Closed);
    assert!(matches!(r, Err(Fault::Write(e)) if e.kind() == std::io::ErrorKind::BrokenPipe));
}

/// The bound: the write that reaches it passes its bytes and stops the
/// writer; a write before it passes all.
#[test]
fn the_cap_passes_its_bytes_and_then_refuses() {
    for (bound, writes, want) in [
        (5u64, &[&b"ab"[..], b"cd", b"efgh"][..], &b"abcde"[..]),
        (2, &[b"ab", b"cd"], b"ab"),
        (10, &[b"ab", b"cd"], b"abcd"),
    ] {
        let mut sink = Vec::new();
        let mut cap = Capped::new(&mut sink, bound);
        let mut stopped = false;
        for w in writes {
            if cap.write_all(w).is_err() {
                stopped = true;
                break;
            }
        }
        assert_eq!(stopped, cap.full(), "{bound}");
        assert_eq!(sink, want, "{bound}");
    }
}
