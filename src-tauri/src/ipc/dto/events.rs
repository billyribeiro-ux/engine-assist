//! Event payloads (`docs/contracts/ipc.md` §6): Rust → webview, invalidation only. Payloads carry
//! identifiers, numbers, enums and booleans, never content (`events::no_content_fields`).

use serde::{Deserialize, Serialize};

use crate::ipc::types::{CwdKind, DoctorLevel, ProjectId, Rev, RunId, RunStatus, Stage};

/// `project:changed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectChanged {
    pub project_id: ProjectId,
    pub rev: Rev,
}

/// `projects:changed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ProjectsChanged {
    pub rev: Rev,
}

/// `packet:changed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct PacketChanged {
    pub project_id: ProjectId,
    pub stage: Stage,
    pub rev: Rev,
}

/// `decision:pending`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DecisionPending {
    pub project_id: ProjectId,
    pub count: u32,
}

/// `tamper:detected` (P2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct TamperDetected {
    pub project_id: ProjectId,
    pub stage: Stage,
    pub rev: Rev,
}

/// `run:status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RunStatusChanged {
    pub run_id: RunId,
    pub status: RunStatus,
    pub rev: Rev,
}

/// `config:changed` (a warning only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ConfigChanged {
    pub project_id: ProjectId,
    pub cwd: CwdKind,
}

/// `doctor:status`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct DoctorStatus {
    pub level: DoctorLevel,
}

/// `settings:changed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SettingsChanged {
    pub rev: Rev,
}

/// One event Rust emits: its §6 name and payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    ProjectChanged(ProjectChanged),
    ProjectsChanged(ProjectsChanged),
    PacketChanged(PacketChanged),
    DecisionPending(DecisionPending),
    TamperDetected(TamperDetected),
    RunStatus(RunStatusChanged),
    ConfigChanged(ConfigChanged),
    DoctorStatus(DoctorStatus),
    SettingsChanged(SettingsChanged),
}

/// Every event name of §6, in the table's order.
pub const EVENT_NAMES: [&str; 9] = [
    "project:changed",
    "projects:changed",
    "packet:changed",
    "decision:pending",
    "tamper:detected",
    "run:status",
    "config:changed",
    "doctor:status",
    "settings:changed",
];

impl Event {
    /// The §6 event name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::ProjectChanged(_) => "project:changed",
            Self::ProjectsChanged(_) => "projects:changed",
            Self::PacketChanged(_) => "packet:changed",
            Self::DecisionPending(_) => "decision:pending",
            Self::TamperDetected(_) => "tamper:detected",
            Self::RunStatus(_) => "run:status",
            Self::ConfigChanged(_) => "config:changed",
            Self::DoctorStatus(_) => "doctor:status",
            Self::SettingsChanged(_) => "settings:changed",
        }
    }

    /// The JSON payload.
    #[must_use]
    pub fn payload(&self) -> serde_json::Value {
        let v = match self {
            Self::ProjectChanged(p) => serde_json::to_value(p),
            Self::ProjectsChanged(p) => serde_json::to_value(p),
            Self::PacketChanged(p) => serde_json::to_value(p),
            Self::DecisionPending(p) => serde_json::to_value(p),
            Self::TamperDetected(p) => serde_json::to_value(p),
            Self::RunStatus(p) => serde_json::to_value(p),
            Self::ConfigChanged(p) => serde_json::to_value(p),
            Self::DoctorStatus(p) => serde_json::to_value(p),
            Self::SettingsChanged(p) => serde_json::to_value(p),
        };
        v.unwrap_or(serde_json::Value::Null)
    }
}
