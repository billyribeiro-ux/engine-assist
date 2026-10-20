//! The plumbing commit with read-back (SR-42, ADR-012 decisions 3–5, DR-142–DR-145). Only the given
//! paths, only if they changed since HEAD, from bytes Foreman re-read through the store; a
//! temporary index; `commit-tree --no-gpg-sign`; `update-ref <ref> <new> <old>`; read-back by
//! hash; then the real index for exactly those paths (retried on `index.lock`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use foreman_core::git::{GitOp, ReadTreeFrom, RevParse, ZERO_OID};
use foreman_core::pathpolicy::{ReadClass, RepoPath};
use foreman_core::store::{Root, StoreError, sha256_hex};

use super::{GitError, GitOpts, run, run_ok, show};
use crate::spawn::SpawnCtx;

/// One path to commit, with the hash Billy reviewed (or Foreman just wrote).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reviewed {
    /// Repo path.
    pub path: RepoPath,
    /// SHA-256 of the reviewed bytes.
    pub sha256: String,
}

/// Test seams and app-data facts.
#[derive(Default)]
#[allow(missing_debug_implementations, reason = "holds a test callback")]
pub struct CommitOpts<'a> {
    /// Foreman's app data (the temporary index goes to `<app_data>/tmp/`).
    pub app_data: PathBuf,
    /// Called between `commit-tree` and `update-ref` (tests move the ref here).
    pub before_update_ref: Option<&'a dyn Fn()>,
    /// Retries and their spacing for the real-index refresh (5 × 200 ms).
    pub index_retries: Option<(u32, Duration)>,
}

/// What the commit did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitOutcome {
    /// Nothing differed from HEAD.
    NothingToCommit,
    /// A new commit on the current branch.
    Committed {
        /// The commit OID (stored in `progress.json`, never in the commit itself).
        commit: String,
        /// The branch ref.
        branch: String,
        /// The paths committed.
        paths: Vec<String>,
        /// The repo's first commit (unborn HEAD).
        first: bool,
        /// The real-index refresh failed after the retries (`indexRefreshPending`).
        index_refresh_pending: bool,
    },
}

/// Why nothing was committed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommitError {
    /// `symbolic-ref -q HEAD` exited 1.
    #[error("Check out a branch first")]
    DetachedHead,
    /// The file changed since it was reviewed.
    #[error("{path} changed since you reviewed it")]
    ChangedSinceReview {
        /// Path.
        path: String,
    },
    /// The ref moved between `commit-tree` and `update-ref`; nothing was overwritten.
    #[error("the branch moved while Foreman was committing; nothing was overwritten")]
    ConcurrentRefMove,
    /// A committed blob doesn't hash to what Foreman wrote (`commitPending`).
    #[error("Your repo's commit hooks changed {path}")]
    ReadbackMismatch {
        /// The commit made.
        commit: String,
        /// The path.
        path: String,
    },
    /// git failed.
    #[error(transparent)]
    Git(#[from] GitError),
    /// The store refused a re-read.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The temporary index couldn't be created.
    #[error("temporary index: {0}")]
    TempIndex(String),
}

fn temp_index(app_data: &Path) -> Result<PathBuf, CommitError> {
    let dir = app_data.join("tmp");
    match rustix::fs::mkdir(&dir, rustix::fs::Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
        Err(e) => return Err(CommitError::TempIndex(e.to_string())),
    }
    Ok(dir.join(format!("git-index-{}", uuid::Uuid::new_v4())))
}

/// Verifies each committed path hashes to the expected SHA-256 (`cat-file blob <commit>:<path>`).
///
/// # Errors
/// [`CommitError::ReadbackMismatch`] naming the first mismatch.
pub fn verify_readback(
    repo: &Path,
    commit: &str,
    expected: &[(String, String)],
    ctx: &SpawnCtx,
) -> Result<(), CommitError> {
    for (path, sha) in expected {
        let blob = show::blob_at(repo, commit, path, ctx)?;
        if blob.as_deref().map(sha256_hex).as_deref() != Some(sha.as_str()) {
            return Err(CommitError::ReadbackMismatch {
                commit: commit.to_owned(),
                path: path.clone(),
            });
        }
    }
    Ok(())
}

/// Commits exactly `paths` on the current branch (SR-42), if they changed since HEAD.
///
/// # Errors
/// See [`CommitError`].
pub fn commit_paths(
    project: &Root,
    paths: &[Reviewed],
    message: &str,
    opts: &CommitOpts<'_>,
    ctx: &SpawnCtx,
) -> Result<CommitOutcome, CommitError> {
    let repo = project.path().to_path_buf();
    let g = GitOpts::default();
    // HEAD state first (current branch only).
    let sym = run(&repo, &GitOp::SymbolicRefHead, &g, ctx)?;
    if !sym.ok() {
        return Err(CommitError::DetachedHead);
    }
    let branch = sym.text();
    let tip = show::head(&repo, ctx)?;

    // Re-read through the store and compare with what was reviewed; keep only changed paths.
    let mut changed: Vec<(String, Vec<u8>)> = Vec::new();
    for r in paths {
        let (bytes, _) = project.read(&r.path, ReadClass::Decision)?;
        if sha256_hex(&bytes) != r.sha256 {
            return Err(CommitError::ChangedSinceReview {
                path: r.path.to_string(),
            });
        }
        let (dirty, _, _) = show::changed_since_head(&repo, r.path.as_str(), &bytes, ctx)?;
        if dirty {
            changed.push((r.path.to_string(), bytes));
        }
    }
    if changed.is_empty() {
        return Ok(CommitOutcome::NothingToCommit);
    }
    let mut entries = Vec::new();
    for (p, bytes) in &changed {
        let oid = show::hash_object(&repo, bytes, true, ctx)?;
        entries.push((oid, p.clone()));
    }
    let index = temp_index(&opts.app_data)?;
    let with_index = GitOpts {
        index_file: Some(index.clone()),
        ..GitOpts::default()
    };
    let result = (|| -> Result<String, CommitError> {
        let from = tip.clone().map_or(ReadTreeFrom::Empty, ReadTreeFrom::Tip);
        run_ok(&repo, &GitOp::ReadTree(from), &with_index, ctx)?;
        run_ok(
            &repo,
            &GitOp::UpdateIndexCacheInfo {
                entries: entries.clone(),
            },
            &with_index,
            ctx,
        )?;
        let tree = run_ok(&repo, &GitOp::WriteTree, &with_index, ctx)?;
        let commit = run_ok(
            &repo,
            &GitOp::CommitTree {
                tree,
                parent: tip.clone(),
                msg: message.to_owned(),
            },
            &g,
            ctx,
        )?;
        if let Some(f) = opts.before_update_ref {
            f();
        }
        let upd = run(
            &repo,
            &GitOp::UpdateRef {
                refname: "HEAD".into(),
                new: commit.clone(),
                old: tip.clone().unwrap_or_else(|| ZERO_OID.to_owned()),
            },
            &g,
            ctx,
        )?;
        if !upd.ok() {
            return Err(CommitError::ConcurrentRefMove);
        }
        Ok(commit)
    })();
    let _ = rustix::fs::unlink(&index);
    let commit = result?;

    let expected: Vec<(String, String)> = changed
        .iter()
        .map(|(p, b)| (p.clone(), sha256_hex(b)))
        .collect();
    verify_readback(&repo, &commit, &expected, ctx)?;

    // The real index, exactly those paths, retried on index.lock. Attributes are neutralized
    // (T-040). `$GIT_DIR/info/attributes` can't be neutralized that way, so a repo that has one
    // keeps the refresh pending (the banner tells Billy) rather than risk running a filter.
    let (tries, gap) = opts
        .index_retries
        .unwrap_or((5, Duration::from_millis(200)));
    let mut refreshed = false;
    let info_attr = run_ok(
        &repo,
        &GitOp::RevParse(RevParse::GitPath("info/attributes".into())),
        &g,
        ctx,
    )?;
    let info_attr_path = if Path::new(&info_attr).is_absolute() {
        PathBuf::from(&info_attr)
    } else {
        repo.join(&info_attr)
    };
    let has_info_attributes = rustix::fs::stat(&info_attr_path).is_ok_and(|st| st.st_size > 0);
    let refresh = GitOpts {
        neutralize_attributes: true,
        ..GitOpts::default()
    };
    for i in 0..if has_info_attributes { 0 } else { tries.max(1) } {
        let out = run(
            &repo,
            &GitOp::UpdateIndexCacheInfo {
                entries: entries.clone(),
            },
            &refresh,
            ctx,
        )?;
        if out.ok() {
            refreshed = true;
            break;
        }
        if i + 1 < tries {
            std::thread::sleep(gap);
        }
    }
    Ok(CommitOutcome::Committed {
        commit,
        branch,
        paths: changed.into_iter().map(|(p, _)| p).collect(),
        first: tip.is_none(),
        index_refresh_pending: !refreshed,
    })
}
