//! Responses of §4.6 (launch) and §4.7 (terminal), and the Channel payloads of §5.

use serde::{Deserialize, Serialize};

use super::app::response;
use crate::ipc::types::{
    Blocker, ChainId, ConfirmReason, EnvName, PlanId, PromptSegment, RepoPath, RunId, RunStatus,
    Sha256,
};

/// What a re-arm needs before relaunching (`sensorBusy`: P2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum RearmNeed {
    IdleConfirmation { chain: ChainId },
    SensorBusy { tasks: u32 },
    Exited { chain: ChainId },
}

response!(
    /// `LaunchPreview` (§4.6): a one-time plan valid 60 s (IPC-R7).
    LaunchPreview {
        plan_id: PlanId,
        expires_in_ms: u32,
        prompt: Vec<PromptSegment>,
        argv: Vec<String>,
        env_names: Vec<EnvName>,
        run_settings_sha256: Sha256,
        confirm: Vec<ConfirmReason>,
        rearm: Option<RearmNeed>,
        warnings: Vec<String>,
        remaps: Vec<String>,
    }
);

/// `PrepareOutcome` (§4.6): a plan, or the blockers (never thrown).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum PrepareOutcome {
    Ready(LaunchPreview),
    Blocked {
        blockers: Vec<Blocker>,
        warnings: Vec<String>,
    },
}

response!(
    /// `RunStarted` (§4.6).
    RunStarted {
        run_id: RunId,
        chain: ChainId,
        tab_name: String,
    }
);

response!(
    /// `AttachInfo` (§4.7).
    AttachInfo {
        status: RunStatus,
        replayed_lines: u32,
        tab_name: String,
    }
);

response!(
    /// `PasteText` (§4.7): sanitized clipboard text the webview pastes with `term.paste`.
    PasteText {
        text: String,
        removed_controls: u32,
    }
);

/// `RearmOutcome` (§4.7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum RearmOutcome {
    Relaunched(RunStarted),
    EndYourself { chain: ChainId },
}

/// `LinkOutcome` (§4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum LinkOutcome {
    Opened,
    Refused,
    Cancelled,
}

/// The step a re-arm's ending is at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum EndingStep {
    CtrlC,
    Exit,
    Waiting,
}

/// `Channel<LaunchProgress>` (§5): phases in DR-120's order; no prompt text, env or config values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "phase",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum LaunchProgress {
    Version { found: String },
    Contract { outcome: String },
    Fingerprint { first_scan: bool },
    Auth { cached: bool },
    Transcript,
    Inputs,
    Ending { step: EndingStep },
    Exited { code: i32 },
    Spawning,
    Done,
}

/// `Channel<SignProgress>` (§5, P2): one message per plumbing step (ADR-012).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "step",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum SignProgress {
    ReRead,
    Write { path: RepoPath },
    HashObject { path: RepoPath },
    WriteTree,
    CommitTree { signed: bool },
    UpdateRef,
    RefreshIndex,
    ReadBack { path: RepoPath, ok: bool },
    Done { commit: String },
}
