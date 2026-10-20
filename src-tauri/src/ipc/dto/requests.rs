//! The 91 request structs (`docs/contracts/ipc.md` §4), one per command. Every struct is
//! `deny_unknown_fields` + `camelCase`, without `flatten` or `untagged` (IPC-R1), and has no field
//! that means approved, confirmed, consented or acknowledged (IPC-R11).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ipc::types::{
    BundleId, ChainId, ChangeId, ClaudeUrl, ConditionId, CopyTarget, CwdKind, Empty, EntryPoint,
    EnvName, HitId, InputValue, IsoDate, ItemId, Note, PasteKind, PickId, PlanId, PreviewId,
    ProjectId, PtyData, RepoPath, Rev, RunId, RunOverride, Sha256, ShortText, Stage, Start,
    StepKey, Surface, Theme, ToolKind, TriState,
};

/// Declares a request struct with the IPC-R1 attributes and ts-rs export.
macro_rules! request {
    ($(#[$m:meta])* $name:ident { $($(#[$fm:meta])* $f:ident : $t:ty),* $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields, rename_all = "camelCase")]
        #[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
        pub struct $name { $($(#[$fm])* pub $f: $t),* }
    };
}

// §4.1 App
request!(
    /// `app_bootstrap`.
    AppBootstrapReq {}
);
request!(
    /// `settings_get`.
    SettingsGetReq {}
);

/// `editor` in a settings patch: set a bundle ID or clear it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum EditorSetting {
    Set(EditorSet),
    Clear(Empty),
}

/// `{ kind: "set", bundleId }`. Variant payloads are structs with `deny_unknown_fields`, so an
/// extra field inside a tagged object is `Invalid` too (IPC-R1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct EditorSet {
    pub bundle_id: BundleId,
}

/// Terminal preferences (`font_size` 9–32, `scrollback` 1,000–100,000).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct TerminalPrefs {
    pub font_size: u8,
    pub scrollback: u32,
    pub mac_option_is_meta: bool,
}

/// What the webview's media queries report (not stored; drives window vibrancy, AC-XC-23).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct OsDisplay {
    pub reduce_transparency: bool,
    pub increase_contrast: bool,
}

/// A settings patch: every `null` means "unchanged".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SettingsPatch {
    pub dri_name: Option<ShortText>,
    pub editor: Option<EditorSetting>,
    pub theme: Option<Theme>,
    pub terminal: Option<TerminalPrefs>,
    pub env_passthrough: Option<Vec<EnvName>>,
    #[cfg_attr(feature = "ts", ts(type = "Record<string, string> | null"))]
    pub global_variables: Option<BTreeMap<String, ShortText>>,
    pub reset_tool: Option<ToolKind>,
    pub clear_login_path: Option<bool>,
    pub remove_allowlist_entry: Option<Sha256>,
    pub screen_reader_mode: Option<bool>,
    pub os_display: Option<OsDisplay>,
}

request!(
    /// `settings_update`.
    SettingsUpdateReq { expect_rev: Rev, patch: SettingsPatch }
);
request!(
    /// `settings_pick_tool`: Rust opens the file dialog; the path never comes from the webview.
    SettingsPickToolReq { tool: ToolKind }
);
request!(
    /// `today_get`.
    TodayGetReq {}
);
request!(
    /// `do_next`.
    DoNextReq {}
);

// §4.2 First run and Doctor
/// `autoDetect` (fixed candidates only) or `choose` (Rust dialog).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum PickMode {
    AutoDetect,
    Choose,
}

/// Doctor scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum DoctorScope {
    P1,
    Full,
}

request!(
    /// `blueprint_pick_repo`.
    BlueprintPickRepoReq { mode: PickMode }
);
request!(
    /// `template_health`: `None` = the blueprint in Settings.
    TemplateHealthReq { project_id: Option<ProjectId> }
);
request!(
    /// `doctor_run`.
    DoctorRunReq { scope: DoctorScope }
);
request!(
    /// `doctor_capture_login_path` (Rust asks for consent in a native dialog first).
    DoctorCaptureLoginPathReq {}
);
request!(
    /// `doctor_login_path_apply`.
    DoctorLoginPathApplyReq { preview_id: PreviewId }
);
request!(
    /// `probe_folder_open`.
    ProbeFolderOpenReq {}
);
request!(
    /// `probe_run`.
    ProbeRunReq {}
);

// §4.3 Projects
request!(
    /// `projects_list`.
    ProjectsListReq {}
);
request!(
    /// `setup_pick_folder`.
    SetupPickFolderReq {}
);
request!(
    /// `setup_preview`.
    SetupPreviewReq {
        pick_id: PickId,
        name: ShortText,
        slug: ShortText,
        #[cfg_attr(feature = "ts", ts(type = "Record<string, string>"))]
        variables: BTreeMap<String, ShortText>,
        conditions: BTreeMap<ConditionId, TriState>,
        chat_project_url: Option<ClaudeUrl>,
    }
);
request!(
    /// `setup_apply`.
    SetupApplyReq { preview_id: PreviewId }
);
request!(
    /// `project_open`.
    ProjectOpenReq { project_id: ProjectId }
);
request!(
    /// `project_url_set`.
    ProjectUrlSetReq { project_id: ProjectId, url: Option<ClaudeUrl> }
);
request!(
    /// `condition_set`.
    ConditionSetReq { project_id: ProjectId, condition: ConditionId, to: TriState, note: Note }
);
request!(
    /// `variable_set`.
    VariableSetReq { project_id: ProjectId, name: ShortText, value: ShortText }
);
request!(
    /// `blueprint_repin`.
    BlueprintRepinReq { project_id: ProjectId }
);
request!(
    /// `project_remove` (native confirmation names what is deleted).
    ProjectRemoveReq { project_id: ProjectId }
);
request!(
    /// `project_pin_set`: slot 1–9; `None` unpins.
    ProjectPinSetReq { project_id: ProjectId, slot: Option<u8> }
);

// §4.4 Steps
request!(
    /// `step_view`.
    StepViewReq { project_id: ProjectId, step: StepKey }
);
request!(
    /// `run_input_set`.
    RunInputSetReq { project_id: ProjectId, step: StepKey, name: ShortText, value: InputValue }
);
request!(
    /// `optional_part_set`.
    OptionalPartSetReq { project_id: ProjectId, step: StepKey, part_id: ShortText, include: bool }
);
request!(
    /// `run_override_set`: `None` clears.
    RunOverrideSetReq { project_id: ProjectId, step: StepKey, over: Option<RunOverride> }
);
request!(
    /// `step_mark_done`.
    StepMarkDoneReq { project_id: ProjectId, step: StepKey }
);
request!(
    /// `step_skip`.
    StepSkipReq { project_id: ProjectId, step: StepKey, reason: ShortText }
);
request!(
    /// `action_preview`.
    ActionPreviewReq { project_id: ProjectId, step: StepKey }
);
request!(
    /// `action_apply`.
    ActionApplyReq { preview_id: PreviewId }
);
request!(
    /// `wait_start`.
    WaitStartReq { project_id: ProjectId, step: StepKey, until: IsoDate }
);
request!(
    /// `clipboard_copy`: one command for every copy target.
    ClipboardCopyReq {
        project_id: ProjectId,
        target: CopyTarget,
        step: Option<StepKey>,
        chain: Option<ChainId>,
        path: Option<RepoPath>,
        stage: Option<Stage>,
    }
);

// §4.5 Chat
/// Where a chat link comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum LinkSource {
    Clipboard(Empty),
    Text(LinkText),
}

/// `{ kind: "text", url }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct LinkText {
    pub url: ClaudeUrl,
}

/// Where a paste-back comes from (`sensor`: P2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum PasteSource {
    Clipboard,
    Sensor,
}

request!(
    /// `chat_copy_and_open`.
    ChatCopyAndOpenReq { project_id: ProjectId, step: StepKey, entry: EntryPoint }
);
request!(
    /// `chat_link_set`.
    ChatLinkSetReq { project_id: ProjectId, chain_start: StepKey, source: LinkSource }
);
request!(
    /// `paste_back_preview`.
    PasteBackPreviewReq { project_id: ProjectId, step: StepKey, kind: PasteKind, source: PasteSource }
);
request!(
    /// `paste_back_apply`.
    PasteBackApplyReq { preview_id: PreviewId }
);

// §4.6 Launch
/// `normal` or a fresh chain whose prompt starts with the last signed packet (AC-F010-09).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum PrepareMode {
    Normal,
    FreshWithContext,
}

request!(
    /// `launch_prepare`.
    LaunchPrepareReq { project_id: ProjectId, step: StepKey, entry: EntryPoint, mode: PrepareMode }
);
request!(
    /// `chain_resume_prepare`.
    ChainResumePrepareReq { chain: ChainId }
);
request!(
    /// `launch_start`: confirmation reasons are shown by Rust in a native dialog.
    LaunchStartReq { plan_id: PlanId }
);
request!(
    /// `config_ack`: Rust shows the native acknowledgement.
    ConfigAckReq { project_id: ProjectId, cwd: CwdKind, fingerprint: Sha256 }
);
request!(
    /// `auth_key_allowlist_add`.
    AuthKeyAllowlistAddReq { hit: HitId }
);
request!(
    /// `project_env_approve` (only on the S-7′ fail branch).
    ProjectEnvApproveReq { project_id: ProjectId, cwd: CwdKind, sha256: Sha256 }
);

// §4.7 Terminal
request!(
    /// `pty_attach`.
    PtyAttachReq { run_id: RunId }
);
request!(
    /// `pty_write`.
    PtyWriteReq { run_id: RunId, data: PtyData }
);
request!(
    /// `pty_paste`.
    PtyPasteReq { run_id: RunId }
);
request!(
    /// `pty_resize`: 2–1000 cols, 2–500 rows.
    PtyResizeReq { run_id: RunId, cols: u16, rows: u16 }
);
request!(
    /// `pty_flow`.
    PtyFlowReq { run_id: RunId, paused: bool }
);
request!(
    /// `pty_interrupt`.
    PtyInterruptReq { run_id: RunId }
);
request!(
    /// `pty_terminate` (native confirmation).
    PtyTerminateReq { run_id: RunId }
);
request!(
    /// `rearm_start` (native idle dialog in P1).
    RearmStartReq { plan_id: PlanId }
);
request!(
    /// `rearm_continue`.
    RearmContinueReq { chain: ChainId }
);
request!(
    /// `terminal_link_open`: ≤ 8 KB, parsed in Rust.
    TerminalLinkOpenReq { run_id: RunId, url: String }
);

// §4.8 Files
request!(
    /// `file_open`.
    FileOpenReq { project_id: ProjectId, path: RepoPath }
);
request!(
    /// `file_reveal`.
    FileRevealReq { project_id: ProjectId, path: RepoPath }
);
request!(
    /// `terminal_open_here`.
    TerminalOpenHereReq { project_id: ProjectId }
);

// §4.9 Decisions and recovery
request!(
    /// `pending_changes_list`.
    PendingChangesListReq { project_id: ProjectId }
);
request!(
    /// `pending_change_accept` (native dialog).
    PendingChangeAcceptReq { project_id: ProjectId, change: ChangeId, seen_sha256: Sha256 }
);
request!(
    /// `pending_change_reject`.
    PendingChangeRejectReq { project_id: ProjectId, change: ChangeId, seen_sha256: Sha256 }
);
request!(
    /// `item_kind_confirm` (native dialog).
    ItemKindConfirmReq { project_id: ProjectId, item: ItemId, text_sha256: Sha256 }
);
request!(
    /// `progress_rebuild_preview`.
    ProgressRebuildPreviewReq { project_id: ProjectId }
);
request!(
    /// `progress_rebuild_apply`.
    ProgressRebuildApplyReq { preview_id: PreviewId }
);

// §4.10 Packets and run log
request!(
    /// `packet_view`.
    PacketViewReq { project_id: ProjectId, stage: Stage }
);
request!(
    /// `packet_normalize_apply`.
    PacketNormalizeApplyReq { project_id: ProjectId, stage: Stage, seen_sha256: Sha256 }
);
request!(
    /// `packet_signature_confirm` (native dialog).
    PacketSignatureConfirmReq { project_id: ProjectId, stage: Stage, seen_sha256: Sha256 }
);
request!(
    /// `packet_repair_prepare`.
    PacketRepairPrepareReq { project_id: ProjectId, stage: Stage }
);
request!(
    /// `metrics_export` (path from the Rust save dialog).
    MetricsExportReq {}
);
request!(
    /// `metrics_view`: `None` = every project.
    MetricsViewReq { project_id: Option<ProjectId> }
);
request!(
    /// `runs_clear` (native confirmation).
    RunsClearReq {}
);
request!(
    /// `runs_search`: `limit` ≤ 200.
    RunsSearchReq { query: ShortText, project_id: Option<ProjectId>, cursor: Option<ShortText>, limit: u16 }
);

// §4.11 Gates
/// A gate item attestation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum AttestOutcome {
    Done(AttestDone),
    NotApplicable(AttestNotApplicable),
    Clear(Empty),
}

/// `{ kind: "done", note, link }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct AttestDone {
    pub note: Note,
    pub link: Option<String>,
}

/// `{ kind: "notApplicable", reason }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct AttestNotApplicable {
    pub reason: Note,
}

/// A Gate A result Billy confirms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum PassFail {
    Pass,
    Fail,
}

request!(
    /// `gate_view`.
    GateViewReq { project_id: ProjectId, stage: Stage }
);
request!(
    /// `gate_item_attest`.
    GateItemAttestReq { project_id: ProjectId, stage: Stage, item: ItemId, outcome: AttestOutcome }
);
request!(
    /// `gate_import_results`.
    GateImportResultsReq { project_id: ProjectId, stage: Stage }
);
request!(
    /// `gate_import_confirm`.
    GateImportConfirmReq { project_id: ProjectId, stage: Stage, item: ItemId, result: PassFail, evidence: Note }
);
request!(
    /// `gate_fix_start`.
    GateFixStartReq { project_id: ProjectId, stage: Stage, item: ItemId }
);

// §4.12 Signing
/// Which branch the signing commit lands on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum SignBranch {
    Current(Empty),
    Other(SignOther),
}

/// `{ kind: "other", branch }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SignOther {
    pub branch: ShortText,
}

request!(
    /// `sign_preview`.
    SignPreviewReq { project_id: ProjectId, stage: Stage, branch: SignBranch, name: ShortText, date: IsoDate }
);
request!(
    /// `sign_apply` (native signing dialog).
    SignApplyReq { preview_id: PreviewId }
);
request!(
    /// `sign_retry_commit`.
    SignRetryCommitReq { project_id: ProjectId, stage: Stage }
);
request!(
    /// `tamper_restore`.
    TamperRestoreReq { project_id: ProjectId, stage: Stage, seen_sha256: Sha256 }
);

// §4.13 Lessons, manifest, assembler
request!(
    /// `lessons_status`.
    LessonsStatusReq { project_id: ProjectId, stage: Stage }
);
request!(
    /// `manifest_view`.
    ManifestViewReq { project_id: ProjectId }
);
request!(
    /// `manifest_mark`.
    ManifestMarkReq { project_id: ProjectId, path: RepoPath, in_project: bool }
);
request!(
    /// `assembler_view`.
    AssemblerViewReq { project_id: ProjectId }
);
request!(
    /// `assembler_paste_preview`.
    AssemblerPastePreviewReq { project_id: ProjectId, path: RepoPath }
);
request!(
    /// `assembler_paste_apply` (root `CLAUDE.md`: native dialog).
    AssemblerPasteApplyReq { preview_id: PreviewId }
);

// §4.14 Desktop, overlay additions, run sheet
/// `optional` or `core` (F-025).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum AddedTag {
    Optional,
    Core,
}

/// An added step's run config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct AddedStepConfig {
    pub surface: Surface,
    pub cwd: Option<CwdKind>,
    pub start: Start,
    pub out: Vec<RepoPath>,
}

request!(
    /// `desktop_open`.
    DesktopOpenReq { project_id: ProjectId, step: StepKey }
);
request!(
    /// `desktop_handoff`.
    DesktopHandoffReq { chain: ChainId }
);
request!(
    /// `add_step`.
    AddStepReq {
        project_id: ProjectId,
        stage: Stage,
        after: StepKey,
        copy_config_from: StepKey,
        title: ShortText,
        text: Note,
        config: AddedStepConfig,
        tag: AddedTag,
    }
);
request!(
    /// `add_item`.
    AddItemReq { project_id: ProjectId, stage: Stage, text: ShortText }
);
request!(
    /// `run_sheet_export`.
    RunSheetExportReq { project_id: ProjectId, stage: Stage }
);
