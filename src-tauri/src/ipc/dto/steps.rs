//! Responses of §4.4 (steps) and §4.5 (chat); §2.4 types settled in Stage 5.

use serde::{Deserialize, Serialize};

use super::app::{StepSummary, response};
use crate::ipc::types::{
    ActionKind, Blocker, Diff, EnvName, Explained, PasteKind, PreviewId, Problem, PromptSegment,
    RepoPath, RunId, RunOverride, Stage, Start, Timestamp,
};

response!(
    /// §2.4 `RunInputView`: one `in=` run input (F-006).
    RunInputView {
        name: String,
        label: String,
        token: String,
        multiline: bool,
        remember: bool,
        required: bool,
        value: String,
        warning: Option<String>,
    }
);

response!(
    /// §2.4 `SwitchCue`: an in-step cue ("re-arms with Accept edits"); never sent as keys (DR-105).
    SwitchCue { text: String }
);

response!(
    /// §2.4 `ChatChip`: one chip of a chat step's run-with line.
    ChatChip {
        id: String,
        label: String,
        strong: bool,
    }
);

response!(
    /// §2.4 `ChatChips`.
    ChatChips {
        chips: Vec<ChatChip>,
        link: Option<String>,
        link_label: Option<String>,
    }
);

response!(
    /// §2.4 `OutView`: a declared `out` path and whether it exists.
    OutView { path: RepoPath, exists: bool }
);

response!(
    /// §2.4 `StepPart`: an optional part's Include toggle (F-006).
    StepPart {
        part_id: String,
        label: String,
        included: bool,
        locked: bool,
    }
);

response!(
    /// `StepView` (§4.4) with `parts` and `copiedAt` (§2.4). `override` is the wire name of
    /// the contract's `override_`.
    StepView {
        summary: StepSummary,
        prompt: Vec<PromptSegment>,
        inputs: Vec<RunInputView>,
        start: Option<Explained<Start>>,
        switches: Vec<SwitchCue>,
        chips: Option<ChatChips>,
        argv_preview: Option<Vec<String>>,
        env_names: Vec<EnvName>,
        blockers: Vec<Blocker>,
        warnings: Vec<String>,
        remaps: Vec<String>,
        outs: Vec<OutView>,
        r#override: Option<RunOverride>,
        parts: Vec<StepPart>,
        copied_at: Option<Timestamp>,
        live: Option<LiveMode>,
    }
);

response!(
    /// (M4-11, AC-F009-11) The live session's mode as the P2 sensor reports it, for the Now/Next
    /// strip: `mode` is its display name; `expected` is set when it is neither the step's start
    /// mode nor a switch target (the mode guard's mismatch, AC-F011-06).
    LiveMode {
        mode: String,
        expected: Option<String>,
    }
);

response!(
    /// `ActionPreview` (§4.4).
    ActionPreview {
        preview_id: PreviewId,
        action: ActionKind,
        writes: Vec<Diff>,
    }
);

response!(
    /// `CopyResult` (§4.4).
    CopyResult {
        removed_controls: u32,
        focus_run: Option<RunId>,
    }
);

/// Which chat link is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum MissingLink {
    Project,
    Conversation,
}

response!(
    /// `ChatCopyResult` (§4.5).
    ChatCopyResult {
        run_id: RunId,
        opened: bool,
        missing_link: Option<MissingLink>,
        removed_controls: u32,
    }
);

response!(
    /// §2.4 `PacketFields`: the header's stages, from a pasted packet.
    PacketFields {
        from_stage: Stage,
        to_stage: Option<Stage>,
    }
);

response!(
    /// `PastePreview` (§4.5).
    PastePreview {
        preview_id: PreviewId,
        kind: PasteKind,
        targets: Vec<Diff>,
        problems: Vec<Problem>,
        notes: Vec<String>,
        packet_fields: Option<PacketFields>,
    }
);

response!(
    /// `PasteApplyResult` (§4.5).
    PasteApplyResult {
        written: Vec<RepoPath>,
        step_done: bool,
    }
);
