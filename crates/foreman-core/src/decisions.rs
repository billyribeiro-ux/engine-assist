//! Decision files and approved copies (F-020, F-022; ADR-022; DR-150–DR-153).
//!
//! Foreman's effective configuration is the approved copy in `approvals/<projectId>/<file>.json`,
//! never the repo file. An outside change is classified: variables only → accepted automatically;
//! anything else → a pending change with a redacted diff; unparsable or newer → read-only.
//! Foreman's own changes go to the approved copy first, then (before Stage 5 is signed) to the
//! repo file, which `foreman-sys` commits at once; from Stage 5 on they stay pending.

use serde_json::Value;
use thiserror::Error;

use crate::pathpolicy::{self, DecisionPath, ReadClass, WritePurpose};
use crate::schema::approvals::{Approval, ApprovalsFile};
use crate::schema::lock::LockFile;
use crate::schema::overlay::OverlayFile;
use crate::schema::progress::{
    ChangeKind, DecisionFileKind, DiffEntry, PendingOutside, PendingOwn,
};
use crate::schema::project::ProjectFile;
use crate::schema::{self, Loaded};
use crate::store::{ReadStamp, Root, StoreError, sha256_hex};

/// `approvals/<projectId>.json`.
#[must_use]
pub fn approvals_path(project_id: &str) -> String {
    format!("approvals/{project_id}.json")
}

/// `approvals/<projectId>/<file>.json`.
#[must_use]
pub fn approved_path(project_id: &str, kind: DecisionFileKind) -> String {
    format!("approvals/{project_id}/{}", kind.approved_name())
}

/// The path-policy decision path of a decision file.
#[must_use]
pub fn decision_path(kind: DecisionFileKind) -> DecisionPath {
    match kind {
        DecisionFileKind::Project => DecisionPath::Project,
        DecisionFileKind::Overlay => DecisionPath::Overlay,
        DecisionFileKind::BlueprintLock => DecisionPath::Lock,
    }
}

fn entry(a: &mut ApprovalsFile, kind: DecisionFileKind) -> &mut Approval {
    match kind {
        DecisionFileKind::Project => &mut a.files.project,
        DecisionFileKind::Overlay => &mut a.files.overlay,
        DecisionFileKind::BlueprintLock => &mut a.files.blueprint_lock,
    }
}

/// Errors.
#[derive(Debug, Error)]
pub enum DecisionError {
    /// Store failure (including a CAS conflict).
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The approved copy or approvals file is unreadable.
    #[error("{0}")]
    Corrupt(String),
    /// The repo file has an outside change waiting for review: Foreman doesn't write over it.
    #[error("{0} has a change from outside Foreman waiting for review. Accept or reject it first")]
    OutsidePending(String),
    /// No approved copy yet (setup writes one; a lost app data needs the one-time review).
    #[error("{0} has no approved copy yet")]
    NoApprovedCopy(String),
}

/// The classification of a repo file against its approved copy (DR-151).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classified {
    /// Same content (or Foreman's own pending change, DR-153).
    Unchanged,
    /// Only `variables`, `chatProjectUrl` or `chatUrls` differ.
    VariablesOnly,
    /// Decision fields differ: a pending outside change with this redacted diff.
    Pending(Vec<DiffEntry>),
    /// Unparsable or newer schema: the project is read-only with this reason.
    ReadOnly(String),
}

fn typed_ok(kind: DecisionFileKind, bytes: &[u8]) -> Result<(), String> {
    fn check<T: schema::Versioned>(bytes: &[u8]) -> Result<(), String> {
        match schema::load::<T>(bytes) {
            Ok(Loaded::NewerReadOnly(h)) => Err(schema::newer_message(h.schema_version)),
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
    match kind {
        DecisionFileKind::Project => check::<ProjectFile>(bytes),
        DecisionFileKind::Overlay => check::<OverlayFile>(bytes),
        DecisionFileKind::BlueprintLock => check::<LockFile>(bytes),
    }
}

const SECRET_KEY_PARTS: [&str; 4] = ["token", "secret", "password", "apikey"];

fn is_secret_key(k: &str) -> bool {
    let l = k.to_ascii_lowercase().replace(['_', '-'], "");
    SECRET_KEY_PARTS.iter().any(|p| l.contains(p))
}

/// SR-55 redaction for diff display: secret-looking keys and anything under `env`/`headers`.
fn shown(path: &[String], v: &Value) -> String {
    let hidden = path
        .iter()
        .any(|k| k == "env" || k == "headers" || is_secret_key(k));
    let text = crate::json::value_to_canonical(v).trim_end().to_owned();
    if hidden {
        let h = sha256_hex(text.as_bytes());
        format!("‹redacted sha256:{}›", &h[..8])
    } else {
        text
    }
}

fn diff_into(
    path: &mut Vec<String>,
    a: Option<&Value>,
    b: Option<&Value>,
    out: &mut Vec<DiffEntry>,
) {
    match (a, b) {
        (Some(Value::Object(x)), Some(Value::Object(y))) => {
            let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                path.push(k.clone());
                diff_into(path, x.get(k), y.get(k), out);
                path.pop();
            }
        }
        (Some(x), Some(y)) if x == y => {}
        (x, y) => {
            let change = match (x, y) {
                (None, _) => ChangeKind::Added,
                (_, None) => ChangeKind::Removed,
                _ => ChangeKind::ValueChanged,
            };
            out.push(DiffEntry {
                key_path: path.join("."),
                change,
                before: x.map(|v| shown(path, v)),
                after: y.map(|v| shown(path, v)),
            });
        }
    }
}

/// The redacted key-level diff from `approved` to `on_disk`.
#[must_use]
pub fn diff(approved: &Value, on_disk: &Value) -> Vec<DiffEntry> {
    let mut out = Vec::new();
    diff_into(&mut Vec::new(), Some(approved), Some(on_disk), &mut out);
    out
}

/// DR-151 (`core::decisions::classify_outside_change`).
#[must_use]
pub fn classify_outside_change(
    kind: DecisionFileKind,
    approved: &[u8],
    on_disk: &[u8],
) -> Classified {
    if approved == on_disk {
        return Classified::Unchanged;
    }
    if let Err(reason) = typed_ok(kind, on_disk) {
        return Classified::ReadOnly(format!("{}: {reason}", kind.repo_path()));
    }
    let (Ok(a), Ok(b)) = (
        crate::json::parse_value(approved),
        crate::json::parse_value(on_disk),
    ) else {
        return Classified::ReadOnly(format!(
            "{}: the approved copy is unreadable",
            kind.repo_path()
        ));
    };
    let d = diff(&a, &b);
    if d.is_empty() {
        return Classified::Unchanged;
    }
    let variables_only = kind == DecisionFileKind::Project
        && d.iter().all(|e| {
            let top = e.key_path.split('.').next().unwrap_or_default();
            ProjectFile::VARIABLE_FIELDS.contains(&top)
        });
    if variables_only {
        Classified::VariablesOnly
    } else {
        Classified::Pending(d)
    }
}

/// What processing one decision file on load did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Processed {
    /// Nothing to do.
    Unchanged,
    /// Variables reloaded into the approved copy (toast "Variables reloaded").
    VariablesReloaded,
    /// A pending outside change; the effective config stays the approved copy.
    Pending(PendingOutside),
    /// Read-only with the reason; nothing is overwritten.
    ReadOnly(String),
}

/// The decision store of one project.
#[derive(Debug)]
pub struct Decisions<'a> {
    /// The project root.
    pub project: &'a Root,
    /// App data.
    pub app: &'a Root,
    /// Project ID.
    pub project_id: &'a str,
}

/// How a Foreman change landed (DR-153).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnChange {
    /// Before Stage 5 is signed: the repo file is written; commit it now by pathspec.
    CommitNow {
        /// Which file.
        file: DecisionFileKind,
        /// Repo path.
        path: String,
        /// SHA-256 of the bytes written (the commit's reviewed hash).
        sha256: String,
    },
    /// From Stage 5 on: held for the next signing commit.
    Pending(PendingOwn),
}

impl Decisions<'_> {
    fn app_rel(rel: &str) -> Result<pathpolicy::RepoPath, DecisionError> {
        Ok(pathpolicy::check_app_data(rel).map_err(StoreError::from)?)
    }

    fn repo_rel(kind: DecisionFileKind) -> Result<pathpolicy::RepoPath, DecisionError> {
        let d = decision_path(kind);
        Ok(
            pathpolicy::check_write(&d.path(), WritePurpose::DecisionFile(d))
                .map_err(StoreError::from)?,
        )
    }

    /// The approvals file (a fresh one when absent).
    ///
    /// # Errors
    /// Store or parse failure.
    pub fn approvals(&self) -> Result<(ApprovalsFile, ReadStamp), DecisionError> {
        let rel = Self::app_rel(&approvals_path(self.project_id))?;
        let (bytes, stamp) = self.app.read_opt(&rel, ReadClass::AppState)?;
        let a = match bytes {
            None => ApprovalsFile::new(self.project_id),
            Some(b) => schema::load::<ApprovalsFile>(&b)
                .ok()
                .and_then(Loaded::value)
                .ok_or_else(|| DecisionError::Corrupt(format!("{rel} is unreadable")))?,
        };
        Ok((a, stamp))
    }

    fn save_approvals(&self, a: &ApprovalsFile, stamp: &ReadStamp) -> Result<(), DecisionError> {
        let rel = Self::app_rel(&approvals_path(self.project_id))?;
        let text = schema::to_file(a).map_err(|e| DecisionError::Corrupt(e.to_string()))?;
        self.app.write(&rel, text.as_bytes(), stamp)?;
        Ok(())
    }

    /// The approved copy's bytes: Foreman's effective configuration (DR-150).
    ///
    /// # Errors
    /// [`DecisionError::NoApprovedCopy`], store failure.
    pub fn approved(&self, kind: DecisionFileKind) -> Result<(Vec<u8>, ReadStamp), DecisionError> {
        let rel = Self::app_rel(&approved_path(self.project_id, kind))?;
        match self.app.read_opt(&rel, ReadClass::Decision)? {
            (Some(b), s) => Ok((b, s)),
            (None, _) => Err(DecisionError::NoApprovedCopy(kind.repo_path().into())),
        }
    }

    /// Why the approved copy of `kind` can't be used (a schema newer than this Foreman, or
    /// unparsable), when it exists but can't (AC-F018-07, AC-F030-03): the project is read-only.
    #[must_use]
    pub fn approved_problem(&self, kind: DecisionFileKind) -> Option<String> {
        let (bytes, _) = self.approved(kind).ok()?;
        typed_ok(kind, &bytes)
            .err()
            .map(|why| format!("{}: {why}", kind.repo_path()))
    }

    /// The effective project file.
    ///
    /// # Errors
    /// As [`Self::approved`], or an unreadable copy.
    pub fn effective_project(&self) -> Result<ProjectFile, DecisionError> {
        self.effective::<ProjectFile>(DecisionFileKind::Project)
    }

    /// The effective overlay.
    ///
    /// # Errors
    /// As [`Self::approved`].
    pub fn effective_overlay(&self) -> Result<OverlayFile, DecisionError> {
        self.effective::<OverlayFile>(DecisionFileKind::Overlay)
    }

    /// The effective lock.
    ///
    /// # Errors
    /// As [`Self::approved`].
    pub fn effective_lock(&self) -> Result<LockFile, DecisionError> {
        self.effective::<LockFile>(DecisionFileKind::BlueprintLock)
    }

    fn effective<T: schema::Versioned>(&self, kind: DecisionFileKind) -> Result<T, DecisionError> {
        let (b, _) = self.approved(kind)?;
        schema::load::<T>(&b)
            .ok()
            .and_then(Loaded::value)
            .ok_or_else(|| {
                DecisionError::Corrupt(format!(
                    "the approved {} is unreadable",
                    kind.approved_name()
                ))
            })
    }

    /// Records the repo bytes as approved and committed (setup, accept, or a commit landing).
    ///
    /// # Errors
    /// Store failure.
    pub fn approve(
        &self,
        kind: DecisionFileKind,
        bytes: &[u8],
        committed: bool,
    ) -> Result<(), DecisionError> {
        let rel = Self::app_rel(&approved_path(self.project_id, kind))?;
        let (_, stamp) = self.app.read_opt(&rel, ReadClass::Decision)?;
        self.app.write(&rel, bytes, &stamp)?;
        let (mut a, s) = self.approvals()?;
        let sha = sha256_hex(bytes);
        let e = entry(&mut a, kind);
        e.approved_sha256 = Some(sha.clone());
        if committed {
            e.committed_sha256 = Some(sha);
        }
        self.save_approvals(&a, &s)
    }

    /// Marks the committed hash after a commit (DR-153, ADR-022 §5).
    ///
    /// # Errors
    /// Store failure.
    pub fn mark_committed(
        &self,
        kind: DecisionFileKind,
        sha256: &str,
    ) -> Result<(), DecisionError> {
        let (mut a, s) = self.approvals()?;
        entry(&mut a, kind).committed_sha256 = Some(sha256.to_owned());
        self.save_approvals(&a, &s)
    }

    /// Records what a signing on another branch committed for a decision file (SR-43, ADR-022 §5):
    /// `committedOnOtherBranches[<ref>] = sha256`.
    ///
    /// # Errors
    /// Store failure.
    pub fn mark_committed_on_branch(
        &self,
        kind: DecisionFileKind,
        branch_ref: &str,
        sha256: &str,
    ) -> Result<(), DecisionError> {
        let (mut a, s) = self.approvals()?;
        entry(&mut a, kind)
            .committed_on_other_branches
            .insert(branch_ref.to_owned(), sha256.to_owned());
        self.save_approvals(&a, &s)
    }

    /// Re-reads the repo file and classifies it (ADR-022 §2). Variables-only changes are accepted
    /// here; nothing is ever written to the repo file.
    ///
    /// # Errors
    /// Store failure, or no approved copy.
    pub fn process(
        &self,
        kind: DecisionFileKind,
        id: &str,
        now: &str,
    ) -> Result<Processed, DecisionError> {
        self.process_on(kind, id, now, None)
    }

    /// [`Self::process`] knowing the branch HEAD names (`symbolic-ref -q HEAD`): while it is a
    /// branch a signing on another branch committed to, a repo file equal to what that signing
    /// committed is no change (ADR-022 §5, `committedOnOtherBranches`).
    ///
    /// # Errors
    /// Store failure, or no approved copy.
    pub fn process_on(
        &self,
        kind: DecisionFileKind,
        id: &str,
        now: &str,
        head_ref: Option<&str>,
    ) -> Result<Processed, DecisionError> {
        let rel = Self::repo_rel(kind)?;
        let disk = match self.project.read(&rel, ReadClass::Decision) {
            Ok((b, _)) => b,
            Err(
                e @ (StoreError::NotFound(_)
                | StoreError::Policy(_)
                | StoreError::NotRegular(_)
                | StoreError::HardLink(..)
                | StoreError::TooLarge { .. }),
            ) => {
                return Ok(Processed::ReadOnly(e.to_string()));
            }
            Err(e) => return Err(e.into()),
        };
        let (approved, _) = self.approved(kind)?;
        let sha = sha256_hex(&disk);
        let (mut a, _) = self.approvals()?;
        let e = entry(&mut a, kind).clone();
        // Foreman's own pending change: the repo file still holds the last committed version.
        let own_pending = e.committed_sha256.is_some() && e.committed_sha256 != e.approved_sha256;
        if own_pending && e.committed_sha256.as_deref() == Some(sha.as_str()) {
            return Ok(Processed::Unchanged);
        }
        if let Some(h) = head_ref
            && e.committed_on_other_branches.get(h).map(String::as_str) == Some(sha.as_str())
        {
            return Ok(Processed::Unchanged);
        }
        Ok(match classify_outside_change(kind, &approved, &disk) {
            Classified::Unchanged => Processed::Unchanged,
            Classified::ReadOnly(r) => Processed::ReadOnly(r),
            Classified::VariablesOnly if !own_pending => {
                self.approve(
                    kind,
                    &disk,
                    e.committed_sha256.as_deref() == Some(sha.as_str()),
                )?;
                Processed::VariablesReloaded
            }
            Classified::VariablesOnly => Processed::Pending(PendingOutside {
                id: id.to_owned(),
                file: kind,
                seen_sha256: sha,
                detected_at: now.to_owned(),
                diff: diff_values(&approved, &disk),
            }),
            Classified::Pending(diff) => Processed::Pending(PendingOutside {
                id: id.to_owned(),
                file: kind,
                seen_sha256: sha,
                detected_at: now.to_owned(),
                diff,
            }),
        })
    }

    /// Foreman's own change (DR-153): the approved copy first, then (before Stage 5 is signed) the
    /// repo file by compare-and-swap; on a repo conflict the approved copy is rolled back.
    ///
    /// # Errors
    /// Store failure (a conflict leaves both files as they were).
    pub fn own_change(
        &self,
        kind: DecisionFileKind,
        bytes: &[u8],
        stage5_signed: bool,
        now: &str,
    ) -> Result<OwnChange, DecisionError> {
        let copy_rel = Self::app_rel(&approved_path(self.project_id, kind))?;
        let (old_copy, copy_stamp) = self.app.read_opt(&copy_rel, ReadClass::Decision)?;
        let repo_rel = Self::repo_rel(kind)?;
        let (repo_now, repo_stamp) = self.project.read_opt(&repo_rel, ReadClass::Decision)?;
        // Never write over an outside edit (SR-33, ADR-022 §4): the repo file must be what was
        // last approved or committed.
        if let (Some(now_bytes), Some(_)) = (&repo_now, &old_copy) {
            let (mut a, _) = self.approvals()?;
            let e = entry(&mut a, kind);
            let h = sha256_hex(now_bytes);
            if e.approved_sha256.as_deref() != Some(h.as_str())
                && e.committed_sha256.as_deref() != Some(h.as_str())
            {
                return Err(DecisionError::OutsidePending(kind.repo_path().into()));
            }
        }
        let sha = sha256_hex(bytes);
        if stage5_signed {
            self.app.write(&copy_rel, bytes, &copy_stamp)?;
            let (mut a, s) = self.approvals()?;
            entry(&mut a, kind).approved_sha256 = Some(sha.clone());
            self.save_approvals(&a, &s)?;
            return Ok(OwnChange::Pending(PendingOwn {
                file: kind,
                sha256: sha,
                at: now.to_owned(),
            }));
        }
        if let Some(old) = old_copy {
            // The copy first, then the repo file; a repo conflict restores the copy.
            let written = self.app.write(&copy_rel, bytes, &copy_stamp)?;
            if let Err(e) = self.project.write(&repo_rel, bytes, &repo_stamp) {
                self.app.write(&copy_rel, &old, &written)?;
                return Err(e.into());
            }
        } else {
            // No approved copy yet (the file's first write): the repo file first, so a failure
            // leaves nothing approved; a copy failure afterwards shows as a pending change.
            self.project.write(&repo_rel, bytes, &repo_stamp)?;
            self.app.write(&copy_rel, bytes, &copy_stamp)?;
        }
        let (mut a, s) = self.approvals()?;
        entry(&mut a, kind).approved_sha256 = Some(sha.clone());
        self.save_approvals(&a, &s)?;
        Ok(OwnChange::CommitNow {
            file: kind,
            path: kind.repo_path().to_owned(),
            sha256: sha,
        })
    }
}

fn diff_values(a: &[u8], b: &[u8]) -> Vec<DiffEntry> {
    match (crate::json::parse_value(a), crate::json::parse_value(b)) {
        (Ok(x), Ok(y)) => diff(&x, &y),
        _ => Vec::new(),
    }
}
