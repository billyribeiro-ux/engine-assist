//! `.foreman/project.json` v1 (`docs/contracts/files.md` §3.1). Decision fields: `name`, `slug`,
//! `specFolder`, `conditions`, `conditionAudit`, `modelPolicy`; variable fields (auto-accepted on an
//! outside change, DR-151): `variables`, `chatProjectUrl`, `chatUrls`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Migration, Versioned};
use crate::ids::{ConditionId, Effort, Model, StepKey, TriState};

/// The conditions, one value per condition ID (all twelve required).
pub type Conditions = BTreeMap<ConditionId, TriState>;

/// One audit line per condition change (DR-052; append-only, time order).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConditionAudit {
    /// The condition.
    pub condition: ConditionId,
    /// The previous value.
    pub from: TriState,
    /// The new value.
    pub to: TriState,
    /// RFC 3339 UTC.
    pub at: String,
    /// Billy's note.
    pub note: String,
}

/// `pro` or `max` (DR-101).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Plan {
    /// Max plan.
    Max,
    /// Pro plan.
    Pro,
}

/// `{from, to, effort}` (DR-102).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Substitution {
    /// Model replaced.
    pub from: Model,
    /// Replacement model.
    pub to: Model,
    /// Replacement effort, when not null.
    pub effort: Option<Effort>,
}

/// The project's model policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelPolicy {
    /// Which `alt=` rule applies.
    pub plan: Plan,
    /// Applied in order, once each.
    pub substitutions: Vec<Substitution>,
}

/// `.foreman/project.json` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectFile {
    /// Always 1.
    #[serde(deserialize_with = "super::version_const::check::<_, 1>")]
    pub schema_version: u32,
    /// UUID v4.
    pub project_id: String,
    /// Display name.
    pub name: String,
    /// `^[a-z0-9-]{1,40}$`.
    pub slug: String,
    /// Always `spec`.
    pub spec_folder: String,
    /// Project variables (`APP_NAME`, …).
    pub variables: BTreeMap<String, String>,
    /// All twelve conditions.
    pub conditions: Conditions,
    /// Append-only audit.
    pub condition_audit: Vec<ConditionAudit>,
    /// Model policy.
    pub model_policy: ModelPolicy,
    /// The claude.ai Project URL.
    pub chat_project_url: Option<String>,
    /// Conversation URL per chat chain start.
    pub chat_urls: BTreeMap<StepKey, String>,
}

impl Versioned for ProjectFile {
    const KIND: &'static str = "project";
    const CURRENT: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}

impl ProjectFile {
    /// The variable fields (DR-151): differences only here are auto-accepted.
    pub const VARIABLE_FIELDS: [&'static str; 3] = ["variables", "chatProjectUrl", "chatUrls"];
}
