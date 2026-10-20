//! The component-wise directory walk (ADR-011 layer 2): every intermediate component is opened
//! with `O_DIRECTORY | O_NOFOLLOW` relative to the previous directory fd, so a symlink at any
//! component is refused, and the name the file system reports for each entry is checked against
//! the deny rules again (catching an existing `.Git` reached through other case on a
//! case-insensitive volume).

use std::os::fd::{AsFd, OwnedFd};

use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags};
use rustix::io::Errno;

use crate::pathpolicy::{self, PolicyViolation};

use super::StoreError;

/// Maps the name the file system reported to the name checked. The identity in production; tests
/// substitute a case-insensitive volume's behavior on file systems that can't fold case.
pub type NameReporter = fn(&str) -> String;

/// Which rules the reported name of each entry is checked against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameCheck {
    /// Foreman's own folders (app data, the exclude writer's git dir).
    None,
    /// Deny rules and secret patterns (project and blueprint roots).
    Deny,
    /// Secret patterns only (the fixed outside-read list).
    SecretsOnly,
}

/// Checks a reported name with the given rules.
///
/// # Errors
/// The violated rule.
pub fn check_name(name: &str, exempt: Option<&str>, mode: NameCheck) -> Result<(), PolicyViolation> {
    match mode {
        NameCheck::None => Ok(()),
        NameCheck::Deny => check_reported(name, exempt),
        NameCheck::SecretsOnly => {
            if pathpolicy::is_secret(name) {
                Err(PolicyViolation::Secret(name.to_owned()))
            } else {
                Ok(())
            }
        }
    }
}

/// How the walk treats missing directories and reported names.
#[derive(Clone, Copy)]
pub(crate) struct WalkOpts<'a> {
    /// Create missing directories with this mode.
    pub create: Option<u32>,
    /// Exemption for the deny rule on the reported name (`.claude`, `.foreman`).
    pub exempt: Option<&'a str>,
    /// Which checks apply to reported names.
    pub names: NameCheck,
    /// The reporter hook.
    pub reporter: NameReporter,
}

pub(crate) fn errno_is_symlink(e: Errno) -> bool {
    e == Errno::LOOP || e == Errno::NOTDIR || e == Errno::MLINK
}

/// The on-disk name of the entry with inode `ino` in `dir`, as the file system reports it.
pub(crate) fn reported_name(dir: &OwnedFd, ino: u64) -> Option<String> {
    let mut d = Dir::read_from(dir.as_fd()).ok()?;
    while let Some(Ok(entry)) = d.read() {
        if entry.ino() == ino {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name != "." && name != ".." {
                return Some(name);
            }
        }
    }
    None
}

/// Checks a reported name: denied components and names (with the purpose's exemption) and secret
/// patterns.
///
/// # Errors
/// The violated rule.
pub fn check_reported(name: &str, exempt: Option<&str>) -> Result<(), PolicyViolation> {
    pathpolicy::check_component(name, exempt)?;
    if pathpolicy::is_secret(name) {
        return Err(PolicyViolation::Secret(name.to_owned()));
    }
    Ok(())
}

#[allow(clippy::useless_conversion, reason = "st_ino's type differs per platform")]
fn ino_of(fd: &OwnedFd) -> Result<u64, Errno> {
    let st = rustix::fs::fstat(fd)?;
    Ok(u64::try_from(st.st_ino).unwrap_or(0))
}

/// Opens one directory component under `parent`, refusing symlinks.
pub(crate) fn open_dir(parent: &OwnedFd, comp: &str, opts: WalkOpts<'_>) -> Result<OwnedFd, StoreError> {
    let flags = OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::RDONLY;
    let fd = match rustix::fs::openat(parent, comp, flags, Mode::empty()) {
        Ok(fd) => fd,
        Err(e) if e == Errno::NOENT => {
            let Some(mode) = opts.create else {
                return Err(StoreError::NotFound(comp.to_owned()));
            };
            match rustix::fs::mkdirat(parent, comp, Mode::from_raw_mode(mode)) {
                Ok(()) | Err(Errno::EXIST) => {}
                Err(e) => return Err(StoreError::io(comp, e)),
            }
            rustix::fs::openat(parent, comp, flags, Mode::empty()).map_err(|e| classify(parent, comp, e))?
        }
        Err(e) => return Err(classify(parent, comp, e)),
    };
    if opts.names != NameCheck::None {
        let ino = ino_of(&fd).map_err(|e| StoreError::io(comp, e))?;
        if let Some(name) = reported_name(parent, ino) {
            check_name(&(opts.reporter)(&name), opts.exempt, opts.names).map_err(StoreError::Policy)?;
        }
    }
    Ok(fd)
}

/// Turns an `openat` error into a symlink refusal when the entry is a symlink.
pub(crate) fn classify(parent: &OwnedFd, comp: &str, e: Errno) -> StoreError {
    if (errno_is_symlink(e) || e == Errno::NOTDIR)
        && let Ok(st) = rustix::fs::statat(parent, comp, AtFlags::SYMLINK_NOFOLLOW)
        && FileType::from_raw_mode(super::stat_mode(&st)) == FileType::Symlink
    {
        return StoreError::Policy(PolicyViolation::Symlink(comp.to_owned()));
    }
    if e == Errno::NOENT {
        return StoreError::NotFound(comp.to_owned());
    }
    StoreError::io(comp, e)
}

/// Walks the parent components of a path from `root`.
pub(crate) fn walk(root: &OwnedFd, comps: &[&str], opts: WalkOpts<'_>) -> Result<OwnedFd, StoreError> {
    let mut cur = rustix::io::dup(root).map_err(|e| StoreError::io(".", e))?;
    for c in comps {
        cur = open_dir(&cur, c, opts)?;
    }
    Ok(cur)
}
