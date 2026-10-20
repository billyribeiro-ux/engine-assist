//! Action steps with fixed destinations (DR-036, SR-34, AC-F007-02/-03). The vocabulary is a Rust
//! table keyed by step ID; destinations are constants. Template, packet or overlay text can't
//! change a destination.

use serde::{Deserialize, Serialize};

use crate::pathpolicy::{COUNCIL_AGENTS, ReadClass, RepoPath, WritePurpose, check_write};
use crate::store::{ReadStamp, Root, StoreError};
use crate::template::Template;

/// One planned write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedWrite {
    /// Destination (a constant).
    pub path: RepoPath,
    /// Bytes to write.
    pub bytes: Vec<u8>,
    /// What's there now (listed with a diff; written only after Billy confirms, AC-F007-03).
    pub existing: Option<Vec<u8>>,
    /// The stamp read when planning (the write is refused if the file changed since).
    pub stamp: ReadStamp,
}

/// An action plan: every destination listed before anything is written (SR-35).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPlan {
    /// The action (`write-blocks`, `copy-file`).
    pub action: &'static str,
    /// The writes.
    pub writes: Vec<PlannedWrite>,
    /// Fences skipped because their `name:` isn't a council agent.
    pub refused: Vec<String>,
}

/// Why an action can't be planned or applied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ActionError {
    /// No action for this step ID.
    #[error("no action is defined for step {0}")]
    Unknown(String),
    /// An existing destination needs Billy's confirmation.
    #[error("{0} exists; review the diff and confirm")]
    NeedsConfirmation(String),
    /// Store failure (a conflict included: the file changed since it was read).
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The source is missing.
    #[error("{0} is missing")]
    MissingSource(String),
}

/// The constant destination of a council agent.
#[must_use]
pub fn council_destination(i: usize) -> Option<RepoPath> {
    let name = COUNCIL_AGENTS.get(i)?;
    check_write(
        &format!("spec/.claude/agents/{name}.md"),
        WritePurpose::CouncilSetup(i),
    )
    .ok()
}

/// `1.4b`'s constant destination (PROPOSAL B-8).
pub const APP_BRIEF: &str = "spec/01-app-brief.md";
/// `1.4b`'s constant sources.
pub const APP_BRIEF_SOURCES: [&str; 2] = [
    "docs/handoff/stage-01.md",
    "docs/handoff/stage-01-attachments.md",
];

fn planned(root: &Root, path: RepoPath, bytes: Vec<u8>) -> Result<PlannedWrite, StoreError> {
    let (existing, stamp) = root.read_opt(&path, ReadClass::Decision)?;
    Ok(PlannedWrite {
        path,
        bytes,
        existing,
        stamp,
    })
}

/// Plans an action step's writes (reads the current destinations through the store).
///
/// # Errors
/// See [`ActionError`].
pub fn plan(t: &Template, step_id: &str, root: &Root) -> Result<ActionPlan, ActionError> {
    match step_id {
        "council-setup" => {
            let mut writes = Vec::new();
            let mut refused = Vec::new();
            for b in &t.council_blocks {
                let Some(i) = COUNCIL_AGENTS.iter().position(|a| *a == b.name) else {
                    refused.push(b.name.clone());
                    continue;
                };
                let Some(dest) = council_destination(i) else {
                    continue;
                };
                writes.push(planned(root, dest, b.text.clone().into_bytes())?);
            }
            Ok(ActionPlan {
                action: "write-blocks",
                writes,
                refused,
            })
        }
        "1.4b" => {
            let mut text = String::new();
            for (k, src) in APP_BRIEF_SOURCES.iter().enumerate() {
                let p = crate::pathpolicy::check_read(src, ReadClass::Decision)
                    .map_err(StoreError::from)?;
                match root.read_opt(&p, ReadClass::Decision)? {
                    (Some(b), _) => {
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(&String::from_utf8_lossy(&b));
                    }
                    (None, _) if k == 0 => {
                        return Err(ActionError::MissingSource((*src).to_owned()));
                    }
                    (None, _) => {}
                }
            }
            let dest = check_write(APP_BRIEF, WritePurpose::Repo).map_err(StoreError::from)?;
            Ok(ActionPlan {
                action: "copy-file",
                writes: vec![planned(root, dest, text.into_bytes())?],
                refused: Vec::new(),
            })
        }
        other => Err(ActionError::Unknown(other.to_owned())),
    }
}

/// Applies a plan. Existing destinations whose content differs are written only when `confirmed`
/// (Billy confirmed the diff); a destination that changed since planning is refused (CAS).
///
/// # Errors
/// See [`ActionError`].
pub fn apply(
    plan: &ActionPlan,
    root: &Root,
    confirmed: bool,
) -> Result<Vec<ReadStamp>, ActionError> {
    for w in &plan.writes {
        if w.existing.as_ref().is_some_and(|e| *e != w.bytes) && !confirmed {
            return Err(ActionError::NeedsConfirmation(w.path.to_string()));
        }
    }
    let mut out = Vec::new();
    for w in &plan.writes {
        if w.existing.as_ref() == Some(&w.bytes) {
            out.push(w.stamp.clone());
            continue;
        }
        out.push(root.write(&w.path, &w.bytes, &w.stamp)?);
    }
    Ok(out)
}

/// The action vocabulary (for display).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionKind {
    /// `council-setup`.
    WriteBlocks,
    /// `1.4b`.
    CopyFile,
}
