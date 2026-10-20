//! Foreman's own decision commits before Stage 5 is signed (DR-153, AC-F020-08): only the
//! decision file, by pathspec, through the signing plumbing and read-back (SR-42).

use foreman_core::decisions::{self, OwnChange};
use foreman_core::pathpolicy::{self, WritePurpose};
use foreman_core::store::Root;

use crate::git::commit::{CommitError, CommitOpts, CommitOutcome, Reviewed, commit_paths};
use crate::spawn::SpawnCtx;

/// The commit message for a decision file (`foreman: .foreman/project.json`).
#[must_use]
pub fn message(path: &str) -> String {
    format!("foreman: {path}")
}

/// Commits a [`OwnChange::CommitNow`]. A pending change (from Stage 5 on) commits nothing and
/// returns `Ok(None)`. On an error (detached HEAD, a changed file …) nothing is committed and the
/// change stays pending: the approvals' `committedSha256` is only updated by the caller on success.
///
/// # Errors
/// [`CommitError`].
pub fn commit_own(
    project: &Root,
    change: &OwnChange,
    opts: &CommitOpts<'_>,
    ctx: &SpawnCtx,
) -> Result<Option<CommitOutcome>, CommitError> {
    let OwnChange::CommitNow { file, path, sha256 } = change else {
        return Ok(None);
    };
    let d = decisions::decision_path(*file);
    let rel = pathpolicy::check_write(&d.path(), WritePurpose::DecisionFile(d))
        .map_err(|e| CommitError::Store(e.into()))?;
    commit_paths(
        project,
        &[Reviewed {
            path: rel,
            sha256: sha256.clone(),
        }],
        &message(path),
        opts,
        ctx,
    )
    .map(Some)
}
