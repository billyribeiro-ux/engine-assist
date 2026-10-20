//! Shared IPC scalars and enums (`docs/contracts/ipc.md` §2). Exported to TS by ts-rs (ADR-004).

use serde::{Deserialize, Serialize};

/// `ProjectId(Uuid)` on the wire: a lowercase hyphenated UUID string (§2.1). Validation (format,
/// existence → `NotFound`) arrives with the first command that takes one (M2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectId(pub String);

/// Monotonic per-project revision (§2.1, IPC-R4: `u32`, never 64-bit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Rev(pub u32);

/// Blueprint stage number, `0..=18` (§2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Stage(pub u8);

/// A repo-relative path, ≤ 1,024 bytes, checked by `pathpolicy` before use (§2.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RepoPath(pub String);

/// Why a project is open read-only (§2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ReadOnlyReason {
    LockMismatch,
    BlueprintMissing,
    DecisionFileInvalid,
    NewerSchema,
    GitMissing,
    FolderMissing,
}

/// Doctor's overall level (§2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum DoctorLevel {
    Ok,
    Warning,
    Failing,
    Unknown,
}

/// An external tool Foreman runs (§2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ToolKind {
    Claude,
    Git,
}

/// `ReadOnly { reason, detail }` (§2.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ReadOnly {
    pub reason: ReadOnlyReason,
    pub detail: String,
}
