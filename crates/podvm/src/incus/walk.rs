//! What a path of an instance is, after its links, as the files API of
//! incusd sees it (TODO/cli.md T-2079: OR-B39-09, OR-B39-17).
//!
//! incusd gives the kind of the last component of a path (`X-Incus-type`)
//! and the target of a link as its body; it resolves the links of the
//! other components in the root of the instance. So podvm follows a link
//! of the last component itself, up to [`MAX_LINKS`] links.

use pod_incus::{Client, ErrorKind, FileContent, FileKind};

/// The most links that [`find`] follows, as the kernel (`ELOOP` after 40).
pub const MAX_LINKS: usize = 40;

/// What a path is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// No entry: incusd answered 404.
    Missing,
    /// A file or a directory, after each link.
    Kind(FileKind),
    /// podvm cannot tell: the reason.
    Unknown(String),
}

/// The path that the link `link` with the target `target` names: an
/// absolute target as it is, else the target in the directory of the link.
/// `.` and `..` go lexically; `..` of `/` is `/`.
pub fn join_link(link: &str, target: &str) -> String {
    let base = match target.starts_with('/') {
        true => String::new(),
        false => link
            .rsplit_once('/')
            .map(|(d, _)| d)
            .unwrap_or("")
            .to_string(),
    };
    normal(&format!("{base}/{target}"))
}

/// `path` as an absolute path with no `.`, no `..` and no empty component.
pub fn normal(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            p => out.push(p),
        }
    }
    format!("/{}", out.join("/"))
}

/// What `path` of the instance `name` is, after each link.
pub async fn find(c: &Client, name: &str, path: &str) -> Found {
    let mut p = normal(path);
    for _ in 0..=MAX_LINKS {
        if p == "/" {
            return Found::Kind(FileKind::Directory);
        }
        // No byte of a file: only its kind. A directory is a usage error
        // of this call (`get_file` gives its entries).
        let read = c
            .get_file_to(name, &p, None, Some(0), &mut std::io::sink())
            .await;
        match read {
            Ok(r) if r.meta.kind == FileKind::Symlink => match c.get_file(name, &p).await {
                Ok(f) => match f.content {
                    FileContent::Bytes(t) => match String::from_utf8(t) {
                        Ok(t) if !t.is_empty() => p = join_link(&p, &t),
                        _ => return Found::Unknown(format!("the link {p} has no UTF-8 target")),
                    },
                    FileContent::Entries(_) => return Found::Kind(FileKind::Directory),
                },
                Err(e) if super::verbs::gone(&e) => return Found::Missing,
                Err(e) => return Found::Unknown(e.to_string()),
            },
            Ok(r) => return Found::Kind(r.meta.kind),
            Err(e) if super::verbs::gone(&e) => return Found::Missing,
            Err(e) if matches!(e.kind(), ErrorKind::Usage(_)) => {
                return Found::Kind(FileKind::Directory)
            }
            Err(e) => return Found::Unknown(e.to_string()),
        }
    }
    Found::Unknown(format!(
        "{path}: more than {MAX_LINKS} links (the kernel says ELOOP)"
    ))
}
