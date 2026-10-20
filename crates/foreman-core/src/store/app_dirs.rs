//! App-data folders the runners manage (SR-53): listing (sensor events, ADR-009) and resetting
//! the probe folder (SR-17: recreated `0700` with only Foreman's files before each probe run).
//! Every path is app-data-relative and walked through the root's fd without following symlinks.

use rustix::fs::{AtFlags, Mode, OFlags};

use super::{Root, RootKind, StoreError};

/// The entry names in an app-data folder (sensor events), sorted. Missing → empty.
///
/// # Errors
/// [`StoreError`] when the folder is a symlink or unreadable.
pub fn list_app_data_dir(root: &Root, rel: &str) -> Result<Vec<String>, StoreError> {
    let p = crate::pathpolicy::check_app_data(rel)?;
    let comps: Vec<&str> = p.components().collect();
    let dir = match super::walk::walk(root.fd(), &comps, root.opts(false, None)) {
        Ok(d) => d,
        Err(StoreError::NotFound(_)) => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    let mut d = rustix::fs::Dir::read_from(&dir).map_err(|e| StoreError::io(rel, e))?;
    let mut out = Vec::new();
    while let Some(e) = d.read() {
        let e = e.map_err(|e| StoreError::io(rel, e))?;
        let n = e.file_name().to_string_lossy().into_owned();
        if n != "." && n != ".." {
            out.push(n);
        }
    }
    out.sort();
    Ok(out)
}

/// Empties `rel` (creating it `0700` if missing) without following any symlink: entries are
/// removed with `unlinkat`, sub-folders recursively through their own fds. Only for app data.
///
/// # Errors
/// [`StoreError`] when the root isn't app data or a removal fails.
pub fn reset_dir(root: &Root, rel: &str) -> Result<(), StoreError> {
    if root.kind() != RootKind::AppData {
        return Err(StoreError::NotFound(format!("{rel}: not app data")));
    }
    let p = crate::pathpolicy::check_app_data(rel)?;
    let comps: Vec<&str> = p.components().collect();
    let dir = super::walk::walk(root.fd(), &comps, root.opts(true, None))?;
    empty(&dir, rel, 0)?;
    rustix::fs::fchmod(&dir, Mode::from_raw_mode(0o700)).map_err(|e| StoreError::io(rel, e))
}

fn empty(dir: &std::os::fd::OwnedFd, rel: &str, depth: usize) -> Result<(), StoreError> {
    if depth > 16 {
        return Err(StoreError::NotFound(format!("{rel}: nested too deeply")));
    }
    let mut names = Vec::new();
    let mut d = rustix::fs::Dir::read_from(dir).map_err(|e| StoreError::io(rel, e))?;
    while let Some(e) = d.read() {
        let e = e.map_err(|e| StoreError::io(rel, e))?;
        let n = e.file_name().to_owned();
        if n.to_bytes() != b"." && n.to_bytes() != b".." {
            names.push(n);
        }
    }
    for n in names {
        let st = rustix::fs::statat(dir, n.as_c_str(), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|e| StoreError::io(rel, e))?;
        let is_dir = rustix::fs::FileType::from_raw_mode(super::stat_mode(&st))
            == rustix::fs::FileType::Directory;
        if is_dir {
            let sub = rustix::fs::openat(
                dir,
                n.as_c_str(),
                OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::RDONLY,
                Mode::empty(),
            )
            .map_err(|e| StoreError::io(rel, e))?;
            empty(&sub, rel, depth + 1)?;
            rustix::fs::unlinkat(dir, n.as_c_str(), AtFlags::REMOVEDIR)
                .map_err(|e| StoreError::io(rel, e))?;
        } else {
            rustix::fs::unlinkat(dir, n.as_c_str(), AtFlags::empty())
                .map_err(|e| StoreError::io(rel, e))?;
        }
    }
    Ok(())
}
