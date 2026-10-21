//! The git side of the blueprint lock (DR-155, DR-156): the blueprint's state for pinning and the
//! pinned blob for restoring, both through `git::run` plumbing.

use std::path::Path;

use foreman_core::lock::{BlueprintState, Fetched};
use foreman_core::pathpolicy::{self, ReadClass};
use foreman_core::schema::lock::LockFile;
use foreman_core::store::{Root, RootKind, StoreError};
use thiserror::Error;

use crate::git::{GitError, show};
use crate::spawn::{SpawnCtx, SpawnError};

/// Why the blueprint's state can't be read.
#[derive(Debug, Error)]
pub enum StateError {
    /// Reading the file.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// git.
    #[error(transparent)]
    Git(#[from] GitError),
    /// The repo has no commit.
    #[error("the blueprint repo has no commit yet")]
    NoCommit,
}

/// Reads the blueprint file (through the store, as a blueprint root) and asks git for HEAD and
/// both hashes (DR-142).
///
/// # Errors
/// [`StateError`].
pub fn blueprint_state(
    repo: &Path,
    path: &str,
    ctx: &SpawnCtx,
) -> Result<BlueprintState, StateError> {
    let root = Root::open(repo, RootKind::Blueprint)?;
    let rel = pathpolicy::check_read(path, ReadClass::Template).map_err(StoreError::from)?;
    let (bytes, _) = root.read(&rel, ReadClass::Template)?;
    let commit = show::head(repo, ctx)?.ok_or(StateError::NoCommit)?;
    let (_, working_oid, head_oid) = show::changed_since_head(repo, path, &bytes, ctx)?;
    Ok(BlueprintState {
        bytes,
        commit,
        working_oid,
        head_oid,
    })
}

/// True when the error means git itself is missing (not executable, not allowlisted).
#[must_use]
pub fn git_missing(e: &GitError) -> bool {
    matches!(
        e,
        GitError::Spawn(
            SpawnError::Unlisted(_) | SpawnError::NotExecutable(_) | SpawnError::RelativePath(_)
        )
    )
}

/// The pinned blob (`cat-file blob <commit>:<path>`) for [`foreman_core::lock::restore`]. A missing
/// repo, commit or path is [`Fetched::NotFound`]; a missing git is [`Fetched::GitMissing`].
#[must_use]
pub fn fetch(repo: &Path, lock: &LockFile, ctx: &SpawnCtx) -> Fetched {
    if !repo.is_dir() {
        return Fetched::NotFound;
    }
    match show::blob_at(repo, &lock.blueprint_repo_commit, &lock.blueprint_path, ctx) {
        Ok(Some(b)) => Fetched::Blob(b),
        Err(e) if git_missing(&e) => Fetched::GitMissing,
        Ok(None) | Err(_) => Fetched::NotFound,
    }
}
