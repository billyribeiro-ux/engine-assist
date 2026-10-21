//! `.foreman/stages/stage-NN.json` v1 (P2; `docs/contracts/files.md` §3.4). Immutable once signed.

use serde::{Deserialize, Serialize};

use super::run::Observed;
use super::{Migration, Versioned};
use crate::ids::{Start, StepKey};

/// The signing branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Branch {
    /// `current` or `other`.
    pub mode: String,
    /// `refs/heads/…`.
    pub r#ref: String,
}

/// A hashed file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HashedPath {
    /// Repo path.
    pub path: String,
    /// Hash.
    pub sha256: String,
}

/// The packet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PacketRef {
    /// Repo path.
    pub path: String,
    /// Hash.
    pub sha256: String,
    /// Attachments.
    pub attachments: Option<HashedPath>,
}

/// One step's record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StepRecord {
    /// Step.
    pub step: StepKey,
    /// `done`, `skipped`, `notApplicable`.
    pub state: String,
    /// Reason (skips).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Runs.
    pub run_ids: Vec<String>,
    /// Chain.
    pub chain_id: Option<String>,
    /// Intended start.
    pub intended: Option<Start>,
    /// Observed.
    pub observed: Option<Observed>,
    /// Prompt hashes.
    pub prompt_sha256s: Vec<String>,
    /// Active time.
    pub active_ms: u64,
}

/// One gate item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GateRecord {
    /// Item.
    pub item: String,
    /// Kind.
    pub kind: String,
    /// Outcome.
    pub outcome: String,
    /// Note.
    pub text: String,
    /// Link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

/// git facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GitRecord {
    /// The `-c` overrides used.
    pub overrides: Vec<String>,
    /// Always `noGpgSign`.
    pub sign_mode: String,
    /// Env names.
    pub env: Vec<String>,
}

/// `stage-NN.json` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StageRecordFile {
    /// Always 1.
    #[serde(deserialize_with = "super::version_const::check::<_, 1>")]
    pub schema_version: u32,
    /// Stage.
    pub stage: u8,
    /// Name.
    pub signed_by: String,
    /// Date.
    pub signed_on: String,
    /// Branch.
    pub branch: Branch,
    /// Packet.
    pub packet: PacketRef,
    /// Steps.
    pub steps: Vec<StepRecord>,
    /// Gate.
    pub gate: Vec<GateRecord>,
    /// `SKIPPED:` lines.
    pub skipped: Vec<String>,
    /// `SUBSTITUTIONS:` lines.
    pub substitutions: Vec<String>,
    /// git.
    pub git: GitRecord,
}

impl Versioned for StageRecordFile {
    const KIND: &'static str = "stage-record";
    const CURRENT: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}
