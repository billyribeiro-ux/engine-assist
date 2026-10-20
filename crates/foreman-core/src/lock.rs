//! The blueprint lock (F-002; DR-155–DR-157; files.md §3.3): pinning, re-pin window, restoring the
//! pinned text from app data or the blueprint repo, and per-item drift. Pure apart from the store;
//! the git read is passed in by `foreman-sys`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::ItemId;
use crate::pathpolicy::{self, DecisionPath, ReadClass, WritePurpose};
use crate::schema::approvals::ItemKindConfirm;
use crate::schema::lock::{LockFile, LockedItem};
use crate::store::{Root, StoreError};
use crate::template::{Template, parse_template, source_sha256};

/// The app-data copy of the pinned text (files.md row 20).
#[must_use]
pub fn text_path(source_sha256: &str) -> String {
    format!("templates/{source_sha256}.md")
}

/// Why pinning or opening fails.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LockError {
    /// AC-F002-03.
    #[error("Stage 1 is signed, so the pin can't change: upgrading the pin is v1.1")]
    RepinClosed,
    /// AC-F002-02 (DR-142).
    #[error("{path} differs from HEAD (working {working} vs HEAD {}). Commit or discard the change, then pin", head.as_deref().unwrap_or("missing"))]
    Dirty {
        /// Blueprint path.
        path: String,
        /// `hash-object --no-filters` of the working file.
        working: String,
        /// `rev-parse HEAD:<path>`.
        head: Option<String>,
    },
    /// Store failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Why a project opens read-only (DR-156).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReadOnly {
    /// AC-F002-05.
    #[error(
        "The pinned blueprint's text doesn't match the lock ({where_}: {found} vs lock {expected}). The project is read-only"
    )]
    HashMismatch {
        /// `app data` or `git`.
        where_: String,
        /// The hash found.
        found: String,
        /// The lock's hash.
        expected: String,
    },
    /// AC-F002-06.
    #[error(
        "Blueprint {version} at {commit} not found. Point Foreman at your blueprint repo or re-pin."
    )]
    NotFound {
        /// Blueprint version.
        version: String,
        /// Commit.
        commit: String,
    },
    /// AC-F002-07.
    #[error(
        "git isn't installed or can't be found, so the pinned blueprint can't be restored. Install git (Doctor shows where Foreman looks)"
    )]
    GitMissing,
    /// The app-data copy couldn't be read or written.
    #[error("the pinned blueprint copy couldn't be read: {0}")]
    Store(String),
}

/// The result of the injected git read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    /// The blob.
    Blob(Vec<u8>),
    /// Repo, commit or path missing.
    NotFound,
    /// No git.
    GitMissing,
}

/// The lock for a parsed template (DR-155). `commit` is the blueprint repo's HEAD.
#[must_use]
pub fn lock_for(t: &Template, commit: &str, path: &str, pinned_at: &str) -> LockFile {
    let items = t
        .items
        .iter()
        .filter_map(|i| {
            let id = ItemId::parse(i.id.as_deref()?).ok()?;
            Some((
                id,
                LockedItem {
                    text_sha256: i.text_sha256.clone(),
                    kind: i.kind.clone(),
                },
            ))
        })
        .collect();
    LockFile {
        schema_version: 1,
        blueprint_repo_commit: commit.to_owned(),
        blueprint_path: path.to_owned(),
        source_sha256: t.source_sha256.clone(),
        blueprint_version: t.blueprint_version.clone(),
        grammar_version: t.grammar_version.unwrap_or(0),
        pinned_at: pinned_at.to_owned(),
        items,
    }
}

/// AC-F002-03: re-pinning only before Stage 1 is signed.
///
/// # Errors
/// [`LockError::RepinClosed`].
pub fn check_repin(existing: Option<&LockFile>, stage1_signed: bool) -> Result<(), LockError> {
    if existing.is_some() && stage1_signed {
        return Err(LockError::RepinClosed);
    }
    Ok(())
}

/// What the blueprint repo says about the file being pinned (from `foreman-sys`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlueprintState {
    /// The file's bytes as read.
    pub bytes: Vec<u8>,
    /// HEAD commit.
    pub commit: String,
    /// `hash-object --no-filters` of `bytes`.
    pub working_oid: String,
    /// `rev-parse HEAD:<path>`.
    pub head_oid: Option<String>,
}

/// Pins (DR-155): refuses a dirty file; stores the text in app data at
/// `templates/<sourceSha256>.md` and writes `.foreman/blueprint.lock.json` in the project. Returns
/// the lock.
///
/// # Errors
/// [`LockError`].
pub fn pin(
    project: &Root,
    app: &Root,
    path: &str,
    state: &BlueprintState,
    stage1_signed: bool,
    pinned_at: &str,
) -> Result<LockFile, LockError> {
    let lock_rel = pathpolicy::check_write(
        &DecisionPath::Lock.path(),
        WritePurpose::DecisionFile(DecisionPath::Lock),
    )
    .map_err(StoreError::from)?;
    let (existing, stamp) = project.read_opt(&lock_rel, ReadClass::Decision)?;
    let existing = existing.and_then(|b| crate::json::parse::<LockFile>(&b).ok());
    check_repin(existing.as_ref(), stage1_signed)?;
    if state.head_oid.as_deref() != Some(state.working_oid.as_str()) {
        return Err(LockError::Dirty {
            path: path.to_owned(),
            working: state.working_oid.clone(),
            head: state.head_oid.clone(),
        });
    }
    let text = String::from_utf8_lossy(&state.bytes);
    let t = parse_template(&text);
    store_text(app, &t.source_sha256, &state.bytes)?;
    let lock = lock_for(&t, &state.commit, path, pinned_at);
    let json = crate::json::to_canonical(&lock).map_err(|e| StoreError::Io {
        name: lock_rel.to_string(),
        message: e.to_string(),
        errno: 0,
    })?;
    project.write(&lock_rel, json.as_bytes(), &stamp)?;
    Ok(lock)
}

fn store_text(app: &Root, sha: &str, bytes: &[u8]) -> Result<(), StoreError> {
    let rel = pathpolicy::check_app_data(&text_path(sha))?;
    let (_, stamp) = app.read_opt(&rel, ReadClass::Template)?;
    match app.write(&rel, bytes, &stamp) {
        Ok(_) | Err(StoreError::Conflict { .. }) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Where the restored text came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restored {
    /// App data.
    AppData,
    /// The blueprint repo (and now stored in app data).
    Git,
}

/// DR-156: the pinned text, from app data by hash, else from `fetch` (`cat-file blob
/// <commit>:<path>` in the blueprint repo), re-hashed and stored. Any failure is a read-only
/// reason.
///
/// # Errors
/// [`ReadOnly`].
pub fn restore(
    app: &Root,
    lock: &LockFile,
    fetch: impl FnOnce() -> Fetched,
) -> Result<(String, Restored), ReadOnly> {
    let rel = pathpolicy::check_app_data(&text_path(&lock.source_sha256))
        .map_err(|e| ReadOnly::Store(e.to_string()))?;
    let cached = app
        .read_opt(&rel, ReadClass::Template)
        .map_err(|e| ReadOnly::Store(e.to_string()))?;
    let mismatch = |where_: &str, found: String| ReadOnly::HashMismatch {
        where_: where_.into(),
        found,
        expected: lock.source_sha256.clone(),
    };
    if let (Some(bytes), _) = cached {
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let sha = source_sha256(&text);
        return if sha == lock.source_sha256 {
            Ok((text, Restored::AppData))
        } else {
            Err(mismatch("app data", sha))
        };
    }
    let bytes = match fetch() {
        Fetched::Blob(b) => b,
        Fetched::NotFound => {
            return Err(ReadOnly::NotFound {
                version: lock.blueprint_version.clone(),
                commit: lock.blueprint_repo_commit.clone(),
            });
        }
        Fetched::GitMissing => return Err(ReadOnly::GitMissing),
    };
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let sha = source_sha256(&text);
    if sha != lock.source_sha256 {
        return Err(mismatch("git", sha));
    }
    store_text(app, &sha, &bytes).map_err(|e| ReadOnly::Store(e.to_string()))?;
    Ok((text, Restored::Git))
}

/// One drifted item (DR-157).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Drift {
    /// Item.
    pub id: ItemId,
    /// The lock's hash (`None`: new since the pin).
    pub locked: Option<String>,
    /// The current hash.
    pub current: String,
    /// The current kind.
    pub kind: String,
}

/// DR-157: items whose current `textSha256` differs from the lock and hasn't been confirmed for
/// that hash. Attesting them is blocked until Billy confirms the kind.
#[must_use]
pub fn drift(
    t: &Template,
    lock: &LockFile,
    confirmed: &BTreeMap<ItemId, ItemKindConfirm>,
) -> Vec<Drift> {
    t.items
        .iter()
        .filter_map(|i| {
            let id = ItemId::parse(i.id.as_deref()?).ok()?;
            let locked = lock.items.get(&id);
            if locked.is_some_and(|l| l.text_sha256 == i.text_sha256) {
                return None;
            }
            if confirmed
                .get(&id)
                .is_some_and(|c| c.text_sha256 == i.text_sha256 && c.kind == i.kind)
            {
                return None;
            }
            Some(Drift {
                id,
                locked: locked.map(|l| l.text_sha256.clone()),
                current: i.text_sha256.clone(),
                kind: i.kind.clone(),
            })
        })
        .collect()
}
