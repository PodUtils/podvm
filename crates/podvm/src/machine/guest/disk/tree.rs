//! The tree of an image as the root disk of a guest holds it
//! (TODO/machine.md T-1994, T-1999): each name, kind, owner, mode, time,
//! link and extended attribute, from the extracted rootfs and the sidecar
//! of its extraction (`.meta.jsonl`).
//!
//! The extraction runs as a user that is not root, so the tree on the host
//! has the user's ids, and a mode or an attribute that the kernel refused
//! is only in the sidecar. The sidecar row of a path gives the owner and
//! the mode of the image; its `xattrs` give each attribute that is not on
//! the tree (`security.capability` of `ping` is EPERM there). The tree gives
//! `security.capability`, `user.*` and the POSIX ACLs that it holds. A name
//! that the host gives a file (`security.selinux`) is the host's.
//!
//! A socket of the tree is left out: no image file system holds one that
//! a process can use.

use std::collections::HashMap;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::incus::xattrs::{self, Attr};

/// What one inode is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// The names in the directory, sorted, and the index of each node.
    Dir(Vec<(Vec<u8>, usize)>),
    /// A regular file: where its bytes are, and their count at the walk.
    Reg {
        src: PathBuf,
        size: u64,
    },
    /// A symbolic link and its target.
    Link(Vec<u8>),
    Fifo,
    /// A character or a block device: major and minor.
    Chr(u32, u32),
    Blk(u32, u32),
}

/// One inode of the image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub kind: Kind,
    /// The permission bits with set-id and sticky (`0o7777`).
    pub perm: u32,
    pub uid: u32,
    pub gid: u32,
    /// The modification time: seconds and nanoseconds.
    pub mtime: (i64, u32),
    pub xattrs: Vec<Attr>,
    /// The names of the inode: more than one for a hard link.
    pub links: u32,
}

/// The tree: `nodes[0]` is the root directory.
#[derive(Debug, Default)]
pub struct Tree {
    pub nodes: Vec<Node>,
}

/// Whether an attribute on the tree is of the image (see the module note).
pub fn of_image(name: &str) -> bool {
    name == "security.capability"
        || name.starts_with("user.")
        || name == "system.posix_acl_access"
        || name == "system.posix_acl_default"
}

/// The rows of the sidecar: the owner and the mode of each path, and the
/// attributes that the extraction did not set.
struct Sidecar {
    meant: HashMap<String, (u64, u64, u32)>,
    unset: HashMap<String, Vec<Attr>>,
}

impl Tree {
    /// The tree of `rootfs`, with the sidecar `.meta.jsonl` beside it
    /// (none: each file is root's, with the mode of the tree).
    pub fn walk(rootfs: &Path) -> Result<Tree, String> {
        let text =
            std::fs::read_to_string(rootfs.with_file_name(".meta.jsonl")).unwrap_or_default();
        let side = Sidecar {
            meant: crate::incus::image::intended(&text),
            unset: xattrs::unset_of(&text),
        };
        let mut t = Tree::default();
        let mut links = HashMap::new();
        let md = std::fs::symlink_metadata(rootfs).map_err(|e| at(rootfs, e))?;
        if !md.is_dir() {
            return Err(format!("{}: not a directory", rootfs.display()));
        }
        t.nodes
            .push(node(rootfs, "", &md, &side, Kind::Dir(Vec::new()))?);
        podvm_extract::tree::with_owner_bits(rootfs, 0o500, || {
            t.dir(rootfs, "", 0, &side, &mut links)
                .map_err(io::Error::other)
        })
        .map_err(|e| format!("the root disk of {}: {e}", rootfs.display()))?;
        Ok(t)
    }

    fn dir(
        &mut self,
        dir: &Path,
        rel: &str,
        me: usize,
        side: &Sidecar,
        links: &mut HashMap<(u64, u64), usize>,
    ) -> Result<(), String> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .and_then(|d| d.collect::<Result<Vec<_>, _>>())
            .map_err(|e| at(dir, e))?;
        names.sort_by_key(|e| e.file_name());
        let mut entries = Vec::new();
        for e in names {
            let name = e.file_name();
            if name.len() > 255 {
                return Err(format!(
                    "{}: a name of more than 255 bytes",
                    e.path().display()
                ));
            }
            let path = e.path();
            let r = if rel.is_empty() {
                name.to_string_lossy().into_owned()
            } else {
                format!("{rel}/{}", name.to_string_lossy())
            };
            let md = std::fs::symlink_metadata(&path).map_err(|e| at(&path, e))?;
            let ft = md.file_type();
            let key = (md.dev(), md.ino());
            if !ft.is_dir()
                && md.nlink() > 1
                && let Some(&n) = links.get(&key)
            {
                self.nodes[n].links += 1;
                entries.push((name.as_bytes().to_vec(), n));
                continue;
            }
            let (major, minor) = (dev_major(md.rdev()), dev_minor(md.rdev()));
            let kind = if ft.is_dir() {
                Kind::Dir(Vec::new())
            } else if ft.is_symlink() {
                let t = std::fs::read_link(&path).map_err(|e| at(&path, e))?;
                Kind::Link(t.as_os_str().as_bytes().to_vec())
            } else if ft.is_file() {
                Kind::Reg {
                    src: path.clone(),
                    size: md.len(),
                }
            } else if ft.is_fifo() {
                Kind::Fifo
            } else if ft.is_char_device() {
                Kind::Chr(major, minor)
            } else if ft.is_block_device() {
                Kind::Blk(major, minor)
            } else {
                continue; // a socket
            };
            let n = self.nodes.len();
            self.nodes.push(node(&path, &r, &md, side, kind)?);
            if md.nlink() > 1 && !ft.is_dir() {
                links.insert(key, n);
            }
            entries.push((name.as_bytes().to_vec(), n));
            if ft.is_dir() {
                // A directory that its owner cannot read or search (mode
                // 0000 in an image) is the user's own: it is read with the
                // owner's bits, as the archive of the guest reads it.
                podvm_extract::tree::with_owner_bits(&path, 0o500, || {
                    self.dir(&path, &r, n, side, links)
                        .map_err(io::Error::other)
                })
                .map_err(|e| e.to_string())?;
            }
        }
        self.nodes[me].kind = Kind::Dir(entries);
        Ok(())
    }
}

impl Tree {
    /// TODO/machine.md T-1998: put `kind` at the absolute path `path` of
    /// the tree, root's with `perm`, over the file of that name. Each
    /// missing directory above it comes with mode 0755; a part above it
    /// that is not a directory (a link included) is an error that names it,
    /// and so is a directory at `path`. A node of one name takes the new
    /// file in place; a hard link keeps its other names. The new nodes take
    /// the newest time of the tree, so the same tree gives the same disk.
    pub fn put(&mut self, path: &str, kind: Kind, perm: u32) -> Result<(), String> {
        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        let Some((last, above)) = parts.split_last() else {
            return Err(format!("{path:?}: no name to put in the tree"));
        };
        if parts
            .iter()
            .any(|p| *p == "." || *p == ".." || p.len() > 255)
        {
            return Err(format!("{path:?}: not a plain path"));
        }
        let mtime = self.nodes.iter().map(|n| n.mtime).max().unwrap_or((0, 0));
        let fresh = |kind: Kind, perm: u32| Node {
            kind,
            perm,
            uid: 0,
            gid: 0,
            mtime,
            xattrs: Vec::new(),
            links: 1,
        };
        let mut dir = 0usize;
        for part in above {
            let found = self.child(dir, part);
            dir = match found {
                Some(n) if matches!(self.nodes[n].kind, Kind::Dir(_)) => n,
                Some(_) => return Err(format!("{path}: {part} above it is not a directory")),
                None => {
                    let n = self.nodes.len();
                    self.nodes.push(fresh(Kind::Dir(Vec::new()), 0o755));
                    self.link(dir, part, n);
                    n
                }
            };
        }
        match self.child(dir, last) {
            Some(n) if matches!(self.nodes[n].kind, Kind::Dir(_)) => {
                Err(format!("{path}: the image has a directory there"))
            }
            Some(n) if self.nodes[n].links == 1 => {
                self.nodes[n] = fresh(kind, perm);
                Ok(())
            }
            Some(n) => {
                self.nodes[n].links -= 1;
                let m = self.nodes.len();
                self.nodes.push(fresh(kind, perm));
                self.unlink(dir, last);
                self.link(dir, last, m);
                Ok(())
            }
            None => {
                let m = self.nodes.len();
                self.nodes.push(fresh(kind, perm));
                self.link(dir, last, m);
                Ok(())
            }
        }
    }

    /// The node of the name `name` in the directory `dir`.
    fn child(&self, dir: usize, name: &str) -> Option<usize> {
        match &self.nodes[dir].kind {
            Kind::Dir(list) => list
                .iter()
                .find(|(n, _)| n.as_slice() == name.as_bytes())
                .map(|(_, c)| *c),
            _ => None,
        }
    }

    /// Add the name `name` of the node `n` to `dir`, in the order of names.
    fn link(&mut self, dir: usize, name: &str, n: usize) {
        if let Kind::Dir(list) = &mut self.nodes[dir].kind {
            let at = list.partition_point(|(k, _)| k.as_slice() < name.as_bytes());
            list.insert(at, (name.as_bytes().to_vec(), n));
        }
    }

    fn unlink(&mut self, dir: usize, name: &str) {
        if let Kind::Dir(list) = &mut self.nodes[dir].kind {
            list.retain(|(k, _)| k.as_slice() != name.as_bytes());
        }
    }
}

fn at(p: &Path, e: io::Error) -> String {
    format!("{}: {e}", p.display())
}

/// glibc's `gnu_dev_major` and `gnu_dev_minor`.
fn dev_major(d: u64) -> u32 {
    (((d >> 32) & 0xffff_f000) | ((d >> 8) & 0xfff)) as u32
}

fn dev_minor(d: u64) -> u32 {
    (((d >> 12) & 0xffff_ff00) | (d & 0xff)) as u32
}

/// The node of `path` (the sidecar path `rel`): the owner and the mode of
/// the image where the sidecar has a row, and the attributes of both.
fn node(
    path: &Path,
    rel: &str,
    md: &std::fs::Metadata,
    side: &Sidecar,
    kind: Kind,
) -> Result<Node, String> {
    let (uid, gid, perm) = side
        .meant
        .get(rel)
        .map(|&(u, g, m)| (u, g, m & 0o7777))
        .unwrap_or((0, 0, md.permissions().mode() & 0o7777));
    let (uid, gid) = match (u32::try_from(uid), u32::try_from(gid)) {
        (Ok(u), Ok(g)) => (u, g),
        _ => {
            return Err(format!(
                "{}: the owner {uid}:{gid} is past 32 bits",
                path.display()
            ))
        }
    };
    // `user.*` reads need the read bit of the file: a file of mode 0000
    // is read with the owner's bit. A link has no mode of its own.
    let mine = xattrs::on_disk_readable(path, md, of_image).map_err(|e| e.to_string())?;
    Ok(Node {
        kind,
        perm,
        uid,
        gid,
        mtime: (md.mtime(), md.mtime_nsec().clamp(0, 999_999_999) as u32),
        xattrs: xattrs::merge(mine, side.unset.get(rel)),
        links: 1,
    })
}
