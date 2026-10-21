//! App-data folders the runners manage (SR-53): listing (sensor events, ADR-009), removing
//! (Clear run history, project removal, tail pruning; M4-02) and resetting
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

/// Opens the folder that holds `rel`'s last component, without creating anything. `None` when a
/// parent doesn't exist.
fn parent_of(
    root: &Root,
    rel: &crate::pathpolicy::RepoPath,
) -> Result<Option<(std::os::fd::OwnedFd, String)>, StoreError> {
    if root.kind() != RootKind::AppData {
        return Err(StoreError::NotFound(format!("{rel}: not app data")));
    }
    let (parents, name) = rel.split_last();
    match super::walk::walk(root.fd(), &parents, root.opts(false, None)) {
        Ok(dir) => Ok(Some((dir, name.to_owned()))),
        Err(StoreError::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Removes one app-data file or symlink (the link itself, never its target). Missing is fine.
/// Returns whether something was removed.
///
/// # Errors
/// [`StoreError`] when the path is refused, names a folder, or the removal fails.
pub fn remove_file(root: &Root, rel: &str) -> Result<bool, StoreError> {
    let p = crate::pathpolicy::check_app_data(rel)?;
    let Some((dir, name)) = parent_of(root, &p)? else {
        return Ok(false);
    };
    match rustix::fs::statat(&dir, name.as_str(), AtFlags::SYMLINK_NOFOLLOW) {
        Err(e) if e == rustix::io::Errno::NOENT => return Ok(false),
        Err(e) => return Err(StoreError::io(rel, e)),
        Ok(st) => {
            if rustix::fs::FileType::from_raw_mode(super::stat_mode(&st))
                == rustix::fs::FileType::Directory
            {
                return Err(StoreError::NotRegular(rel.to_owned()));
            }
        }
    }
    rustix::fs::unlinkat(&dir, name.as_str(), AtFlags::empty())
        .map_err(|e| StoreError::io(rel, e))?;
    Ok(true)
}

/// Removes an app-data folder and everything in it, without following any symlink (a link inside
/// is unlinked, not entered). Missing is fine. Returns whether something was removed.
///
/// # Errors
/// [`StoreError`] when the path is refused, is not a folder, or a removal fails.
pub fn remove_tree(root: &Root, rel: &str) -> Result<bool, StoreError> {
    let p = crate::pathpolicy::check_app_data(rel)?;
    let Some((dir, name)) = parent_of(root, &p)? else {
        return Ok(false);
    };
    match rustix::fs::statat(&dir, name.as_str(), AtFlags::SYMLINK_NOFOLLOW) {
        Err(e) if e == rustix::io::Errno::NOENT => return Ok(false),
        Err(e) => return Err(StoreError::io(rel, e)),
        Ok(st) => match rustix::fs::FileType::from_raw_mode(super::stat_mode(&st)) {
            rustix::fs::FileType::Directory => {}
            rustix::fs::FileType::Symlink => {
                rustix::fs::unlinkat(&dir, name.as_str(), AtFlags::empty())
                    .map_err(|e| StoreError::io(rel, e))?;
                return Ok(true);
            }
            _ => return Err(StoreError::NotRegular(rel.to_owned())),
        },
    }
    let sub = rustix::fs::openat(
        &dir,
        name.as_str(),
        OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::RDONLY,
        Mode::empty(),
    )
    .map_err(|e| StoreError::io(rel, e))?;
    empty(&sub, rel, 0)?;
    drop(sub);
    rustix::fs::unlinkat(&dir, name.as_str(), AtFlags::REMOVEDIR)
        .map_err(|e| StoreError::io(rel, e))?;
    Ok(true)
}

/// The total size in bytes of the regular files under an app-data folder (links are not
/// followed). Missing is zero.
///
/// # Errors
/// [`StoreError`] when the path is refused or a folder can't be read.
pub fn tree_size(root: &Root, rel: &str) -> Result<u64, StoreError> {
    let p = crate::pathpolicy::check_app_data(rel)?;
    let Some((dir, name)) = parent_of(root, &p)? else {
        return Ok(0);
    };
    let sub = match rustix::fs::openat(
        &dir,
        name.as_str(),
        OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::RDONLY,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(e) if e == rustix::io::Errno::NOENT => return Ok(0),
        Err(e) => return Err(StoreError::io(rel, e)),
    };
    size_of(&sub, rel, 0)
}

fn size_of(dir: &std::os::fd::OwnedFd, rel: &str, depth: usize) -> Result<u64, StoreError> {
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
    let mut total = 0_u64;
    for n in names {
        let st = rustix::fs::statat(dir, n.as_c_str(), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|e| StoreError::io(rel, e))?;
        match rustix::fs::FileType::from_raw_mode(super::stat_mode(&st)) {
            rustix::fs::FileType::Directory => {
                let sub = rustix::fs::openat(
                    dir,
                    n.as_c_str(),
                    OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::RDONLY,
                    Mode::empty(),
                )
                .map_err(|e| StoreError::io(rel, e))?;
                total = total.saturating_add(size_of(&sub, rel, depth + 1)?);
            }
            rustix::fs::FileType::RegularFile => {
                total = total.saturating_add(u64::try_from(st.st_size).unwrap_or(0));
            }
            _ => {}
        }
    }
    Ok(total)
}

/// The size in bytes of one app-data regular file (never following a link). Missing is `None`.
///
/// # Errors
/// [`StoreError`] when the path is refused, is not a regular file, or can't be read.
pub fn file_len(root: &Root, rel: &str) -> Result<Option<u64>, StoreError> {
    let p = crate::pathpolicy::check_app_data(rel)?;
    let Some((dir, name)) = parent_of(root, &p)? else {
        return Ok(None);
    };
    match rustix::fs::statat(&dir, name.as_str(), AtFlags::SYMLINK_NOFOLLOW) {
        Err(e) if e == rustix::io::Errno::NOENT => Ok(None),
        Err(e) => Err(StoreError::io(rel, e)),
        Ok(st) => {
            if rustix::fs::FileType::from_raw_mode(super::stat_mode(&st))
                != rustix::fs::FileType::RegularFile
            {
                return Err(StoreError::NotRegular(rel.to_owned()));
            }
            Ok(Some(u64::try_from(st.st_size).unwrap_or(0)))
        }
    }
}

/// When an app-data regular file was last modified, in seconds since the epoch (never following a
/// link). Missing is `None`.
///
/// # Errors
/// [`StoreError`] when the path is refused, is not a regular file, or can't be read.
pub fn modified_secs(root: &Root, rel: &str) -> Result<Option<u64>, StoreError> {
    let p = crate::pathpolicy::check_app_data(rel)?;
    let Some((dir, name)) = parent_of(root, &p)? else {
        return Ok(None);
    };
    match rustix::fs::statat(&dir, name.as_str(), AtFlags::SYMLINK_NOFOLLOW) {
        Err(e) if e == rustix::io::Errno::NOENT => Ok(None),
        Err(e) => Err(StoreError::io(rel, e)),
        Ok(st) => {
            if rustix::fs::FileType::from_raw_mode(super::stat_mode(&st))
                != rustix::fs::FileType::RegularFile
            {
                return Err(StoreError::NotRegular(rel.to_owned()));
            }
            Ok(Some(super::stat_mtime_secs(&st)))
        }
    }
}

/// Sets the access and modification times of one app-data regular file to `secs` (seconds since
/// the epoch), through the no-symlink walk and without following a link. Only the times change;
/// the content and mode stay. Retention uses it as "last used" (a template a project still pins is
/// refreshed at each startup scan, so 90 days of no refresh means 90 days unused; files.md row 20).
/// Missing is fine. Returns whether a file was touched.
///
/// # Errors
/// [`StoreError`] when the path is refused, is not a regular file, or the call fails.
pub fn touch(root: &Root, rel: &str, secs: u64) -> Result<bool, StoreError> {
    let p = crate::pathpolicy::check_app_data(rel)?;
    let Some((dir, name)) = parent_of(root, &p)? else {
        return Ok(false);
    };
    match rustix::fs::statat(&dir, name.as_str(), AtFlags::SYMLINK_NOFOLLOW) {
        Err(e) if e == rustix::io::Errno::NOENT => return Ok(false),
        Err(e) => return Err(StoreError::io(rel, e)),
        Ok(st) => {
            if rustix::fs::FileType::from_raw_mode(super::stat_mode(&st))
                != rustix::fs::FileType::RegularFile
            {
                return Err(StoreError::NotRegular(rel.to_owned()));
            }
        }
    }
    let t = rustix::fs::Timespec {
        tv_sec: i64::try_from(secs).unwrap_or(i64::MAX),
        tv_nsec: 0,
    };
    let times = rustix::fs::Timestamps {
        last_access: t,
        last_modification: t,
    };
    rustix::fs::utimensat(&dir, name.as_str(), &times, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|e| StoreError::io(rel, e))?;
    Ok(true)
}

/// Size rotation for an app-data log (ADR-018 §6, files.md §2 row 25: `name`, `name.1` …
/// `name.<keep>`): `name.<keep-1>` replaces `name.<keep>`, and so on down, then `name` becomes
/// `name.1`. Renames happen inside the folder reached by the no-symlink walk; a missing file is
/// skipped. The next append creates `name` afresh.
///
/// # Errors
/// [`StoreError`] when the path is refused or a rename fails.
pub fn rotate(root: &Root, rel: &str, keep: u32) -> Result<(), StoreError> {
    let p = crate::pathpolicy::check_app_data(rel)?;
    for i in 1..=keep {
        crate::pathpolicy::check_app_data(&format!("{rel}.{i}"))?;
    }
    let Some((dir, name)) = parent_of(root, &p)? else {
        return Ok(());
    };
    let step = |from: &str, to: &str| match rustix::fs::renameat(&dir, from, &dir, to) {
        Ok(()) => Ok(()),
        Err(e) if e == rustix::io::Errno::NOENT => Ok(()),
        Err(e) => Err(StoreError::io(rel, e)),
    };
    for i in (1..keep).rev() {
        step(&format!("{name}.{i}"), &format!("{name}.{}", i + 1))?;
    }
    if keep > 0 {
        step(&name, &format!("{name}.1"))?;
    }
    Ok(())
}

/// Installs an executable into app data (ADR-009: `bin/foreman-sensor-<sha256-12>`, files.md §2
/// row 24, mode `0700`): whatever is at `rel` (a file or a symlink, never followed) is removed,
/// `bytes` are written through the atomic writer (`O_EXCL`, never replacing), and the new file is
/// set to `0700` through an fd opened without following links. Only for app data.
///
/// # Errors
/// [`StoreError`] when the path is refused, names a folder, or a step fails.
pub fn install_executable(root: &Root, rel: &str, bytes: &[u8]) -> Result<(), StoreError> {
    if root.kind() != RootKind::AppData {
        return Err(StoreError::NotFound(format!("{rel}: not app data")));
    }
    let p = crate::pathpolicy::check_app_data(rel)?;
    remove_file(root, rel)?;
    root.write(&p, bytes, &super::ReadStamp::Absent)?;
    let Some((dir, name)) = parent_of(root, &p)? else {
        return Err(StoreError::NotFound(rel.to_owned()));
    };
    let fd = rustix::fs::openat(
        &dir,
        name.as_str(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|e| StoreError::io(rel, e))?;
    rustix::fs::fchmod(&fd, Mode::from_raw_mode(0o700)).map_err(|e| StoreError::io(rel, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    #[test]
    fn rotate_keeps_the_newest_files() {
        let d = tempfile::tempdir().unwrap();
        let app = d.path().join("app");
        let root = Root::open_app_data(&app).unwrap();
        let rel = crate::pathpolicy::check_app_data("logs/x.log").unwrap();
        crate::store::dirs::ensure_dir(&root, &crate::pathpolicy::check_app_data("logs").unwrap())
            .unwrap();
        assert_eq!(file_len(&root, "logs/x.log").unwrap(), None);
        for n in 0..6_u8 {
            root.append_line(&rel, &[b'a' + n, b'\n']).unwrap();
            assert_eq!(file_len(&root, "logs/x.log").unwrap(), Some(2));
            rotate(&root, "logs/x.log", 4).unwrap();
            assert_eq!(file_len(&root, "logs/x.log").unwrap(), None);
        }
        // Six generations, four kept: the newest is .1.
        for (i, want) in [(1, "f"), (2, "e"), (3, "d"), (4, "c")] {
            assert_eq!(
                std::fs::read_to_string(app.join(format!("logs/x.log.{i}"))).unwrap(),
                format!("{want}\n")
            );
        }
        assert!(!app.join("logs/x.log.5").exists());
        // A symlink in place of the log is not a regular file.
        std::os::unix::fs::symlink(d.path(), app.join("logs/x.log")).unwrap();
        assert!(file_len(&root, "logs/x.log").is_err());
    }

    #[test]
    fn install_executable_is_0700_and_replaces_links() {
        let d = tempfile::tempdir().unwrap();
        let app = d.path().join("app");
        let root = Root::open_app_data(&app).unwrap();
        install_executable(&root, "bin/tool-0123456789ab", b"one").unwrap();
        let f = app.join("bin/tool-0123456789ab");
        assert_eq!(std::fs::read(&f).unwrap(), b"one");
        assert_eq!(
            std::fs::metadata(&f).unwrap().permissions().mode() & 0o777,
            0o700
        );
        // A planted symlink is replaced, its target untouched.
        let victim = d.path().join("victim");
        std::fs::write(&victim, b"keep").unwrap();
        std::fs::remove_file(&f).unwrap();
        std::os::unix::fs::symlink(&victim, &f).unwrap();
        install_executable(&root, "bin/tool-0123456789ab", b"two").unwrap();
        assert_eq!(std::fs::read(&victim).unwrap(), b"keep");
        assert!(!std::fs::symlink_metadata(&f).unwrap().is_symlink());
        assert_eq!(std::fs::read(&f).unwrap(), b"two");
        // A folder in the way is refused.
        std::fs::create_dir_all(app.join("bin/dir-0123456789ab")).unwrap();
        assert!(install_executable(&root, "bin/dir-0123456789ab", b"x").is_err());
    }
}
