//! `IpcError` (`docs/contracts/ipc.md` §3): a closed code set; the UI branches only on `code` and
//! `detail.kind`, never on `message` (ADR-004 §5).

use serde::{Deserialize, Serialize};

use super::types::{ReadOnly, RepoPath, ToolKind};

/// The error every command returns (IPC-R3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct IpcError {
    pub code: IpcErrorCode,
    pub message: String,
    pub detail: Option<IpcErrorDetail>,
}

/// The closed set of error codes (`ipc::error_codes_closed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum IpcErrorCode {
    Invalid,
    NotFound,
    Expired,
    Stale,
    ReadOnly,
    NotAllowed,
    Conflict,
    PolicyViolation,
    ToolMissing,
    ToolFailed,
    Clipboard,
    Cancelled,
    Unsupported,
    Io,
    Internal,
}

/// What went stale (§3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum StaleWhat {
    ProjectRev,
    ReadStamp,
    Fingerprint,
    Plan,
}

/// Why a clipboard read failed (§3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ClipboardProblem {
    Empty,
    NotText,
    TooLarge,
}

/// Structured detail per code (§3), tagged by `kind`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum IpcErrorDetail {
    Invalid {
        field: String,
        rule: String,
    },
    Stale {
        what: StaleWhat,
        #[cfg_attr(feature = "ts", ts(type = "unknown"))]
        fresh: Option<serde_json::Value>,
    },
    ReadOnly(ReadOnly),
    NotAllowed {
        rule: String,
    },
    Conflict {
        path: RepoPath,
        written: Vec<RepoPath>,
    },
    PolicyViolation {
        path: String,
        rule: String,
    },
    ToolMissing {
        tool: ToolKind,
        path: Option<String>,
    },
    ToolFailed {
        tool: String,
        exit: Option<i32>,
        timed_out: bool,
        stderr_tail: String,
    },
    Clipboard {
        why: ClipboardProblem,
    },
    Unsupported {
        feature: String,
        why: String,
    },
}

impl IpcError {
    /// A request that didn't match its command's shape (IPC-R1, IPC-R2). The message is
    /// Foreman-written and never echoes request values (SR-57).
    #[must_use]
    pub fn invalid(field: &str, rule: &str) -> Self {
        Self {
            code: IpcErrorCode::Invalid,
            message: "The request didn't match the command's shape.".to_owned(),
            detail: Some(IpcErrorDetail::Invalid {
                field: field.to_owned(),
                rule: rule.to_owned(),
            }),
        }
    }
}
