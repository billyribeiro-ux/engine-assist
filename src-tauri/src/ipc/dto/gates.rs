//! Responses of §4.11–§4.14 (P2: gates, signing, lessons, manifest, assembler, desktop); §2.4
//! types settled in Stage 5.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::app::response;
use super::steps::ChatChips;
use crate::ipc::types::{
    ConditionId, Diff, ItemId, PreviewId, RepoPath, Sha256, Stage, StepKey, TextSpan, TriState,
};

/// §2.4 `GateState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum GateState {
    NotStarted,
    InProgress,
    Failed,
    Passed,
    Signed,
}

/// §2.4 an artifact's kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ArtifactKind {
    Path,
    Command,
}

response!(
    /// §2.4 `GateArtifact`.
    GateArtifact { kind: ArtifactKind, text: String }
);

/// §2.4 an item's kind as v1 acts on it (DR-031).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ItemKind {
    Attest,
    Time,
}

/// §2.4 an item's status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ItemStatus {
    Pass,
    Fail,
    Waiting,
    NotApplicable,
}

/// §2.4 a Gate A proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Proposed {
    Pass,
    Fail,
}

response!(
    /// §2.4 `GateItemView`.
    GateItemView {
        id: ItemId,
        text: String,
        kind: ItemKind,
        how: Option<String>,
        status: ItemStatus,
        evidence: Option<Vec<TextSpan>>,
        proposed: Option<Proposed>,
        note: Option<String>,
        waits_for: Option<String>,
        artifacts: Vec<GateArtifact>,
        failures: u32,
        needs_reconfirm: bool,
        session: Option<String>,
        text_sha256: Sha256,
        previous_text: Option<String>,
    }
);

/// §2.4 a close-out step's key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum CloseoutKey {
    GateA,
    Results,
    GateB,
    GateC,
    Sign,
}

/// §2.4 a close-out step's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum CloseoutState {
    Done,
    Now,
    Failed,
    Pending,
    Locked,
}

response!(
    /// §2.4 `CloseoutStep`.
    CloseoutStep {
        key: CloseoutKey,
        label: String,
        state: CloseoutState,
        note: Option<String>,
    }
);

response!(
    /// `GateView` (§4.11) with `importedFrom` (§2.4).
    GateView {
        stage: Stage,
        state: GateState,
        items: Vec<GateItemView>,
        closeout: Vec<CloseoutStep>,
        signable: bool,
        why_not: Vec<String>,
        imported_from: Option<String>,
    }
);

/// A Gate A import proposal's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ImportResult {
    Pass,
    Fail,
    Unmatched,
}

response!(
    /// `ImportProposal` (§4.11).
    ImportProposal {
        item: ItemId,
        proposed: ImportResult,
        evidence: Vec<TextSpan>,
    }
);

response!(
    /// `SignPreview` (§4.12) with `branch`, `commitMessage` and `pathNotes` (§2.4).
    SignPreview {
        preview_id: PreviewId,
        packet_sha256: Sha256,
        short: String,
        added_fields: Diff,
        paths: Vec<RepoPath>,
        confirm_conditions: Option<Vec<ConditionId>>,
        chain_busy: bool,
        blockers: Vec<String>,
        branch: String,
        commit_message: String,
        #[cfg_attr(feature = "ts", ts(type = "Record<string, string>"))]
        path_notes: BTreeMap<String, String>,
    }
);

/// `SignResult` (§4.12).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum SignResult {
    Signed { commit: String, stage: Stage },
    CommitPending { stderr_tail: String },
    ReadBackMismatch { diffs: Vec<Diff> },
}

response!(
    /// §2.4 `LessonRef`.
    LessonRef { id: String, title: String }
);

response!(
    /// `LessonsStatus` (§4.13).
    LessonsStatus {
        met: bool,
        entries: Vec<LessonRef>,
        next_id: String,
        file_present: bool,
    }
);

response!(
    /// §2.4 `ManifestRow`.
    ManifestRow {
        path: RepoPath,
        exists: bool,
        in_project: bool,
        producer: Option<StepKey>,
    }
);

response!(
    /// `Manifest` (§4.13).
    Manifest {
        files: Vec<ManifestRow>,
        ready: u32,
        total: u32,
        first_producer: Option<StepKey>,
    }
);

/// An assembler row's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum AssemblerState {
    Exists,
    Missing,
    Empty,
    Refused,
}

response!(
    /// `AssemblerRow` (§4.13) with `note` (§2.4).
    AssemblerRow {
        path: String,
        state: AssemblerState,
        rule: Option<String>,
        note: Option<String>,
    }
);

response!(
    /// §2.4 `LateConditionView`.
    LateConditionView {
        condition: ConditionId,
        value: TriState,
        proposed: Option<TriState>,
        evidence: Option<String>,
        decided: Option<String>,
    }
);

response!(
    /// `Assembler` (§4.13).
    Assembler {
        rows: Vec<AssemblerRow>,
        complete: bool,
        late_conditions: Vec<LateConditionView>,
    }
);

response!(
    /// `DesktopOpened` (§4.14).
    DesktopOpened { chips: ChatChips }
);
