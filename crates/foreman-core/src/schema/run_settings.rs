//! `runs/<runId>/settings.json`, in Claude Code's settings format (`docs/contracts/files.md` §4.4).
//! No `schemaVersion` (FILE-R2 exception): written once, never read back as data.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The permissions block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Permissions {
    /// Always `disable`.
    pub disable_bypass_permissions_mode: String,
    /// Exactly four deny rules.
    pub deny: Vec<String>,
}

/// One hook command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookCommand {
    /// Always `command`.
    pub r#type: String,
    /// The sensor binary.
    pub command: String,
    /// Eight arguments.
    pub args: Vec<String>,
}

/// One hook matcher group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HookGroup {
    /// `Bash` on `PostToolUse` only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    /// One command.
    pub hooks: Vec<HookCommand>,
}

/// The run settings file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunSettingsFile {
    /// Always false.
    pub disable_all_hooks: bool,
    /// Deny rules.
    pub permissions: Permissions,
    /// S-3 fail branch only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort_level: Option<String>,
    /// P2 sensor hooks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<BTreeMap<String, Vec<HookGroup>>>,
}
