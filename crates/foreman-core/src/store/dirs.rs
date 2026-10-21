//! Creating directories through the store (AC-F018-03: setup creates empty `docs/handoff/`,
//! `spec/` …). The same component walk as reads and writes: no symlink at any component, the deny
//! rules on every reported name, repo directories `0755`, app-data directories `0700`. Added in
//! Stage 5 for project setup (ticket M2-17); nothing else changes in the store.

use crate::pathpolicy::RepoPath;

use super::{Root, StoreError, walk};

/// Creates every component of `p` that doesn't exist yet (like `mkdir -p`, but refusing
/// symlinks and denied names). Existing directories are left alone.
///
/// # Errors
/// A policy refusal, a component that is a file or a symlink, or an OS error.
pub fn ensure_dir(root: &Root, p: &RepoPath) -> Result<(), StoreError> {
    let comps: Vec<&str> = p.components().collect();
    let exempt = Root::exempt_for(p);
    walk::walk(root.fd(), &comps, root.opts(true, exempt)).map(|_| ())
}

/// What is at a path, read without following any symlink (the build-package assembler's row
/// state, F-023).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// Nothing there.
    Missing,
    /// A regular file of this many bytes.
    File(u64),
    /// A directory with this many entries (`.` and `..` not counted).
    Dir(usize),
    /// A symlink, FIFO, device or socket (never followed).
    Other,
}

/// Stats `p` under `root` without following symlinks; a directory's entries are counted through
/// its own fd. The parents are walked like a read (no symlink at any component, the deny rules on
/// every reported name).
///
/// # Errors
/// A policy refusal on a parent, or an OS error other than not-found.
pub fn entry_kind(root: &Root, p: &RepoPath) -> Result<EntryKind, StoreError> {
    use rustix::fs::{AtFlags, FileType};
    let (parents, name) = p.split_last();
    let dir = match walk::walk(root.fd(), &parents, root.opts(false, None)) {
        Ok(d) => d,
        Err(StoreError::NotFound(_)) => return Ok(EntryKind::Missing),
        Err(e) => return Err(e),
    };
    let st = match rustix::fs::statat(&dir, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(st) => st,
        Err(rustix::io::Errno::NOENT) => return Ok(EntryKind::Missing),
        Err(e) => return Err(StoreError::io(name, e)),
    };
    match FileType::from_raw_mode(super::stat_mode(&st)) {
        FileType::RegularFile => Ok(EntryKind::File(u64::try_from(st.st_size).unwrap_or(0))),
        FileType::Directory => {
            let fd = walk::open_dir(&dir, name, root.opts(false, None))?;
            let mut listing =
                rustix::fs::Dir::read_from(&fd).map_err(|err| StoreError::io(name, err))?;
            let mut count = 0usize;
            while let Some(entry) = listing.read() {
                let entry = entry.map_err(|err| StoreError::io(name, err))?;
                let bytes = entry.file_name().to_bytes();
                if bytes != b"." && bytes != b".." {
                    count += 1;
                }
            }
            Ok(EntryKind::Dir(count))
        }
        _ => Ok(EntryKind::Other),
    }
}

/// Folder names [`list_paths`] never descends into: dependency and build output trees.
const SKIP_DIRS: [&str; 4] = ["node_modules", "target", "dist", "build"];

/// The repo-relative paths that exist under `root`, for path evidence only (DR-063: names,
/// never contents). Depth-first, sorted per folder, at most `max_depth` folders deep and
/// `max_entries` entries; hidden entries (a leading `.`), dependency trees and symlinks are
/// skipped and never followed. Folders are listed with a trailing `/`. Unreadable folders are
/// skipped.
#[must_use]
pub fn list_paths(root: &Root, max_depth: usize, max_entries: usize) -> Vec<String> {
    use rustix::fs::{AtFlags, FileType, Mode, OFlags};
    let mut out = Vec::new();
    let Ok(top) = rustix::io::dup(root.fd()) else {
        return out;
    };
    let mut stack: Vec<(std::os::fd::OwnedFd, String, usize)> = vec![(top, String::new(), 0)];
    while let Some((fd, prefix, depth)) = stack.pop() {
        let Ok(mut d) = rustix::fs::Dir::read_from(&fd) else {
            continue;
        };
        let mut names: Vec<String> = Vec::new();
        while let Some(Ok(e)) = d.read() {
            let n = e.file_name().to_string_lossy().into_owned();
            if n.starts_with('.') || n.contains('/') {
                continue;
            }
            names.push(n);
        }
        names.sort();
        let mut subdirs = Vec::new();
        for n in names {
            if out.len() >= max_entries {
                return out;
            }
            let Ok(st) = rustix::fs::statat(&fd, n.as_str(), AtFlags::SYMLINK_NOFOLLOW) else {
                continue;
            };
            match FileType::from_raw_mode(super::stat_mode(&st)) {
                FileType::RegularFile => out.push(format!("{prefix}{n}")),
                FileType::Directory => {
                    out.push(format!("{prefix}{n}/"));
                    if depth + 1 < max_depth && !SKIP_DIRS.contains(&n.as_str()) {
                        let flags =
                            OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::RDONLY;
                        if let Ok(sub) = rustix::fs::openat(&fd, n.as_str(), flags, Mode::empty()) {
                            subdirs.push((sub, format!("{prefix}{n}/"), depth + 1));
                        }
                    }
                }
                _ => {}
            }
        }
        // Reverse so the first folder (sorted) is listed next.
        stack.extend(subdirs.into_iter().rev());
    }
    out
}
