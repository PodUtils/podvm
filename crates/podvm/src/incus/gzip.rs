//! The upload of an image to incusd as gzip, TODO/machine.md T-2085
//! (PodUtils/podrunner#347 ask 1).
//!
//! incusd keeps the upload as a file in its `images` directory until the
//! image is removed, and it unpacks that file into the pool at the first
//! create. A plain unified tarball is a second full copy of the tree on
//! the disk; as gzip it is about a third of it (measured in T-2085). Where
//! incusd's images have room ([`wanted`]), the upload stays plain.
//!
//! [`Members`] cuts the stream into blocks of [`BLOCK`] bytes and writes
//! each block as one gzip member: the threads compress the blocks of one
//! batch at the same time, and the members go out in their order. The
//! concatenation of gzip members is one gzip file (RFC 1952, section 2.2),
//! which `tar -z` of incusd reads. The bytes do not depend on the count of
//! threads: the header of each member has no name and no time, and each
//! block is cut at the same offset. So the SHA-256 of the stream (the
//! fingerprint of incusd, T-2081) is the same for the same tarball.
//!
//! The writer has no podvm-specific part: the request to podcrates to take
//! it is in T-2085.

use std::io::{self, Write};

use flate2::write::GzEncoder;
use flate2::Compression;

/// The bytes of one gzip member. Each member starts with no dictionary: a
/// block of 2 MiB loses less than 0.1 % of the ratio of one stream.
pub const BLOCK: usize = 2 << 20;

/// The most threads of one writer.
pub const MAX_THREADS: usize = 8;

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
    Gzip(Members<W>),
}

impl<W: Write> Upload<W> {
    /// The upload into `inner`, gzip where `gzip`.
    pub fn new(inner: W, gzip: bool) -> Upload<W> {
        if gzip {
            Upload::Gzip(Members::new(inner, host_threads()))
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

/// The threads of this host: its parallelism, at most [`MAX_THREADS`], 1
/// where the kernel does not say.
pub fn host_threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(MAX_THREADS))
}

/// A writer of gzip members into `inner`.
pub struct Members<W: Write> {
    inner: W,
    threads: usize,
    level: Compression,
    full: Vec<Vec<u8>>,
    cur: Vec<u8>,
    wrote: bool,
}

impl<W: Write> Members<W> {
    /// A writer with `threads` threads (from 1 to [`MAX_THREADS`]) at the
    /// fast level of gzip.
    pub fn new(inner: W, threads: usize) -> Members<W> {
        Members {
            inner,
            threads: threads.clamp(1, MAX_THREADS),
            level: Compression::fast(),
            full: Vec::new(),
            cur: Vec::with_capacity(BLOCK),
            wrote: false,
        }
    }

    /// Compress the full blocks and write their members in order.
    fn drain(&mut self) -> io::Result<()> {
        let blocks = std::mem::take(&mut self.full);
        if blocks.is_empty() {
            return Ok(());
        }
        let level = self.level;
        let members: Vec<io::Result<Vec<u8>>> = std::thread::scope(|s| {
            let handles: Vec<_> = blocks
                .iter()
                .map(|b| s.spawn(move || member(b, level)))
                .collect();
            handles
                .into_iter()
                .map(|h| {
                    h.join().unwrap_or_else(|_| {
                        Err(io::Error::other(
                            "a thread of the gzip writer ended with a panic",
                        ))
                    })
                })
                .collect()
        });
        for m in members {
            self.inner.write_all(&m?)?;
            self.wrote = true;
        }
        Ok(())
    }

    /// Write the last block and flush: the inner writer. A stream with no
    /// byte is one empty member, which is a valid gzip file.
    pub fn finish(mut self) -> io::Result<W> {
        if !self.cur.is_empty() || (!self.wrote && self.full.is_empty()) {
            let last = std::mem::take(&mut self.cur);
            self.full.push(last);
        }
        self.drain()?;
        self.inner.flush()?;
        Ok(self.inner)
    }
}

/// One gzip member of `block`.
fn member(block: &[u8], level: Compression) -> io::Result<Vec<u8>> {
    let mut e = GzEncoder::new(Vec::with_capacity(block.len() / 2 + 64), level);
    e.write_all(block)?;
    e.finish()
}

impl<W: Write> Write for Members<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = buf.len().min(BLOCK - self.cur.len());
        self.cur.extend_from_slice(&buf[..n]);
        if self.cur.len() == BLOCK {
            let block = std::mem::replace(&mut self.cur, Vec::with_capacity(BLOCK));
            self.full.push(block);
            if self.full.len() >= self.threads {
                self.drain()?;
            }
        }
        Ok(n)
    }

    /// A flush writes nothing: a member ends only at its block's end, so
    /// that the bytes do not depend on where a caller flushes.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
