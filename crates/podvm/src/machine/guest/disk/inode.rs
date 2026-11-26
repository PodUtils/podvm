//! The bytes of the inodes, the extent trees, the directory blocks and the
//! extended attributes of the root disk of a guest (TODO/machine.md
//! T-1994), in the on-disk form of the kernel's `fs/ext4` (`ext4.h`,
//! `ext4_extents.h`, `xattr.h`, `acl.h`) with no checksums.

use super::ext::{put16, put32, BLOCK, INODE_SIZE};

pub const EXTENTS_FL: u64 = 0x8_0000;
const EXTENT_MAGIC: u64 = 0xf30a;
/// The extents of a leaf block: (4096 - 12) / 12.
pub const LEAF_MAX: usize = 340;
/// The most blocks of one extent of data.
pub const EXTENT_MAX: u64 = 1 << 15;
/// The extent entries in the inode itself.
const INODE_EXTENTS: usize = 4;
/// `i_extra_isize`: the in-inode attributes start after it.
const EXTRA_ISIZE: usize = 32;
const XATTR_MAGIC: u32 = 0xea02_0000;
/// The bytes of the inode after `i_extra_isize` and the magic.
const IBODY: usize = INODE_SIZE as usize - 128 - EXTRA_ISIZE - 4;

/// The tree of extents of one file: the 60 bytes of `i_block`, and the
/// leaf blocks with the block number that each one needs (the caller
/// gives them in order).
#[derive(Debug, PartialEq, Eq)]
pub struct Extents {
    pub iblock: Vec<u8>,
    pub leaves: Vec<Vec<u8>>,
}

fn header(b: &mut [u8], entries: usize, max: usize, depth: u64) {
    put16(b, 0, EXTENT_MAGIC);
    put16(b, 2, entries as u64);
    put16(b, 4, max as u64);
    put16(b, 6, depth);
}

fn extent(b: &mut [u8], logical: u64, start: u64, len: u64) {
    put32(b, 0, logical);
    put16(b, 4, len);
    put16(b, 6, start >> 32);
    put32(b, 8, start);
}

/// How many leaf blocks `n` extents need: none where the inode holds them.
pub fn leaves_for(n: usize) -> usize {
    if n <= INODE_EXTENTS {
        0
    } else {
        n.div_ceil(LEAF_MAX)
    }
}

/// The extents of `runs` (start block, length), at logical blocks from 0
/// in order; `leaf_at` gives the block of each leaf. Err where the runs
/// need more than four leaves (a depth of 2, which no image needs: each
/// leaf holds 340 runs of up to 128 MiB).
pub fn extents(runs: &[(u64, u64)], leaf_at: &[u64]) -> Result<Extents, String> {
    let mut iblock = vec![0u8; 60];
    let n = runs.len();
    if let Some(r) = runs.iter().find(|r| r.1 == 0 || r.1 > EXTENT_MAX) {
        return Err(format!("an extent of {} blocks", r.1));
    }
    let logical: Vec<u64> = runs
        .iter()
        .scan(0u64, |l, r| {
            let at = *l;
            *l += r.1;
            Some(at)
        })
        .collect();
    if n <= INODE_EXTENTS {
        header(&mut iblock, n, INODE_EXTENTS, 0);
        for (i, (r, l)) in runs.iter().zip(&logical).enumerate() {
            extent(&mut iblock[12 + 12 * i..], *l, r.0, r.1);
        }
        return Ok(Extents {
            iblock,
            leaves: Vec::new(),
        });
    }
    let count = leaves_for(n);
    if count > INODE_EXTENTS {
        return Err(format!(
            "a file of {n} runs of blocks: the root disk holds {} at most",
            INODE_EXTENTS * LEAF_MAX
        ));
    }
    if leaf_at.len() != count {
        return Err(format!(
            "{count} leaves need {count} blocks, not {}",
            leaf_at.len()
        ));
    }
    header(&mut iblock, count, INODE_EXTENTS, 1);
    let mut leaves = Vec::new();
    for (i, chunk) in (0..n).collect::<Vec<_>>().chunks(LEAF_MAX).enumerate() {
        let mut leaf = vec![0u8; BLOCK as usize];
        header(&mut leaf, chunk.len(), LEAF_MAX, 0);
        for (j, &k) in chunk.iter().enumerate() {
            extent(&mut leaf[12 + 12 * j..], logical[k], runs[k].0, runs[k].1);
        }
        let e = &mut iblock[12 + 12 * i..];
        put32(e, 0, logical[chunk[0]]);
        put32(e, 4, leaf_at[i]);
        put16(e, 8, leaf_at[i] >> 32);
        leaves.push(leaf);
    }
    Ok(Extents { iblock, leaves })
}

/// The `file_type` byte of a directory entry of the mode `mode`.
pub fn dirent_type(mode: u32) -> u8 {
    match mode & 0o170_000 {
        0o100_000 => 1,
        0o040_000 => 2,
        0o020_000 => 3,
        0o060_000 => 4,
        0o010_000 => 5,
        0o140_000 => 6,
        0o120_000 => 7,
        _ => 0,
    }
}

/// The blocks of a linear directory of `entries` (inode, type, name), with
/// `.` and `..` first. No entry crosses a block, and the last entry of
/// each block takes the rest of it.
pub fn dir_blocks(me: u32, parent: u32, entries: &[(u32, u8, &[u8])]) -> Vec<Vec<u8>> {
    let all = [(me, 2u8, &b"."[..]), (parent, 2, &b".."[..])];
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    let mut cur = vec![0u8; BLOCK as usize];
    let (mut at, mut last) = (0usize, None::<usize>);
    for &(ino, ty, name) in all.iter().chain(entries) {
        let len = (8 + name.len()).div_ceil(4) * 4;
        if at + len > BLOCK as usize {
            if let Some(l) = last {
                put16(&mut cur, l + 4, (BLOCK as usize - l) as u64);
            }
            blocks.push(std::mem::replace(&mut cur, vec![0u8; BLOCK as usize]));
            at = 0;
        }
        put32(&mut cur, at, u64::from(ino));
        put16(&mut cur, at + 4, len as u64);
        cur[at + 6] = name.len() as u8;
        cur[at + 7] = ty;
        cur[at + 8..at + 8 + name.len()].copy_from_slice(name);
        last = Some(at);
        at += len;
    }
    if let Some(l) = last {
        put16(&mut cur, l + 4, (BLOCK as usize - l) as u64);
    }
    blocks.push(cur);
    blocks
}

/// One attribute as ext4 keeps it: the index of its prefix, the rest of
/// its name, and its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xattr {
    pub index: u8,
    pub name: Vec<u8>,
    pub value: Vec<u8>,
}

/// The ext4 form of the attribute `name` with `value`, or why the root
/// disk cannot hold it. A POSIX ACL changes from the form of the VFS to
/// the form of ext4.
pub fn xattr(name: &str, value: &[u8]) -> Result<Xattr, String> {
    let (index, rest, value) = match name {
        "system.posix_acl_access" => (2, "", acl(value)?),
        "system.posix_acl_default" => (3, "", acl(value)?),
        _ => {
            let (index, rest) = [
                ("user.", 1),
                ("trusted.", 4),
                ("security.", 6),
                ("system.", 7),
            ]
            .iter()
            .find_map(|(p, i)| name.strip_prefix(p).map(|r| (*i, r)))
            .ok_or_else(|| format!("the attribute {name}: ext4 has no prefix for its name"))?;
            (index, rest, value.to_vec())
        }
    };
    if (rest.is_empty() && index > 3) || rest.len() > 255 {
        return Err(format!("the attribute {name}: not a name that ext4 holds"));
    }
    Ok(Xattr {
        index,
        name: rest.as_bytes().to_vec(),
        value,
    })
}

/// A POSIX ACL of the VFS (`posix_acl_xattr_header`: version 2, then
/// entries of tag, permissions and id) in the form of ext4 (version 1; the
/// id only for a named user or group).
pub fn acl(v: &[u8]) -> Result<Vec<u8>, String> {
    let bad = |why: &str| Err(format!("a POSIX ACL of {} bytes: {why}", v.len()));
    if v.len() < 4 || !(v.len() - 4).is_multiple_of(8) {
        return bad("not a header and entries of 8 bytes");
    }
    if u32::from_le_bytes([v[0], v[1], v[2], v[3]]) != 2 {
        return bad("not version 2");
    }
    let mut out = 1u32.to_le_bytes().to_vec();
    for e in v[4..].chunks(8) {
        let tag = u16::from_le_bytes([e[0], e[1]]);
        match tag {
            0x01 | 0x04 | 0x10 | 0x20 => out.extend_from_slice(e.split_at(4).0),
            0x02 | 0x08 => out.extend_from_slice(e),
            _ => return bad(&format!("the tag {tag:#x}")),
        }
    }
    Ok(out)
}

fn pad4(n: usize) -> usize {
    n.div_ceil(4) * 4
}

/// `ext4_xattr_hash_entry`: the hash of a name and a value padded to
/// words.
pub fn hash(x: &Xattr) -> u32 {
    let mut h = 0u32;
    for &c in &x.name {
        h = (h << 5) ^ (h >> 27) ^ u32::from(c);
    }
    let mut v = x.value.clone();
    v.resize(pad4(v.len()), 0);
    for w in v.chunks(4) {
        h = (h << 16) ^ (h >> 16) ^ u32::from_le_bytes([w[0], w[1], w[2], w[3]]);
    }
    h
}

/// The entries of `attrs` in `area` from `first`, sorted as the kernel
/// finds them in a block (index, length, name), with each value at the end
/// of the area and its offset from `base`. `None` where they do not fit.
fn pack(attrs: &[Xattr], area: &mut [u8], first: usize, base: usize) -> Option<Vec<u32>> {
    let mut sorted: Vec<&Xattr> = attrs.iter().collect();
    sorted.sort_by(|a, b| (a.index, a.name.len(), &a.name).cmp(&(b.index, b.name.len(), &b.name)));
    let names: usize = sorted.iter().map(|x| pad4(16 + x.name.len())).sum();
    let values: usize = sorted.iter().map(|x| pad4(x.value.len())).sum();
    if first + names + 4 + values > area.len() {
        return None;
    }
    let (mut at, mut end, mut hashes) = (first, area.len(), Vec::new());
    for x in sorted {
        let h = hash(x);
        let offs = if x.value.is_empty() {
            0
        } else {
            end -= pad4(x.value.len());
            area[end..end + x.value.len()].copy_from_slice(&x.value);
            end - base
        };
        area[at] = x.name.len() as u8;
        area[at + 1] = x.index;
        put16(area, at + 2, offs as u64);
        put32(area, at + 8, x.value.len() as u64);
        put32(area, at + 12, u64::from(h));
        area[at + 16..at + 16 + x.name.len()].copy_from_slice(&x.name);
        at += pad4(16 + x.name.len());
        hashes.push(h);
    }
    Some(hashes)
}

/// Where the attributes of one inode go.
#[derive(Debug, PartialEq, Eq)]
pub enum Placed {
    None,
    /// The bytes after `i_extra_isize`, in the inode itself.
    Inode(Vec<u8>),
    /// A block of their own.
    Block(Vec<u8>),
}

/// The attributes of one inode in the inode where they fit, else in a
/// block; Err where one block does not hold them (ext4 with no
/// `ea_inode` keeps the values of one inode in one block).
pub fn place(attrs: &[Xattr]) -> Result<Placed, String> {
    if attrs.is_empty() {
        return Ok(Placed::None);
    }
    let mut body = vec![0u8; 4 + IBODY];
    put32(&mut body, 0, u64::from(XATTR_MAGIC));
    if pack(attrs, &mut body, 4, 4).is_some() {
        return Ok(Placed::Inode(body));
    }
    let mut block = vec![0u8; BLOCK as usize];
    let Some(hashes) = pack(attrs, &mut block, 32, 0) else {
        let bytes: usize = attrs.iter().map(|x| x.value.len()).sum();
        return Err(format!(
            "{} attributes with {bytes} bytes of values: one block of 4096 bytes does not hold \
             them",
            attrs.len()
        ));
    };
    put32(&mut block, 0, u64::from(XATTR_MAGIC));
    put32(&mut block, 4, 1); // h_refcount
    put32(&mut block, 8, 1); // h_blocks
    let h = hashes.iter().fold(0u32, |h, e| (h << 16) ^ (h >> 16) ^ e);
    put32(&mut block, 12, u64::from(h));
    Ok(Placed::Block(block))
}

/// What one inode holds besides its data.
pub struct Fields {
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub links: u32,
    /// The blocks of 512 bytes: data, leaves and the attribute block.
    pub sectors: u64,
    pub mtime: (i64, u32),
    pub flags: u64,
    pub iblock: Vec<u8>,
    pub xattr_block: u64,
    pub ibody: Option<Vec<u8>>,
}

/// The 256 bytes of one inode.
pub fn inode(f: &Fields) -> Vec<u8> {
    let mut i = vec![0u8; INODE_SIZE as usize];
    let (secs, nsec) = f.mtime;
    // The 32 bits of seconds are signed; the two epoch bits of each extra
    // field carry the rest.
    let epoch = (((secs - i64::from(secs as i32)) >> 32) & 3) as u64;
    let extra = (u64::from(nsec) << 2) | epoch;
    put16(&mut i, 0, u64::from(f.mode));
    put16(&mut i, 2, u64::from(f.uid));
    put32(&mut i, 4, f.size);
    for at in [8, 12, 16] {
        put32(&mut i, at, secs as u64);
    }
    put16(&mut i, 24, u64::from(f.gid));
    put16(&mut i, 26, u64::from(f.links));
    put32(&mut i, 28, f.sectors);
    put32(&mut i, 32, f.flags);
    i[40..40 + f.iblock.len()].copy_from_slice(&f.iblock);
    put32(&mut i, 104, f.xattr_block);
    put32(&mut i, 108, f.size >> 32);
    put16(&mut i, 118, f.xattr_block >> 32);
    put16(&mut i, 120, u64::from(f.uid >> 16));
    put16(&mut i, 122, u64::from(f.gid >> 16));
    put16(&mut i, 128, EXTRA_ISIZE as u64);
    for at in [132, 136, 140, 148] {
        put32(&mut i, at, extra);
    }
    put32(&mut i, 144, secs as u64); // i_crtime
    if let Some(body) = &f.ibody {
        i[128 + EXTRA_ISIZE..].copy_from_slice(body);
    }
    i
}

/// The `i_block` of a device: the old encoding where the major and the
/// minor are below 256, else the new one in the second word.
pub fn device(major: u32, minor: u32) -> Vec<u8> {
    let mut b = vec![0u8; 60];
    if major < 256 && minor < 256 {
        put32(&mut b, 0, u64::from((major << 8) | minor));
    } else {
        let v = (minor & 0xff) | (major << 8) | ((minor & !0xff) << 12);
        put32(&mut b, 4, u64::from(v));
    }
    b
}
