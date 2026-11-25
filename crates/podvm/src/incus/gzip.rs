//! The upload of an image to incusd as gzip, TODO/machine.md T-2085
//! (PodUtils/podrunner#347 ask 1).
//!
//! incusd keeps the upload as a file in its `images` directory until the
//! image is removed, and it unpacks that file into the pool at the first
//! create. A plain unified tarball is a second full copy of the tree on
//! the disk; as gzip it is about a third of it (measured in T-2085). Where
//! incusd's images have room ([`wanted`]), the upload stays plain.
//!
//! The shared writer keeps the member offsets and bytes independent of
//! the thread count, so the fingerprint stays the same for one tarball.

use std::io::{self, Write};

/// TODO/machine.md T-2085: the upload is plain where incusd's images have
/// at least this many times the bytes of the tree free (the upload, the
/// root of the instance, and as much again to spare): plain costs no CPU,
/// and gzip of a tree of 57 GB takes minutes. Else it is gzip, also where
/// podvm cannot read the free space.
pub const ROOM_TIMES: u64 = 3;

/// Whether the upload of a tree of `tree` bytes is gzip, with `free`
/// bytes free in incusd's images (`None`: podvm cannot read them).
pub fn wanted(free: Option<u64>, tree: u64) -> bool {
    free.is_none_or(|f| f / ROOM_TIMES < tree)
}

/// The upload: the plain tarball or its gzip.
pub enum Upload<W: Write> {
    Plain(W),
    Gzip(pod_gzip::Members<W>),
}

impl<W: Write> Upload<W> {
    /// The upload into `inner`, gzip where `gzip`.
    pub fn new(inner: W, gzip: bool) -> Upload<W> {
        if gzip {
            Upload::Gzip(pod_gzip::Members::new(inner, pod_gzip::host_threads()))
        } else {
            Upload::Plain(inner)
        }
    }

    /// The end of the stream: the last member of gzip, and a flush.
    pub fn finish(self) -> io::Result<W> {
        match self {
            Upload::Plain(mut w) => w.flush().map(|()| w),
            Upload::Gzip(g) => g.finish(),
        }
    }
}

impl<W: Write> Write for Upload<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Upload::Plain(w) => w.write(buf),
            Upload::Gzip(g) => g.write(buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Upload::Plain(w) => w.flush(),
            Upload::Gzip(g) => g.flush(),
        }
    }
}
