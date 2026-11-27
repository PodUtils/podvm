//! The root disk of a guest written from the tree of an image, with no
//! tool of the host (TODO/machine.md T-1994).
//!
//! The file system is the ext4 of [`super::ext`], sized to its content:
//! [`plan`] counts the inodes and the blocks of the tree and fits a layout
//! with no free block to spare; [`write`] gives each inode its blocks in
//! the order of the walk (a parent before its children), each run of
//! blocks in one group, and writes the data, the directories, the extent
//! leaves, the attribute blocks, the inodes, the bitmaps and the heads. A
//! block of zeros in a file stays a hole of the disk file.
//!
//! The guest mounts the disk read-only. The file system has no journal,
//! no `lost+found` and no checksums.

use std::fs::File;
use std::io::Read;
use std::os::unix::fs::FileExt;

use super::ext::{self, Layout, BLOCK, FIRST_INO, INODE_SIZE, ROOT_INO};
use super::inode::{self, Fields, Placed, Xattr, EXTENTS_FL, EXTENT_MAX};
use super::tree::{Kind, Node, Tree};

/// A floor of the data blocks of one group: 32768 less the most metadata
/// that a group has (a superblock, 1024 blocks of descriptors, two bitmaps
/// and 2048 blocks of inodes). It bounds the runs of a file.
const MIN_RUN: u64 = 29_000;
/// The longest target of a link: a block less its NUL.
const LINK_MAX: usize = BLOCK as usize - 1;
/// The longest target that the inode holds itself (a fast link).
const FAST_LINK: usize = 60;

/// The plan of one disk: the layout, the inode of each node, and the
/// path, the parent and the attributes of each node.
pub struct Plan {
    pub layout: Layout,
    paths: Vec<String>,
    parent: Vec<usize>,
    xattrs: Vec<Placed>,
}

fn ino(n: usize) -> u32 {
    if n == 0 {
        ROOT_INO
    } else {
        FIRST_INO + n as u32 - 1
    }
}

fn mode(n: &Node) -> u32 {
    let kind = match n.kind {
        Kind::Dir(_) => 0o040_000,
        Kind::Reg { .. } => 0o100_000,
        Kind::Link(_) => 0o120_000,
        Kind::Fifo => 0o010_000,
        Kind::Chr(..) => 0o020_000,
        Kind::Blk(..) => 0o060_000,
    };
    kind | n.perm
}

fn entries<'a>(t: &'a Tree, list: &'a [(Vec<u8>, usize)]) -> Vec<(u32, u8, &'a [u8])> {
    list.iter()
        .map(|(name, c)| (ino(*c), inode::dirent_type(mode(&t.nodes[*c])), &name[..]))
        .collect()
}

/// The blocks of data of node `n`: its bytes, its directory or its link.
fn data_blocks(t: &Tree, n: usize, parent: usize) -> u64 {
    match &t.nodes[n].kind {
        Kind::Reg { size, .. } => size.div_ceil(BLOCK),
        Kind::Dir(list) => inode::dir_blocks(ino(n), ino(parent), &entries(t, list)).len() as u64,
        Kind::Link(target) if target.len() >= FAST_LINK => 1,
        _ => 0,
    }
}

/// The plan of the disk of `t`, or why the disk cannot hold the tree.
pub fn plan(t: &Tree) -> Result<Plan, String> {
    plan_with(t, 0, 0)
}

/// [`plan`] with `spare_blocks` free blocks and `spare_inodes` free inodes
/// past the tree: the writable root of an incus VM (TODO/machine.md
/// T-1998).
pub fn plan_with(t: &Tree, spare_blocks: u64, spare_inodes: u64) -> Result<Plan, String> {
    let n = t.nodes.len();
    if n == 0 || !matches!(t.nodes[0].kind, Kind::Dir(_)) {
        return Err("the tree has no root directory".to_string());
    }
    let mut paths = vec![String::new(); n];
    let mut parent = vec![0usize; n];
    for d in 0..n {
        if let Kind::Dir(list) = &t.nodes[d].kind {
            for (name, c) in list {
                if *c > d && paths[*c].is_empty() {
                    paths[*c] = format!("{}/{}", paths[d], String::from_utf8_lossy(name));
                    parent[*c] = d;
                }
            }
        }
    }
    let mut xattrs = Vec::with_capacity(n);
    let mut data = 0u64;
    for (i, node) in t.nodes.iter().enumerate() {
        let at = |why: String| format!("{}: {why}", if i == 0 { "/" } else { &paths[i] });
        if let Kind::Link(target) = &node.kind
            && (target.is_empty() || target.len() > LINK_MAX)
        {
            return Err(at(format!("a link target of {} bytes", target.len())));
        }
        let list = node
            .xattrs
            .iter()
            .map(|(k, v)| inode::xattr(k, v))
            .collect::<Result<Vec<Xattr>, _>>()
            .map_err(at)?;
        let placed = inode::place(&list).map_err(at)?;
        let blocks = data_blocks(t, i, parent[i]);
        let runs = if blocks == 0 { 0 } else { 2 + blocks / MIN_RUN };
        data += blocks + inode::leaves_for(runs as usize) as u64;
        data += u64::from(matches!(placed, Placed::Block(_)));
        xattrs.push(placed);
    }
    let layout = Layout::fit(
        data.saturating_add(spare_blocks),
        (u64::from(FIRST_INO) - 2 + n as u64).saturating_add(spare_inodes),
    )?;
    Ok(Plan {
        layout,
        paths,
        parent,
        xattrs,
    })
}

/// The blocks of the disk in use, and the cursor of the next free block.
struct Alloc<'a> {
    l: &'a Layout,
    used: Vec<u8>,
    group: u64,
    next: u64,
}

impl<'a> Alloc<'a> {
    fn new(l: &'a Layout) -> Alloc<'a> {
        let mut a = Alloc {
            l,
            used: vec![0u8; l.blocks.div_ceil(8) as usize],
            group: 0,
            next: l.overhead(0),
        };
        for g in 0..l.groups {
            a.mark(l.group_start(g), l.overhead(g));
        }
        a
    }

    fn mark(&mut self, start: u64, len: u64) {
        for b in start..start + len {
            self.used[(b / 8) as usize] |= 1 << (b % 8);
        }
    }

    fn is_used(&self, b: u64) -> bool {
        self.used[(b / 8) as usize] & (1 << (b % 8)) != 0
    }

    /// The next run of at most `want` free blocks, in one group.
    fn take(&mut self, want: u64) -> Result<(u64, u64), String> {
        loop {
            if self.group >= self.l.groups {
                return Err("the plan of the root disk has fewer blocks than its tree".into());
            }
            let end = self.l.group_start(self.group) + self.l.group_blocks(self.group);
            if self.next >= end {
                self.group += 1;
                if self.group < self.l.groups {
                    self.next = self.l.group_start(self.group) + self.l.overhead(self.group);
                }
                continue;
            }
            let len = want.min(end - self.next).min(EXTENT_MAX);
            let start = self.next;
            self.mark(start, len);
            self.next += len;
            return Ok((start, len));
        }
    }

    fn runs(&mut self, mut blocks: u64) -> Result<Vec<(u64, u64)>, String> {
        let mut out = Vec::new();
        while blocks > 0 {
            let r = self.take(blocks)?;
            blocks -= r.1;
            out.push(r);
        }
        Ok(out)
    }
}

/// Open the file `src` of the tree for reading. The walk read it with the
/// owner's bits, then gave each mode back: a file of mode 0000 in a
/// directory of mode 0000 (an image can have both) is the user's own but
/// cannot be reached by a user that is not root. Where the open is
/// refused, each directory above `src` that its owner cannot search gets
/// the owner's search bit, from the top down, for the open only; each mode
/// goes back after it, from the bottom up. An open descriptor reads on.
fn open_own(src: &std::path::Path) -> std::io::Result<File> {
    use std::os::unix::fs::PermissionsExt;
    let open = || podvm_extract::tree::with_owner_bits(src, 0o400, || File::open(src));
    match open() {
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {}
        r => return r,
    }
    let mut above: Vec<&std::path::Path> = src.ancestors().skip(1).collect();
    above.reverse();
    let mut given: Vec<(&std::path::Path, u32)> = Vec::new();
    for dir in above {
        let Ok(md) = std::fs::symlink_metadata(dir) else {
            continue;
        };
        let mode = md.permissions().mode() & 0o7777;
        if md.is_dir()
            && mode & 0o100 == 0
            && std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode | 0o100)).is_ok()
        {
            given.push((dir, mode));
        }
    }
    let r = open();
    for (dir, mode) in given.into_iter().rev() {
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode));
    }
    r
}

/// Copy the `size` bytes of `src` into `runs`; a chunk of zeros stays a
/// hole. Err where the file has another count of bytes than the walk saw.
fn copy(f: &File, src: &std::path::Path, size: u64, runs: &[(u64, u64)]) -> Result<(), String> {
    let what = |e: std::io::Error| format!("{}: {e}", src.display());
    let mut from = open_own(src).map_err(what)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut left = size;
    for &(start, len) in runs {
        let mut off = 0u64;
        while off < len * BLOCK && left > 0 {
            let n = (buf.len() as u64).min(len * BLOCK - off).min(left) as usize;
            from.read_exact(&mut buf[..n]).map_err(|e| {
                format!(
                    "{} changed while its root disk was built: {e}",
                    src.display()
                )
            })?;
            if buf[..n].iter().any(|&b| b != 0) {
                f.write_all_at(&buf[..n], start * BLOCK + off)
                    .map_err(what)?;
            }
            off += n as u64;
            left -= n as u64;
        }
    }
    if from.read(&mut [0u8; 1]).map_err(what)? != 0 {
        return Err(format!(
            "{} changed while its root disk was built: it grew past {size} bytes",
            src.display()
        ));
    }
    Ok(())
}

/// Write the disk of `p` and `t` into `f`, a new file of zeros of at least
/// `p.layout.blocks` blocks.
pub fn write(p: &Plan, t: &Tree, f: &File, names: &ext::Names) -> Result<(), String> {
    let l = &p.layout;
    let mut a = Alloc::new(l);
    let mut dirs = vec![0u64; l.groups as usize];
    let io = |e: std::io::Error| format!("the root disk: {e}");
    for (i, node) in t.nodes.iter().enumerate() {
        let at = |why: String| format!("{}: {why}", if i == 0 { "/" } else { &p.paths[i] });
        let blocks = data_blocks(t, i, p.parent[i]);
        let runs = a.runs(blocks)?;
        let (mut size, mut links, mut iblock, mut flags) = (0u64, node.links, vec![0u8; 60], 0);
        match &node.kind {
            Kind::Reg { src, size: s } => {
                copy(f, src, *s, &runs)?;
                size = *s;
            }
            Kind::Dir(list) => {
                let all = inode::dir_blocks(ino(i), ino(p.parent[i]), &entries(t, list));
                let mut k = all.iter();
                for &(start, len) in &runs {
                    for b in start..start + len {
                        let block = k
                            .next()
                            .ok_or("a directory has fewer blocks than its plan")?;
                        f.write_all_at(block, b * BLOCK).map_err(io)?;
                    }
                }
                let subdirs = list
                    .iter()
                    .filter(|(_, c)| matches!(t.nodes[*c].kind, Kind::Dir(_)))
                    .count() as u64;
                // dir_nlink: a count past 65000 is 1.
                links = if subdirs + 2 > 65_000 {
                    1
                } else {
                    subdirs as u32 + 2
                };
                size = blocks * BLOCK;
                dirs[((ino(i) - 1) as u64 / l.inodes_per_group) as usize] += 1;
            }
            Kind::Link(target) => {
                size = target.len() as u64;
                if let Some(&(start, _)) = runs.first() {
                    f.write_all_at(target, start * BLOCK).map_err(io)?;
                } else {
                    iblock[..target.len()].copy_from_slice(target);
                }
            }
            Kind::Chr(major, minor) | Kind::Blk(major, minor) => {
                iblock = inode::device(*major, *minor);
            }
            Kind::Fifo => {}
        }
        let mut sectors = blocks;
        if !runs.is_empty() || matches!(node.kind, Kind::Reg { .. } | Kind::Dir(_)) {
            let count = inode::leaves_for(runs.len());
            let leaf_at = (0..count)
                .map(|_| a.take(1).map(|r| r.0))
                .collect::<Result<Vec<_>, _>>()?;
            let e = inode::extents(&runs, &leaf_at).map_err(at)?;
            for (leaf, b) in e.leaves.iter().zip(&leaf_at) {
                f.write_all_at(leaf, b * BLOCK).map_err(io)?;
            }
            sectors += count as u64;
            iblock = e.iblock;
            flags = EXTENTS_FL;
        }
        let (mut xattr_block, mut ibody) = (0u64, None);
        match &p.xattrs[i] {
            Placed::None => {}
            Placed::Inode(body) => ibody = Some(body.clone()),
            Placed::Block(block) => {
                xattr_block = a.take(1)?.0;
                f.write_all_at(block, xattr_block * BLOCK).map_err(io)?;
                sectors += 1;
            }
        }
        let bytes = inode::inode(&Fields {
            mode: mode(node),
            uid: node.uid,
            gid: node.gid,
            size,
            links,
            sectors: sectors * (BLOCK / 512),
            mtime: node.mtime,
            flags,
            iblock,
            xattr_block,
            ibody,
        });
        let n = u64::from(ino(i) - 1);
        let (g, k) = (n / l.inodes_per_group, n % l.inodes_per_group);
        f.write_all_at(&bytes, l.inode_table(g) * BLOCK + k * INODE_SIZE)
            .map_err(io)?;
    }
    let last_ino = u64::from(ino(t.nodes.len() - 1).max(FIRST_INO - 1));
    let mut counts = Vec::new();
    for g in 0..l.groups {
        let (start, blocks) = (l.group_start(g), l.group_blocks(g));
        let mut bits = ext::bitmap(0, blocks);
        let mut used = 0;
        for b in 0..blocks {
            if a.is_used(start + b) {
                bits[(b / 8) as usize] |= 1 << (b % 8);
                used += 1;
            }
        }
        let inodes = last_ino
            .saturating_sub(g * l.inodes_per_group)
            .min(l.inodes_per_group);
        f.write_all_at(&bits, l.block_bitmap(g) * BLOCK)
            .map_err(io)?;
        f.write_all_at(
            &ext::bitmap(inodes, l.inodes_per_group),
            (l.block_bitmap(g) + 1) * BLOCK,
        )
        .map_err(io)?;
        counts.push((blocks - used, l.inodes_per_group - inodes, dirs[g as usize]));
    }
    // The time of the newest file, not of the build: the same tree gives
    // the same bytes.
    let when = t.nodes.iter().map(|n| n.mtime.0).max().unwrap_or(0).max(0) as u64;
    ext::write_heads(f, l, names, &counts, when).map_err(io)
}
