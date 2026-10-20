//! Responses of §4.1 (app, settings, Today, Do next) and §4.3 (projects). Types marked "§2.4" were
//! contract gaps settled in Stage 5 (`docs/contracts/ipc.md` §2.4).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::launch::PrepareOutcome;
use super::requests::TerminalPrefs;
use crate::ipc::types::{
    Applicability, BundleId, ChainId, ConditionId, CwdKind, DoctorLevel, EnvName, Explained,
    PreviewId, ProjectId, ReadOnly, RedactedDiff, RepoPath, Rev, RunId, RunStatus, Sha256, Stage,
    StepKey, StepKind, StepState, Surface, Tag, Theme, Timestamp, TriState,
};
use crate::ipc::types::{Blocker, PickId};

/// Declares a response struct (`camelCase`, ts-rs export).
macro_rules! response {
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $f:ident : $t:ty),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
        pub struct $name { $($(#[$fm])* pub $f: $t),* }
    };
}
pub(crate) use response;

response!(
    /// `ProjectSummary` (§4.3).
    ProjectSummary {
        id: ProjectId,
        name: String,
        display_path: String,
        rev: Rev,
        read_only: Option<ReadOnly>,
        current_stage: Option<Stage>,
        pinned: Option<u8>,
    }
);

response!(
    /// §2.4 `RunSummary`: one run as the run log, Interrupted sessions and Resume show it. `argv`
    /// has the prompt replaced by `<prompt sha256:…>` (SR-52).
    RunSummary {
        run_id: RunId,
        chain: ChainId,
        project: ProjectId,
        project_name: String,
        step: Option<StepKey>,
        step_title: String,
        tab_name: String,
        ran_with: String,
        substituted: Option<String>,
        status: RunStatus,
        exit_code: Option<i32>,
        error: Option<String>,
        started_at: Timestamp,
        time_to_start_ms: Option<u32>,
        argv: Vec<String>,
        resumable: bool,
        why_not_resumable: Option<String>,
    }
);

response!(
    /// §2.4 `Capabilities`: what this Claude Code supports, as the UI gates features on it.
    Capabilities {
        claude_version: Option<String>,
        desktop: bool,
        sensor: bool,
        probes_verified: bool,
    }
);

response!(
    /// `AppBootstrap` (§4.1).
    AppBootstrap {
        first_run: bool,
        settings_rev: Rev,
        projects: Vec<ProjectSummary>,
        interrupted: Vec<RunSummary>,
        doctor: DoctorLevel,
        capabilities: Capabilities,
    }
);

response!(
    /// §2.4 `ToolPathView`: one tool's resolved path.
    ToolPathView {
        path: Option<String>,
        version: Option<String>,
        auto_detected: bool,
        problem: Option<String>,
    }
);

response!(
    /// §2.4 `ToolPaths`.
    ToolPaths { claude: ToolPathView, git: ToolPathView }
);

response!(
    /// §2.4 `TemplateHealthSummary`: the one-line health shown with the blueprint repo.
    TemplateHealthSummary {
        runnable: String,
        needs_annotation: String,
        errors: u32,
        warnings: u32,
    }
);

response!(
    /// §2.4 `VariableSpec`: a project variable the setup form asks for (DR-082 #1–7).
    VariableSpec {
        name: String,
        label: String,
        placeholder: String,
        multiline: bool,
        from_settings: bool,
    }
);

response!(
    /// §2.4 `ConditionSpec`: a condition the setup form asks about (DR-027).
    ConditionSpec {
        id: ConditionId,
        label: String,
        effect: String,
        late: bool,
    }
);

response!(
    /// `BlueprintRepoInfo` (§4.2) with the setup form's fields (§2.4).
    BlueprintRepoInfo {
        display_path: String,
        commit: String,
        version: String,
        grammar_version: u32,
        dirty: bool,
        health: TemplateHealthSummary,
        variables: Vec<VariableSpec>,
        conditions: Vec<ConditionSpec>,
    }
);

response!(
    /// §2.4 `LoginPathView`.
    LoginPathView { captured_at: Timestamp, entries: Vec<String> }
);

response!(
    /// §2.4 `AllowlistEntry`: an allowlisted auth key in user settings (SR-07); no value.
    AllowlistEntry {
        sha256: Sha256,
        file: String,
        key: String,
        added_at: Timestamp,
    }
);

response!(
    /// §2.4 `RunHistorySize`: Settings → Data.
    RunHistorySize { runs: u32, megabytes: u32 }
);

response!(
    /// `SettingsView` (§4.1) with `screenReaderMode` and `runHistory` (§2.4).
    SettingsView {
        rev: Rev,
        dri_name: Option<String>,
        tools: ToolPaths,
        editor_bundle_id: Option<BundleId>,
        blueprint_repo: Option<BlueprintRepoInfo>,
        theme: Theme,
        terminal: TerminalPrefs,
        env_passthrough: Vec<EnvName>,
        login_path: Option<LoginPathView>,
        #[cfg_attr(feature = "ts", ts(type = "Record<string, string>"))]
        global_variables: BTreeMap<String, String>,
        auth_key_allowlist: Vec<AllowlistEntry>,
        screen_reader_mode: bool,
        run_history: RunHistorySize,
    }
);

/// §2.4 `InboxKind` (the order is AC-F017-07's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum InboxKind {
    SessionBlocked,
    SessionYourTurn,
    PacketLanded,
    PacketInvalid,
    GateFailed,
    GateSignable,
    OutsideChange,
    Tamper,
    ChatAwaitingPaste,
    WaitElapsed,
    InterruptedSession,
    StepReady,
}

response!(
    /// `InboxItem` (§4.1) with Rust-written text (§2.4, SR-56).
    InboxItem {
        kind: InboxKind,
        project: ProjectId,
        step: Option<StepKey>,
        stage: Option<Stage>,
        at: Timestamp,
        project_name: String,
        title: String,
        detail: String,
        chain: Option<ChainId>,
        run: Option<RunId>,
    }
);

/// §2.4 `SpecSurface`: a spec line's surface (`action` for action steps).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum SpecSurface {
    Chat,
    Cli,
    Desktop,
    Action,
}

response!(
    /// §2.4 `SpecLine`: the run config as display text, each edited value explained (F-005).
    SpecLine {
        surface: SpecSurface,
        r#where: String,
        model: Option<Explained<String>>,
        effort: Option<Explained<String>>,
        mode: Option<Explained<String>>,
        agents: Vec<String>,
        dropped_agents: Vec<String>,
        note: Option<String>,
    }
);

response!(
    /// `StepSummary` (§4.3) with the row facts settled in §2.4.
    StepSummary {
        key: StepKey,
        number: String,
        title: String,
        state: StepState,
        tag: Tag,
        surface: Option<Surface>,
        spec_line: Option<SpecLine>,
        why_not_runnable: Option<String>,
        state_text: String,
        condition_note: Option<String>,
        kind: StepKind,
        run: Option<RunId>,
        next: bool,
    }
);

/// §2.4 `SheetStatus`: a stage's status in the sheet strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum SheetStatus {
    Locked,
    Ready,
    InProgress,
    Inspecting,
    Waiting,
    Signed,
    NotApplicable,
    NeedsAnnotation,
    Undecided,
}

response!(
    /// §2.4 `SignedInfo`.
    SignedInfo {
        date: String,
        commit: String,
        branch: String,
        merged: bool,
    }
);

response!(
    /// §2.4 `PacketSummary`.
    PacketSummary {
        path: RepoPath,
        exists: bool,
        problems: u32,
        signed: bool,
    }
);

response!(
    /// `StageView` (§4.3) with `subtitle`, `status`, `statusWhy` (§2.4).
    StageView {
        stage: Stage,
        title: String,
        applicable: Applicability,
        signed: Option<SignedInfo>,
        steps: Vec<StepSummary>,
        packet: Option<PacketSummary>,
        subtitle: String,
        status: SheetStatus,
        status_why: Option<String>,
        closeout_checklist: Vec<String>,
    }
);

response!(
    /// §2.4 `ConditionView`.
    ConditionView {
        value: TriState,
        locked: bool,
        lock_rule: Option<String>,
        label: String,
        effect: String,
        late: bool,
    }
);

response!(
    /// §2.4 `BlueprintPin`: the pinned blueprint in the title block.
    BlueprintPin { version: String, commit: String }
);

response!(
    /// §2.4 `ConfigChangedNotice`: the config-changed banner (F-008).
    ConfigChangedNotice {
        cwd: CwdKind,
        fingerprint: Sha256,
        blocking: bool,
        title: String,
        detail: String,
        diffs: Vec<RedactedDiff>,
    }
);

response!(
    /// `ProjectView` (§4.3) with the title-block facts (§2.4).
    ProjectView {
        summary: ProjectSummary,
        conditions: BTreeMap<ConditionId, ConditionView>,
        stages: Vec<StageView>,
        pending_changes: u32,
        read_only: Option<ReadOnly>,
        branch: String,
        blueprint: BlueprintPin,
        dri: Option<String>,
        chat_project_url: Option<String>,
        config_changed: Option<ConfigChangedNotice>,
    }
);

/// §2.4 `SchemaState`: a picked folder's `.foreman/` against this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum SchemaState {
    Current,
    Older,
    Newer,
}

response!(
    /// `FolderPick` (§4.3).
    FolderPick {
        pick_id: PickId,
        display_path: String,
        is_git_repo: bool,
        has_foreman: Option<SchemaState>,
        refusal: Option<String>,
    }
);

/// What setup does with a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum PlannedAction {
    Create,
    LeaveAlone,
    Refuse,
}

response!(
    /// `PlannedWrite` (§4.3).
    PlannedWrite {
        path: RepoPath,
        action: PlannedAction,
        reason: Option<String>,
    }
);

response!(
    /// §2.4 `PinPreview`.
    PinPreview {
        version: String,
        commit: String,
        clean: bool,
    }
);

/// §2.4 a row's status in the applicable-path summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum PathRowStatus {
    Applies,
    NotApplicable,
    Undecided,
    NeedsAnnotation,
}

response!(
    /// §2.4 `ApplicablePathRow`.
    ApplicablePathRow {
        stages: String,
        title: String,
        status: PathRowStatus,
        why: String,
    }
);

response!(
    /// §2.4 one stage in the setup preview's strip.
    StripCell {
        stage: Stage,
        status: SheetStatus,
        why: Option<String>,
    }
);

response!(
    /// §2.4 `ApplicablePathSummary`.
    ApplicablePathSummary {
        applies: u32,
        total: u32,
        summary: String,
        rows: Vec<ApplicablePathRow>,
        strip: Vec<StripCell>,
    }
);

response!(
    /// `SetupPreview` (§4.3).
    SetupPreview {
        preview_id: PreviewId,
        writes: Vec<PlannedWrite>,
        git_init: bool,
        pin: PinPreview,
        path: ApplicablePathSummary,
    }
);

response!(
    /// `NextStepRow` (§4.1) with the strip and stage title (§2.4).
    NextStepRow {
        project: ProjectSummary,
        next: Option<StepSummary>,
        why_none: Option<String>,
        strip: Vec<SheetStatus>,
        stage_title: Option<String>,
    }
);

response!(
    /// `TodayView` (§4.1).
    TodayView { rows: Vec<NextStepRow>, inbox: Vec<InboxItem> }
);

/// `DoNextOutcome` (§4.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum DoNextOutcome {
    Nothing {
        why: String,
    },
    Cli {
        project: ProjectId,
        step: StepKey,
        prepare: Box<PrepareOutcome>,
    },
    Chat {
        project: ProjectId,
        step: StepKey,
        ready: bool,
        blockers: Vec<Blocker>,
    },
}
