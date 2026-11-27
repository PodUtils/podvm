//! A qcow2 file of a raw disk, with no tool of the host (TODO/machine.md
//! T-1998). incusd takes the disk of a VM image as qcow2 only: it reads
//! the image with `qemu-img info -f qcow2` and refuses each other format.
//!
//! The file is qcow2 version 3 with 64 KiB clusters and 16-bit reference
//! counts, in this order: the header, the L1 table, the reference count
//! table, the reference count blocks, the L2 tables and the data clusters.
//! A cluster of the raw disk that has only zeros is not in the file: its
//! L2 entry stays 0, and a reader gets zeros. Each cluster of the file has
//! the reference count 1, and each L1 and L2 entry has the flag COPIED.
//! The holes of a sparse raw disk are passed over with `SEEK_DATA` and
//! `SEEK_HOLE`, where the file system has them.

use std::fs::File;
use std::os::unix::fs::FileExt;

/// The bits of a cluster: 64 KiB.
pub const CLUSTER_BITS: u32 = 16;
pub const CLUSTER: u64 = 1 << CLUSTER_BITS;
/// The entries of one L2 table.
pub const L2_ENTRIES: u64 = CLUSTER / 8;
/// The 16-bit reference counts of one block.
pub const REFS_PER_BLOCK: u64 = CLUSTER / 2;
/// The flag of an entry whose cluster has the reference count 1.
pub const COPIED: u64 = 1 << 63;
const MAGIC: u32 = 0x5146_49fb;
const HEADER_LEN: u32 = 104;

/// The places of the parts of one file, in clusters of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// The size of the disk in bytes.
    pub size: u64,
    pub l1_entries: u64,
    pub l1_clusters: u64,
    pub rt_clusters: u64,
    pub rb_clusters: u64,
    /// The L1 index of each L2 table, in order.
    pub l2: Vec<u64>,
    /// The cluster of the disk of each data cluster, in order.
    pub data: Vec<u64>,
}

impl Layout {
    /// The layout of a disk of `size` bytes whose clusters `data` (sorted,
    /// each once) hold bytes that are not zero.
    pub fn of(size: u64, data: Vec<u64>) -> Layout {
        let clusters = size.div_ceil(CLUSTER);
        let l1_entries = clusters.div_ceil(L2_ENTRIES).max(1);
        let l1_clusters = (l1_entries * 8).div_ceil(CLUSTER);
        let mut l2: Vec<u64> = data.iter().map(|c| c / L2_ENTRIES).collect();
        l2.dedup();
        let base = 1 + l1_clusters + l2.len() as u64 + data.len() as u64;
        // The blocks count themselves and the table: a fixed point.
        let (mut rt, mut rb) = (1u64, 1u64);
        loop {
            let total = base + rt + rb;
            let need_rb = total.div_ceil(REFS_PER_BLOCK);
            let need_rt = (need_rb * 8).div_ceil(CLUSTER);
            if (need_rb, need_rt) == (rb, rt) {
                break;
            }
            (rb, rt) = (need_rb.max(rb), need_rt.max(rt));
        }
        Layout {
            size,
            l1_entries,
            l1_clusters,
            rt_clusters: rt,
            rb_clusters: rb,
            l2,
            data,
        }
    }

    fn l1_at(&self) -> u64 {
        1
    }
    fn rt_at(&self) -> u64 {
        self.l1_at() + self.l1_clusters
    }
    fn rb_at(&self) -> u64 {
        self.rt_at() + self.rt_clusters
    }
    fn l2_at(&self) -> u64 {
        self.rb_at() + self.rb_clusters
    }
    fn data_at(&self) -> u64 {
        self.l2_at() + self.l2.len() as u64
    }

    /// The clusters of the file.
    pub fn total(&self) -> u64 {
        self.data_at() + self.data.len() as u64
    }

    /// The header of the file (one cluster, zeros past it).
    pub fn header(&self) -> Vec<u8> {
        let mut h = vec![0u8; CLUSTER as usize];
        let mut put = |at: usize, v: &[u8]| h[at..at + v.len()].copy_from_slice(v);
        put(0, &MAGIC.to_be_bytes());
        put(4, &3u32.to_be_bytes());
        put(20, &CLUSTER_BITS.to_be_bytes());
        put(24, &self.size.to_be_bytes());
        put(36, &(self.l1_entries as u32).to_be_bytes());
        put(40, &(self.l1_at() * CLUSTER).to_be_bytes());
        put(48, &(self.rt_at() * CLUSTER).to_be_bytes());
        put(56, &(self.rt_clusters as u32).to_be_bytes());
        put(96, &4u32.to_be_bytes()); // refcount_order: 16 bits
        put(100, &HEADER_LEN.to_be_bytes());
        // At 104: the end of the header extensions (type 0), zeros.
        h
    }

    /// The tables of the file: each (offset in bytes, bytes), in order.
    pub fn tables(&self) -> Vec<(u64, Vec<u8>)> {
        let mut out = Vec::new();
        let mut l1 = vec![0u8; (self.l1_clusters * CLUSTER) as usize];
        for (k, j) in self.l2.iter().enumerate() {
            let at = (j * 8) as usize;
            let v = ((self.l2_at() + k as u64) * CLUSTER) | COPIED;
            l1[at..at + 8].copy_from_slice(&v.to_be_bytes());
        }
        out.push((self.l1_at() * CLUSTER, l1));
        let mut rt = vec![0u8; (self.rt_clusters * CLUSTER) as usize];
        for k in 0..self.rb_clusters {
            let at = (k * 8) as usize;
            rt[at..at + 8].copy_from_slice(&((self.rb_at() + k) * CLUSTER).to_be_bytes());
        }
        out.push((self.rt_at() * CLUSTER, rt));
        let mut rb = vec![0u8; (self.rb_clusters * CLUSTER) as usize];
        for c in 0..self.total() {
            rb[(c * 2) as usize + 1] = 1;
        }
        out.push((self.rb_at() * CLUSTER, rb));
        // `data` is sorted: the clusters of each L2 table come together.
        let mut n = 0usize;
        for (k, j) in self.l2.iter().enumerate() {
            let mut t = vec![0u8; CLUSTER as usize];
            while let Some(c) = self.data.get(n).filter(|c| *c / L2_ENTRIES == *j) {
                let at = ((c % L2_ENTRIES) * 8) as usize;
                let v = ((self.data_at() + n as u64) * CLUSTER) | COPIED;
                t[at..at + 8].copy_from_slice(&v.to_be_bytes());
                n += 1;
            }
            out.push(((self.l2_at() + k as u64) * CLUSTER, t));
        }
        out
    }

    /// The offset in the file of the data cluster `n` of [`Layout::data`].
    pub fn data_offset(&self, n: usize) -> u64 {
        (self.data_at() + n as u64) * CLUSTER
    }
}

/// The extents of `f` that can hold bytes: `SEEK_DATA` and `SEEK_HOLE`
/// where the file system has them, else the whole file.
fn extents(f: &File, size: u64) -> Vec<(u64, u64)> {
    use std::os::fd::AsRawFd;
    unsafe extern "C" {
        fn lseek(fd: i32, off: i64, whence: i32) -> i64;
    }
    const SEEK_DATA: i32 = 3;
    const SEEK_HOLE: i32 = 4;
    let fd = f.as_raw_fd();
    let mut out = Vec::new();
    let mut at = 0u64;
    while at < size {
        // SAFETY: lseek on a descriptor that `f` holds open.
        let data = unsafe { lseek(fd, at as i64, SEEK_DATA) };
        if data < 0 {
            // ENXIO: no data past `at`; another error: no hole support.
            let e = std::io::Error::last_os_error().raw_os_error();
            if e != Some(6) {
                return vec![(0, size)];
            }
            break;
        }
        // SAFETY: as above.
        let hole = unsafe { lseek(fd, data, SEEK_HOLE) };
        if hole <= data {
            return vec![(0, size)];
        }
        out.push((data as u64, (hole as u64).min(size)));
        at = hole as u64;
    }
    out
}

/// The clusters of the raw disk `f` of `size` bytes that hold a byte that
/// is not zero, sorted.
pub fn used_clusters(f: &File, size: u64) -> std::io::Result<Vec<u64>> {
    let mut out: Vec<u64> = Vec::new();
    let mut buf = vec![0u8; CLUSTER as usize];
    for (start, end) in extents(f, size) {
        let mut c = start / CLUSTER;
        while c * CLUSTER < end {
            if out.last() != Some(&c) {
                let n = read_cluster(f, c, size, &mut buf)?;
                if buf[..n].iter().any(|&b| b != 0) {
                    out.push(c);
                }
            }
            c += 1;
        }
    }
    Ok(out)
}

/// Read the cluster `c` of the raw disk into `buf`; the bytes read (the
/// last cluster can be short).
fn read_cluster(f: &File, c: u64, size: u64, buf: &mut [u8]) -> std::io::Result<usize> {
    let n = (size - c * CLUSTER).min(CLUSTER) as usize;
    buf.fill(0);
    f.read_exact_at(&mut buf[..n], c * CLUSTER)?;
    Ok(n)
}

/// Write the qcow2 file of the raw disk `raw` into `out`, a new empty
/// file. The bytes of the file.
pub fn write(raw: &File, out: &File) -> Result<u64, String> {
    let io = |e: std::io::Error| format!("the qcow2 disk: {e}");
    let size = raw.metadata().map_err(io)?.len();
    if size == 0 || !size.is_multiple_of(512) {
        return Err(format!(
            "the raw disk has {size} bytes, not a whole count of sectors"
        ));
    }
    let used = used_clusters(raw, size).map_err(io)?;
    let l = Layout::of(size, used);
    out.write_all_at(&l.header(), 0).map_err(io)?;
    for (at, bytes) in l.tables() {
        out.write_all_at(&bytes, at).map_err(io)?;
    }
    let mut buf = vec![0u8; CLUSTER as usize];
    for (n, c) in l.data.iter().enumerate() {
        read_cluster(raw, *c, size, &mut buf).map_err(io)?;
        out.write_all_at(&buf, l.data_offset(n)).map_err(io)?;
    }
    let len = l.total() * CLUSTER;
    out.set_len(len).map_err(io)?;
    Ok(len)
}
