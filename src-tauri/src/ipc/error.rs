//! `IpcError` (`docs/contracts/ipc.md` §3): a closed code set; the UI branches only on `code` and
//! `detail.kind`, never on `message` (ADR-004 §5). Messages are Foreman-written and never echo
//! request values (SR-57).

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
    fn new(code: IpcErrorCode, message: impl Into<String>, detail: Option<IpcErrorDetail>) -> Self {
        Self {
            code,
            message: message.into(),
            detail,
        }
    }

    /// A request that didn't match its command's shape (IPC-R1, IPC-R2). The message is
    /// Foreman-written and never echoes request values (SR-57).
    #[must_use]
    pub fn invalid(field: &str, rule: &str) -> Self {
        Self::new(
            IpcErrorCode::Invalid,
            "The request didn't match the command's shape.",
            Some(IpcErrorDetail::Invalid {
                field: field.to_owned(),
                rule: rule.to_owned(),
            }),
        )
    }

    /// An unknown ID or missing thing.
    #[must_use]
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(IpcErrorCode::NotFound, message, None)
    }

    /// A one-time ID already used or past its validity (IPC-R7).
    #[must_use]
    pub fn expired() -> Self {
        Self::new(
            IpcErrorCode::Expired,
            "This preview expired. Review it again.",
            None,
        )
    }

    /// The state moved since the preview (IPC-R7); `fresh` is the new preview, if any.
    #[must_use]
    pub fn stale(
        what: StaleWhat,
        message: impl Into<String>,
        fresh: Option<serde_json::Value>,
    ) -> Self {
        Self::new(
            IpcErrorCode::Stale,
            message,
            Some(IpcErrorDetail::Stale { what, fresh }),
        )
    }

    /// A mutation on a read-only project (IPC-R9).
    #[must_use]
    pub fn read_only(ro: ReadOnly) -> Self {
        Self::new(
            IpcErrorCode::ReadOnly,
            ro.detail.clone(),
            Some(IpcErrorDetail::ReadOnly(ro)),
        )
    }

    /// A rule refused the action; `rule` is the stable rule code.
    #[must_use]
    pub fn not_allowed(rule: &str, message: impl Into<String>) -> Self {
        Self::new(
            IpcErrorCode::NotAllowed,
            message,
            Some(IpcErrorDetail::NotAllowed {
                rule: rule.to_owned(),
            }),
        )
    }

    /// A compare-and-swap write found a newer file (IPC-R10).
    #[must_use]
    pub fn conflict(path: &str, written: Vec<RepoPath>) -> Self {
        Self::new(
            IpcErrorCode::Conflict,
            format!("{path} changed on disk since Foreman read it. Review the newer version."),
            Some(IpcErrorDetail::Conflict {
                path: RepoPath::from_trusted(path),
                written,
            }),
        )
    }

    /// A path-policy refusal (SR-30, SR-31).
    #[must_use]
    pub fn policy(path: &str, rule: &str) -> Self {
        Self::new(
            IpcErrorCode::PolicyViolation,
            "Foreman doesn't read or write that path.",
            Some(IpcErrorDetail::PolicyViolation {
                path: crate::render::escape_visible(path),
                rule: rule.to_owned(),
            }),
        )
    }

    /// A tool isn't installed or its path is refused.
    #[must_use]
    pub fn tool_missing(tool: ToolKind, path: Option<String>) -> Self {
        let name = match tool {
            ToolKind::Claude => "claude",
            ToolKind::Git => "git",
        };
        Self::new(
            IpcErrorCode::ToolMissing,
            format!("{name} wasn't found. Doctor shows where Foreman looks."),
            Some(IpcErrorDetail::ToolMissing { tool, path }),
        )
    }

    /// A tool ran and failed; `stderr_tail` must already be redacted (SR-54).
    #[must_use]
    pub fn tool_failed(
        tool: &str,
        exit: Option<i32>,
        timed_out: bool,
        stderr_tail: String,
    ) -> Self {
        Self::new(
            IpcErrorCode::ToolFailed,
            format!("{tool} failed."),
            Some(IpcErrorDetail::ToolFailed {
                tool: tool.to_owned(),
                exit,
                timed_out,
                stderr_tail,
            }),
        )
    }

    /// The clipboard couldn't be read as text (SR-56).
    #[must_use]
    pub fn clipboard(why: ClipboardProblem) -> Self {
        let message = match why {
            ClipboardProblem::Empty => "The clipboard is empty.",
            ClipboardProblem::NotText => "The clipboard doesn't hold text.",
            ClipboardProblem::TooLarge => "The clipboard holds more than 1 MB.",
        };
        Self::new(
            IpcErrorCode::Clipboard,
            message,
            Some(IpcErrorDetail::Clipboard { why }),
        )
    }

    /// Billy declined a native dialog (IPC-R11): nothing changed.
    #[must_use]
    pub fn cancelled() -> Self {
        Self::new(IpcErrorCode::Cancelled, "Cancelled. Nothing changed.", None)
    }

    /// A feature this build or this Claude Code doesn't support.
    #[must_use]
    pub fn unsupported(feature: &str, why: &str, message: impl Into<String>) -> Self {
        Self::new(
            IpcErrorCode::Unsupported,
            message,
            Some(IpcErrorDetail::Unsupported {
                feature: feature.to_owned(),
                why: why.to_owned(),
            }),
        )
    }

    /// IPC-R12: a registered command whose backend ticket hasn't landed yet.
    #[must_use]
    pub fn not_yet_available(command: &str, ticket: &str) -> Self {
        Self::unsupported(
            command,
            &format!("notYetAvailable:{ticket}"),
            format!("This isn't built yet ({ticket})."),
        )
    }

    /// A filesystem error.
    #[must_use]
    pub fn io(message: impl Into<String>) -> Self {
        Self::new(IpcErrorCode::Io, message, None)
    }

    /// A bug: a caught panic or an invariant broken. Logged with the location only (SR-57).
    #[must_use]
    pub fn internal() -> Self {
        Self::new(
            IpcErrorCode::Internal,
            "Something went wrong inside Foreman. Nothing was changed by this request.",
            None,
        )
    }
}

impl From<foreman_core::store::StoreError> for IpcError {
    fn from(e: foreman_core::store::StoreError) -> Self {
        use foreman_core::store::StoreError as S;
        match e {
            S::Policy(p) => Self::policy(&p.to_string(), p.rule()),
            S::NotFound(name) => Self::not_found(format!("{name} wasn't found.")),
            S::Conflict { name, .. } => Self::conflict(&name, Vec::new()),
            other => Self::io(other.to_string()),
        }
    }
}

impl From<foreman_core::pathpolicy::PolicyViolation> for IpcError {
    fn from(p: foreman_core::pathpolicy::PolicyViolation) -> Self {
        Self::policy(&p.to_string(), p.rule())
    }
}
