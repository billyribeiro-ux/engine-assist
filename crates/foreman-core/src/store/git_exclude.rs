//! The `info/exclude` writer (SR-32, ADR-010 §4, AC-F020-13): the only write under `.git`. It
//! appends `.*.foreman-tmp` once to the path `git rev-parse --git-path info/exclude` reported,
//! refusing symlinks and any path outside the project root or the repo's common git directory.

use std::path::{Component, Path, PathBuf};

use super::{ReadStamp, Root, RootKind, StoreError};
use crate::pathpolicy::{self, PolicyViolation, ReadClass};

/// The line appended.
pub const PATTERN: &str = ".*.foreman-tmp";

/// What `ensure` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ensured {
    /// The line was already there.
    AlreadyPresent,
    /// The line was appended (file created if needed).
    Appended,
}

/// Why the exclude path was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExcludeError {
    /// Outside the project root and the common git directory.
    #[error("{0} is outside the project and its git directory")]
    OutsideScope(String),
    /// Not a normalized absolute path.
    #[error("{0} is not a plain path")]
    NotPlain(String),
    /// A store failure (symlink, conflict, …).
    #[error(transparent)]
    Store(#[from] StoreError),
}

fn plain(p: &Path) -> bool {
    p.is_absolute() && p.components().all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
}

/// Appends [`PATTERN`] to `exclude_path` (from `--git-path info/exclude`; relative paths are
/// relative to the project root) if missing. `common_dir` is `--git-common-dir`'s result.
///
/// # Errors
/// See [`ExcludeError`].
pub fn ensure(project: &Root, exclude_path: &Path, common_dir: &Path) -> Result<Ensured, ExcludeError> {
    let abs: PathBuf = if exclude_path.is_absolute() {
        exclude_path.to_path_buf()
    } else {
        project.path().join(exclude_path)
    };
    let common: PathBuf = if common_dir.is_absolute() {
        common_dir.to_path_buf()
    } else {
        project.path().join(common_dir)
    };
    let shown = abs.display().to_string();
    if !plain(&abs) || !plain(&common) {
        return Err(ExcludeError::NotPlain(shown));
    }
    // The common dir must itself be the project's `.git` or a folder git reports; its ancestors
    // may be symlinks Billy chose, so compare canonical bases.
    let (base, rel) = if let Ok(r) = abs.strip_prefix(project.path()) {
        (project.path().to_path_buf(), r.to_path_buf())
    } else {
        let canon_common = super::canonical(&common);
        if let Ok(r) = abs.strip_prefix(&canon_common).or_else(|_| abs.strip_prefix(&common)) {
            (canon_common, r.to_path_buf())
        } else {
            return Err(ExcludeError::OutsideScope(shown));
        }
    };
    let rel = rel.to_str().ok_or_else(|| ExcludeError::NotPlain(shown.clone()))?.to_owned();
    // Syntax only: this purpose is exempt from the `.git` deny rule by its type (SR-32).
    let rp = pathpolicy::check_app_data(&rel).map_err(|e| ExcludeError::Store(e.into()))?;
    if !rel.ends_with("info/exclude") {
        return Err(ExcludeError::OutsideScope(shown));
    }
    let root = Root::open(&base, RootKind::GitDir)?;
    let (bytes, stamp) = root.read_opt(&rp, ReadClass::Decision)?;
    let mut text = bytes.map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    if text.lines().any(|l| l.trim() == PATTERN) {
        return Ok(Ensured::AlreadyPresent);
    }
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(PATTERN);
    text.push('\n');
    match root.write(&rp, text.as_bytes(), &stamp) {
        Ok(_) => Ok(Ensured::Appended),
        Err(StoreError::Policy(PolicyViolation::Symlink(c))) => Err(StoreError::Policy(PolicyViolation::Symlink(c)).into()),
        Err(e) => Err(e.into()),
    }
}

/// Used by `ensure` when nothing exists yet.
#[must_use]
pub fn absent() -> ReadStamp {
    ReadStamp::Absent
}
