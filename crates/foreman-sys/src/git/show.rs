//! Blueprint reads for the lock (F-002, ADR-012 decision 6, DR-142, DR-156): blobs at a commit
//! (raw, no textconv or filters) and the dirty check by hash, never `git status`.

use std::path::Path;

use foreman_core::git::{GitOp, RevParse};

use super::{GitError, GitOpts, run, run_ok};
use crate::spawn::SpawnCtx;

/// The blob of `path` at `rev`, raw (`cat-file blob <rev>:<path>`). `None` when the commit or path
/// doesn't exist.
///
/// # Errors
/// When git can't run or the repo isn't its own.
pub fn blob_at(
    repo: &Path,
    rev: &str,
    path: &str,
    ctx: &SpawnCtx,
) -> Result<Option<Vec<u8>>, GitError> {
    let out = run(
        repo,
        &GitOp::CatFileBlob {
            rev: rev.into(),
            path: path.into(),
        },
        &GitOpts::default(),
        ctx,
    )?;
    Ok(out.ok().then_some(out.stdout))
}

/// The OID of `path` at `rev` (`rev-parse <rev>:<path>`), `None` when missing.
///
/// # Errors
/// When git can't run.
pub fn blob_oid(
    repo: &Path,
    rev: &str,
    path: &str,
    ctx: &SpawnCtx,
) -> Result<Option<String>, GitError> {
    let out = run(
        repo,
        &GitOp::RevParse(RevParse::BlobAt {
            rev: rev.into(),
            path: path.into(),
        }),
        &GitOpts::default(),
        ctx,
    )?;
    Ok(out.ok().then(|| out.text()))
}

/// `hash-object [-w] --no-filters --stdin` of bytes Foreman holds.
///
/// # Errors
/// When git fails.
pub fn hash_object(
    repo: &Path,
    bytes: &[u8],
    write: bool,
    ctx: &SpawnCtx,
) -> Result<String, GitError> {
    run_ok(
        repo,
        &GitOp::HashObjectNoFilters { write },
        &GitOpts {
            stdin: Some(bytes.to_vec()),
            ..GitOpts::default()
        },
        ctx,
    )
}

/// DR-142: "changed since HEAD" is `hash-object --no-filters` of the bytes ≠ `rev-parse HEAD:<path>`
/// (missing at HEAD counts as changed). Returns `(working, head)` hashes.
///
/// # Errors
/// When git fails.
pub fn changed_since_head(
    repo: &Path,
    path: &str,
    bytes: &[u8],
    ctx: &SpawnCtx,
) -> Result<(bool, String, Option<String>), GitError> {
    let working = hash_object(repo, bytes, false, ctx)?;
    let head = blob_oid(repo, "HEAD", path, ctx)?;
    Ok((head.as_deref() != Some(working.as_str()), working, head))
}

/// `rev-parse HEAD` (`None` on an unborn branch).
///
/// # Errors
/// When git fails to run.
pub fn head(repo: &Path, ctx: &SpawnCtx) -> Result<Option<String>, GitError> {
    let out = run(
        repo,
        &GitOp::RevParse(RevParse::VerifyHead),
        &GitOpts::default(),
        ctx,
    )?;
    Ok(out.ok().then(|| out.text()))
}
