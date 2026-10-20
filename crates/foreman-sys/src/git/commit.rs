//! The plumbing commit with read-back (SR-42, SR-43, ADR-012 decisions 3–5, DR-142–DR-145). Only
//! the given paths, only if they changed since the tip, from bytes Foreman holds (re-read through
//! the store on the current branch); a temporary index; `commit-tree --no-gpg-sign`;
//! `update-ref <ref> <new> <old>`; read-back by hash; then, on the current branch only, the real
//! index for exactly those paths (retried on `index.lock`). Another branch never touches HEAD,
//! the index or the working tree.

use std::path::{Path, PathBuf};
use std::time::Duration;

use foreman_core::git::{self as core_git, GitOp, ReadTreeFrom, RevParse, ZERO_OID};
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

/// One plumbing step, reported as it happens (`Channel<SignProgress>`, ADR-012).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitStep<'a> {
    /// `hash-object -w --no-filters` of a path.
    HashObject(&'a str),
    /// `write-tree` (after `read-tree` and `update-index` on the temporary index).
    WriteTree,
    /// `commit-tree --no-gpg-sign`.
    CommitTree,
    /// `update-ref`.
    UpdateRef,
    /// The real-index refresh (current branch only).
    RefreshIndex,
    /// A path read back from the commit, and whether it matched.
    ReadBack(&'a str, bool),
}

/// A read-back test seam: the path and the committed bytes in, the bytes compared out.
pub type ReadbackSeam = dyn Fn(&str, Vec<u8>) -> Vec<u8>;

/// Where the commit lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// The branch HEAD names (SR-42).
    Current,
    /// Another branch, `refs/heads/<name>` (SR-43). A branch that doesn't exist yet starts at
    /// HEAD's commit.
    Other(String),
}

/// Test seams, progress and app-data facts.
#[derive(Default)]
#[allow(missing_debug_implementations, reason = "holds callbacks")]
pub struct CommitOpts<'a> {
    /// Foreman's app data (the temporary index goes to `<app_data>/tmp/`).
    pub app_data: PathBuf,
    /// Called between `commit-tree` and `update-ref` (tests move the ref here).
    pub before_update_ref: Option<&'a dyn Fn()>,
    /// Retries and their spacing for the real-index refresh (5 × 200 ms).
    pub index_retries: Option<(u32, Duration)>,
    /// Told of each plumbing step.
    pub on_step: Option<&'a dyn Fn(CommitStep<'_>)>,
    /// A test seam: changes the bytes of a path between `hash-object` and read-back's
    /// comparison, as a repo hook rewriting the packet would (AC-F014-10).
    pub tamper_readback: Option<&'a ReadbackSeam>,
}

impl CommitOpts<'_> {
    fn step(&self, s: CommitStep<'_>) {
        if let Some(f) = self.on_step {
            f(s);
        }
    }
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
    /// The branch name isn't valid, or is the branch HEAD names (sign on the current branch).
    #[error("{0}")]
    BadBranch(String),
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
    verify_readback_with(repo, commit, expected, &CommitOpts::default(), ctx)
}

fn verify_readback_with(
    repo: &Path,
    commit: &str,
    expected: &[(String, String)],
    opts: &CommitOpts<'_>,
    ctx: &SpawnCtx,
) -> Result<(), CommitError> {
    for (path, sha) in expected {
        let mut blob = show::blob_at(repo, commit, path, ctx)?;
        if let Some(f) = opts.tamper_readback {
            blob = blob.map(|b| f(path, b));
        }
        let ok = blob.as_deref().map(sha256_hex).as_deref() == Some(sha.as_str());
        opts.step(CommitStep::ReadBack(path, ok));
        if !ok {
            return Err(CommitError::ReadbackMismatch {
                commit: commit.to_owned(),
                path: path.clone(),
            });
        }
    }
    Ok(())
}

/// Commits exactly `paths` on the current branch (SR-42), if they changed since HEAD. Each path
/// is re-read through the store and must still hash to what was reviewed.
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
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for r in paths {
        let (bytes, _) = project.read(&r.path, ReadClass::Decision)?;
        if sha256_hex(&bytes) != r.sha256 {
            return Err(CommitError::ChangedSinceReview {
                path: r.path.to_string(),
            });
        }
        files.push((r.path.to_string(), bytes));
    }
    commit_bytes(project.path(), &files, message, &Target::Current, opts, ctx)
}

/// The tip a target commits onto: `(ref to update, its current OID, the parent to use)`.
fn tip_of(
    repo: &Path,
    target: &Target,
    ctx: &SpawnCtx,
) -> Result<(String, Option<String>, Option<String>), CommitError> {
    let g = GitOpts::default();
    let sym = run(repo, &GitOp::SymbolicRefHead, &g, ctx)?;
    let head_branch = sym.ok().then(|| sym.text());
    match target {
        Target::Current => {
            // HEAD state first (current branch only).
            let Some(branch) = head_branch else {
                return Err(CommitError::DetachedHead);
            };
            let tip = show::head(repo, ctx)?;
            Ok((branch, tip.clone(), tip))
        }
        Target::Other(b) => {
            let full =
                core_git::branch_ref(b).map_err(|e| CommitError::BadBranch(e.to_string()))?;
            if head_branch.as_deref() == Some(full.as_str()) {
                return Err(CommitError::BadBranch(
                    "That's the branch you're on. Sign on the current branch instead.".into(),
                ));
            }
            let out = run(
                repo,
                &GitOp::RevParse(RevParse::VerifyBranch(full.clone())),
                &g,
                ctx,
            )?;
            if out.ok() {
                let tip = out.text();
                Ok((full, Some(tip.clone()), Some(tip)))
            } else {
                // A new branch starts at HEAD's commit (or empty in an unborn repo).
                Ok((full, None, show::head(repo, ctx)?))
            }
        }
    }
}

/// Commits `files` (path, bytes Foreman holds) onto `target` if they differ from its tip.
///
/// # Errors
/// See [`CommitError`].
#[allow(
    clippy::too_many_lines,
    reason = "ADR-012 decision 4's steps in order, kept together for review"
)]
pub fn commit_bytes(
    repo: &Path,
    files: &[(String, Vec<u8>)],
    message: &str,
    target: &Target,
    opts: &CommitOpts<'_>,
    ctx: &SpawnCtx,
) -> Result<CommitOutcome, CommitError> {
    let repo = repo.to_path_buf();
    let g = GitOpts::default();
    let (refname, old, parent) = tip_of(&repo, target, ctx)?;
    let current = *target == Target::Current;

    // DR-142 against the commit we build on: keep only paths that changed.
    let mut changed: Vec<(String, &[u8])> = Vec::new();
    for (p, bytes) in files {
        let working = show::hash_object(&repo, bytes, false, ctx)?;
        let at = match &parent {
            Some(tip) => show::blob_oid(&repo, tip, p, ctx)?,
            None => None,
        };
        if at.as_deref() != Some(working.as_str()) {
            changed.push((p.clone(), bytes.as_slice()));
        }
    }
    if changed.is_empty() {
        return Ok(CommitOutcome::NothingToCommit);
    }
    let mut entries = Vec::new();
    for (p, bytes) in &changed {
        let oid = show::hash_object(&repo, bytes, true, ctx)?;
        opts.step(CommitStep::HashObject(p));
        entries.push((oid, p.clone()));
    }
    let index = temp_index(&opts.app_data)?;
    let with_index = GitOpts {
        index_file: Some(index.clone()),
        ..GitOpts::default()
    };
    let result = (|| -> Result<String, CommitError> {
        let from = parent
            .clone()
            .map_or(ReadTreeFrom::Empty, ReadTreeFrom::Tip);
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
        opts.step(CommitStep::WriteTree);
        let commit = run_ok(
            &repo,
            &GitOp::CommitTree {
                tree,
                parent: parent.clone(),
                msg: message.to_owned(),
            },
            &g,
            ctx,
        )?;
        opts.step(CommitStep::CommitTree);
        if let Some(f) = opts.before_update_ref {
            f();
        }
        let upd = run(
            &repo,
            &GitOp::UpdateRef {
                refname: if current {
                    "HEAD".into()
                } else {
                    refname.clone()
                },
                new: commit.clone(),
                old: old.clone().unwrap_or_else(|| ZERO_OID.to_owned()),
            },
            &g,
            ctx,
        )?;
        if !upd.ok() {
            return Err(CommitError::ConcurrentRefMove);
        }
        opts.step(CommitStep::UpdateRef);
        Ok(commit)
    })();
    let _ = rustix::fs::unlink(&index);
    let commit = result?;

    let expected: Vec<(String, String)> = changed
        .iter()
        .map(|(p, b)| (p.clone(), sha256_hex(b)))
        .collect();
    let paths: Vec<String> = changed.into_iter().map(|(p, _)| p).collect();
    if !current {
        verify_readback_with(&repo, &commit, &expected, opts, ctx)?;
        return Ok(CommitOutcome::Committed {
            commit,
            branch: refname,
            paths,
            first: parent.is_none(),
            index_refresh_pending: false,
        });
    }
    // Read-back before the real index is touched: a mismatch leaves Billy's index alone.
    verify_readback_with(&repo, &commit, &expected, opts, ctx)?;
    let refreshed = refresh_index(&repo, &entries, opts, ctx)?;
    Ok(CommitOutcome::Committed {
        commit,
        branch: refname,
        paths,
        first: parent.is_none(),
        index_refresh_pending: !refreshed,
    })
}

/// The real index, exactly those paths, retried on `index.lock`. Attributes are neutralized
/// (T-040). `$GIT_DIR/info/attributes` can't be neutralized that way, so a repo that has one
/// keeps the refresh pending (the banner tells Billy) rather than risk running a filter.
fn refresh_index(
    repo: &Path,
    entries: &[(String, String)],
    opts: &CommitOpts<'_>,
    ctx: &SpawnCtx,
) -> Result<bool, CommitError> {
    let g = GitOpts::default();
    let (tries, gap) = opts
        .index_retries
        .unwrap_or((5, Duration::from_millis(200)));
    let mut refreshed = false;
    let info_attr = run_ok(
        repo,
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
            repo,
            &GitOp::UpdateIndexCacheInfo {
                entries: entries.to_vec(),
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
    if refreshed {
        opts.step(CommitStep::RefreshIndex);
    }
    Ok(refreshed)
}

/// Retries a pending real-index refresh (`indexRefreshPending`: on project open and window
/// focus). Returns true once it succeeded. Only paths whose working bytes still hash to the
/// committed blob are refreshed; a path Billy changed since is left alone.
///
/// # Errors
/// git failed to run.
pub fn retry_index_refresh(
    project: &Root,
    commit: &str,
    paths: &[String],
    opts: &CommitOpts<'_>,
    ctx: &SpawnCtx,
) -> Result<bool, CommitError> {
    let repo = project.path().to_path_buf();
    let mut entries = Vec::new();
    for p in paths {
        let Some(oid) = show::blob_oid(&repo, commit, p, ctx)? else {
            continue;
        };
        entries.push((oid, p.clone()));
    }
    if entries.is_empty() {
        return Ok(true);
    }
    refresh_index(&repo, &entries, opts, ctx)
}
