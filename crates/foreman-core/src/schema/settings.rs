//! `settings.json` v1 · class (f) (`docs/contracts/files.md` §4.1).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Migration, Versioned};

/// `autoDetect` or `chosen`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RepoSource {
    /// Found among the fixed candidates.
    AutoDetect,
    /// Picked by Billy.
    Chosen,
}

/// The blueprint repo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BlueprintRepo {
    /// Absolute path.
    pub path: String,
    /// How it was found.
    pub source: RepoSource,
}

/// `auto` or `settings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolSource {
    /// Resolved from the fixed candidates.
    Auto,
    /// Set by Billy.
    Settings,
}

/// One tool path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolPath {
    /// Absolute path or null.
    pub path: Option<String>,
    /// Where it came from.
    pub source: ToolSource,
}

/// Tool paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tools {
    /// `claude`.
    pub claude: ToolPath,
    /// `git`.
    pub git: ToolPath,
}

/// `system`, `vellum`, `night`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    /// Follow macOS.
    System,
    /// Light.
    Vellum,
    /// Dark.
    Night,
}

/// Terminal settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Terminal {
    /// 9–32.
    pub font_size: u8,
    /// 1,000–100,000.
    pub scrollback: u32,
    /// Option as Meta.
    pub mac_option_is_meta: bool,
}

/// The captured login PATH.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LoginPath {
    /// Absolute entries.
    pub entries: Vec<String>,
    /// The shell used.
    pub shell: String,
    /// RFC 3339 UTC.
    pub captured_at: String,
}

/// An allowlisted auth key in user settings (SR-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthKeyAllow {
    /// The key.
    pub key: String,
    /// `sha256("<key>=<canonical JSON value>")`.
    pub sha256: String,
    /// RFC 3339 UTC.
    pub added_at: String,
}

/// `pass`, `flagFailure`, `inconclusive`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProbeResult {
    /// Passed.
    Pass,
    /// A flag failed.
    FlagFailure,
    /// Neither.
    Inconclusive,
}

/// A probe outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProbeOutcome {
    /// Result.
    pub result: ProbeResult,
    /// The failing flag, if any.
    pub flag: Option<String>,
}

/// Probe outcomes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProbeOutcomes {
    /// Probe A.
    pub probe_a: ProbeOutcome,
    /// Probe B.
    pub probe_b: ProbeOutcome,
}

/// The last verified Claude Code contract (ADR-017).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Contract {
    /// Version verified.
    pub version: String,
    /// RFC 3339 UTC.
    pub verified_at: String,
    /// Outcomes.
    pub outcomes: ProbeOutcomes,
}

/// `settings.json` v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsFile {
    /// Always 1.
    #[serde(deserialize_with = "super::version_const::check::<_, 1>")]
    pub schema_version: u32,
    /// The blueprint repo.
    pub blueprint_repo: Option<BlueprintRepo>,
    /// The DRI's name.
    pub dri_name: Option<String>,
    /// Tool paths.
    pub tools: Tools,
    /// Editor bundle ID.
    pub editor_bundle_id: Option<String>,
    /// Theme.
    pub theme: Theme,
    /// Terminal.
    pub terminal: Terminal,
    /// Pass-through env names.
    pub env_passthrough: Vec<String>,
    /// Captured login PATH.
    pub login_path: Option<LoginPath>,
    /// Defaults for project variables.
    pub global_variables: BTreeMap<String, String>,
    /// Allowlisted auth keys.
    pub auth_key_allowlist: Vec<AuthKeyAllow>,
    /// Last verified contract.
    pub contract: Option<Contract>,
    /// Last `claude -v` any launch saw.
    pub last_seen_claude_version: Option<String>,
    /// The probe folder was trusted.
    pub probe_folder_trusted: bool,
}

impl Versioned for SettingsFile {
    const KIND: &'static str = "settings";
    const CURRENT: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[];
}
