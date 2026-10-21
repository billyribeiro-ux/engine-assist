//! `approvals/<projectId>.json` v1 · class (b) (`docs/contracts/files.md` §4.5; ADR-022).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Migration, Versioned};
use crate::ids::ItemId;

/// Approval hashes for one decision file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Approval {
    /// Hash of the approved copy.
    pub approved_sha256: Option<String>,
    /// Hash last committed on the checked-out branch.
    pub committed_sha256: Option<String>,
    /// Per other branch ref, what a signing there committed.
    pub committed_on_other_branches: BTreeMap<String, String>,
}

/// The three decision files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalFiles {
    /// `project.json`.
    pub project: Approval,
    /// `overlay.json`.
    pub overlay: Approval,
    /// `blueprint.lock.json`.
    pub blueprint_lock: Approval,
}

/// A confirmed item kind after drift (DR-157).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ItemKindConfirm {
    /// The drifted text hash confirmed.
    pub text_sha256: String,
    /// The kind confirmed.
    pub kind: String,
    /// RFC 3339 UTC.
    pub at: String,
}

/// A project env approval (S-7′ fail branch).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvApproval {
    /// File.
    pub file: String,
    /// Hash.
    pub sha256: String,
    /// RFC 3339 UTC.
    pub at: String,
}

/// A confirmed outside signature (DR-135).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutsideSignature {
    /// The packet hash bound by the confirmation.
    pub packet_sha256: String,
    /// Name.
    pub signed_by: String,
    /// Date.
    pub signed_on: String,
    /// RFC 3339 UTC.
    pub at: String,
}

/// `approvals/<projectId>.json` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovalsFile {
    /// Always 1.
    #[serde(deserialize_with = "super::version_const::check::<_, 1>")]
    pub schema_version: u32,
    /// Project.
    pub project_id: String,
    /// Hashes per decision file.
    pub files: ApprovalFiles,
    /// Confirmed item kinds.
    pub item_kinds: BTreeMap<ItemId, ItemKindConfirm>,
    /// Env approvals.
    pub env_approvals: Vec<EnvApproval>,
    /// Outside signatures by stage number.
    pub outside_signatures: BTreeMap<String, OutsideSignature>,
}

impl ApprovalsFile {
    /// A fresh approvals file.
    #[must_use]
    pub fn new(project_id: &str) -> Self {
        Self {
            schema_version: 1,
            project_id: project_id.to_owned(),
            files: ApprovalFiles::default(),
            item_kinds: BTreeMap::new(),
            env_approvals: Vec::new(),
            outside_signatures: BTreeMap::new(),
        }
    }
}

impl Versioned for ApprovalsFile {
    const KIND: &'static str = "approvals";
    const CURRENT: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}
