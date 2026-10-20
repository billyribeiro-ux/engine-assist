//! The command table for integration tests: every command's request and response type
//! (`docs/contracts/ipc.md` §4). Generated from the same table as `src/ipc/handlers.rs`.
#![allow(dead_code, reason = "each test binary uses a subset")]

use foreman_app::ipc::dto::app::{
    AppBootstrap, BlueprintRepoInfo, DoNextOutcome, FolderPick, ProjectSummary, ProjectView,
    SettingsView, SetupPreview, TodayView, ToolPaths,
};
use foreman_app::ipc::dto::doctor::{
    ContractResult, DoctorReport, LoginPathPreview, TemplateHealth,
};
use foreman_app::ipc::dto::gates::{
    Assembler, DesktopOpened, GateView, ImportProposal, LessonsStatus, Manifest, SignPreview,
    SignResult,
};
use foreman_app::ipc::dto::launch::{
    AttachInfo, LinkOutcome, PasteText, PrepareOutcome, RearmOutcome, RunStarted,
};
use foreman_app::ipc::dto::records::{
    ExportResult, MetricsView, PacketView, PendingChange, RebuildPreview, RunsPage,
};
use foreman_app::ipc::dto::requests::{
    ActionApplyReq, ActionPreviewReq, AddItemReq, AddStepReq, AppBootstrapReq,
    AssemblerPasteApplyReq, AssemblerPastePreviewReq, AssemblerViewReq, AuthKeyAllowlistAddReq,
    BlueprintPickRepoReq, BlueprintRepinReq, ChainResumePrepareReq, ChatCopyAndOpenReq,
    ChatLinkSetReq, ClipboardCopyReq, ConditionSetReq, ConfigAckReq, DesktopHandoffReq,
    DesktopOpenReq, DoNextReq, DoctorCaptureLoginPathReq, DoctorLoginPathApplyReq, DoctorRunReq,
    FileOpenReq, FileRevealReq, GateFixStartReq, GateImportConfirmReq, GateImportResultsReq,
    GateItemAttestReq, GateViewReq, ItemKindConfirmReq, LaunchPrepareReq, LaunchStartReq,
    LessonsStatusReq, ManifestMarkReq, ManifestViewReq, MetricsExportReq, MetricsViewReq,
    OptionalPartSetReq, PacketNormalizeApplyReq, PacketRepairPrepareReq, PacketSignatureConfirmReq,
    PacketViewReq, PasteBackApplyReq, PasteBackPreviewReq, PendingChangeAcceptReq,
    PendingChangeRejectReq, PendingChangesListReq, ProbeFolderOpenReq, ProbeRunReq,
    ProgressRebuildApplyReq, ProgressRebuildPreviewReq, ProjectEnvApproveReq, ProjectOpenReq,
    ProjectPinSetReq, ProjectRemoveReq, ProjectUrlSetReq, ProjectsListReq, PtyAttachReq,
    PtyFlowReq, PtyInterruptReq, PtyPasteReq, PtyResizeReq, PtyTerminateReq, PtyWriteReq,
    RearmContinueReq, RearmStartReq, RunInputSetReq, RunOverrideSetReq, RunSheetExportReq,
    RunsClearReq, RunsSearchReq, SettingsGetReq, SettingsPickToolReq, SettingsUpdateReq,
    SetupApplyReq, SetupPickFolderReq, SetupPreviewReq, SignApplyReq, SignPreviewReq,
    SignRetryCommitReq, StepMarkDoneReq, StepSkipReq, StepViewReq, TamperRestoreReq,
    TemplateHealthReq, TerminalLinkOpenReq, TerminalOpenHereReq, TodayGetReq, VariableSetReq,
    WaitStartReq,
};
use foreman_app::ipc::dto::steps::{
    ActionPreview, ChatCopyResult, CopyResult, PasteApplyResult, PastePreview, StepView,
};
use foreman_app::ipc::types::Rev;
use serde::Serialize;
use serde::de::DeserializeOwned;

fn check<T: DeserializeOwned>(json: &str) -> Result<(), String> {
    serde_json::from_str::<T>(json)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn roundtrip<T: DeserializeOwned + Serialize>(json: &str) -> Result<(), String> {
    let typed: T = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let original: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let back = serde_json::to_value(&typed).map_err(|e| e.to_string())?;
    if back == original {
        Ok(())
    } else {
        Err(format!(
            "re-serialized differently:\n{back:#}\nvs\n{original:#}"
        ))
    }
}

/// Deserializes a request fixture into the command's request struct.
pub fn request(cmd: &str, json: &str) -> Result<(), String> {
    match cmd {
        "app_bootstrap" => check::<AppBootstrapReq>(json),
        "settings_get" => check::<SettingsGetReq>(json),
        "settings_update" => check::<SettingsUpdateReq>(json),
        "settings_pick_tool" => check::<SettingsPickToolReq>(json),
        "today_get" => check::<TodayGetReq>(json),
        "do_next" => check::<DoNextReq>(json),
        "blueprint_pick_repo" => check::<BlueprintPickRepoReq>(json),
        "template_health" => check::<TemplateHealthReq>(json),
        "doctor_run" => check::<DoctorRunReq>(json),
        "doctor_capture_login_path" => check::<DoctorCaptureLoginPathReq>(json),
        "doctor_login_path_apply" => check::<DoctorLoginPathApplyReq>(json),
        "probe_folder_open" => check::<ProbeFolderOpenReq>(json),
        "probe_run" => check::<ProbeRunReq>(json),
        "projects_list" => check::<ProjectsListReq>(json),
        "setup_pick_folder" => check::<SetupPickFolderReq>(json),
        "setup_preview" => check::<SetupPreviewReq>(json),
        "setup_apply" => check::<SetupApplyReq>(json),
        "project_open" => check::<ProjectOpenReq>(json),
        "project_url_set" => check::<ProjectUrlSetReq>(json),
        "condition_set" => check::<ConditionSetReq>(json),
        "variable_set" => check::<VariableSetReq>(json),
        "blueprint_repin" => check::<BlueprintRepinReq>(json),
        "project_remove" => check::<ProjectRemoveReq>(json),
        "project_pin_set" => check::<ProjectPinSetReq>(json),
        "step_view" => check::<StepViewReq>(json),
        "run_input_set" => check::<RunInputSetReq>(json),
        "optional_part_set" => check::<OptionalPartSetReq>(json),
        "run_override_set" => check::<RunOverrideSetReq>(json),
        "step_mark_done" => check::<StepMarkDoneReq>(json),
        "step_skip" => check::<StepSkipReq>(json),
        "action_preview" => check::<ActionPreviewReq>(json),
        "action_apply" => check::<ActionApplyReq>(json),
        "wait_start" => check::<WaitStartReq>(json),
        "clipboard_copy" => check::<ClipboardCopyReq>(json),
        "chat_copy_and_open" => check::<ChatCopyAndOpenReq>(json),
        "chat_link_set" => check::<ChatLinkSetReq>(json),
        "paste_back_preview" => check::<PasteBackPreviewReq>(json),
        "paste_back_apply" => check::<PasteBackApplyReq>(json),
        "launch_prepare" => check::<LaunchPrepareReq>(json),
        "chain_resume_prepare" => check::<ChainResumePrepareReq>(json),
        "launch_start" => check::<LaunchStartReq>(json),
        "config_ack" => check::<ConfigAckReq>(json),
        "auth_key_allowlist_add" => check::<AuthKeyAllowlistAddReq>(json),
        "project_env_approve" => check::<ProjectEnvApproveReq>(json),
        "pty_attach" => check::<PtyAttachReq>(json),
        "pty_write" => check::<PtyWriteReq>(json),
        "pty_paste" => check::<PtyPasteReq>(json),
        "pty_resize" => check::<PtyResizeReq>(json),
        "pty_flow" => check::<PtyFlowReq>(json),
        "pty_interrupt" => check::<PtyInterruptReq>(json),
        "pty_terminate" => check::<PtyTerminateReq>(json),
        "rearm_start" => check::<RearmStartReq>(json),
        "rearm_continue" => check::<RearmContinueReq>(json),
        "terminal_link_open" => check::<TerminalLinkOpenReq>(json),
        "file_open" => check::<FileOpenReq>(json),
        "file_reveal" => check::<FileRevealReq>(json),
        "terminal_open_here" => check::<TerminalOpenHereReq>(json),
        "pending_changes_list" => check::<PendingChangesListReq>(json),
        "pending_change_accept" => check::<PendingChangeAcceptReq>(json),
        "pending_change_reject" => check::<PendingChangeRejectReq>(json),
        "item_kind_confirm" => check::<ItemKindConfirmReq>(json),
        "progress_rebuild_preview" => check::<ProgressRebuildPreviewReq>(json),
        "progress_rebuild_apply" => check::<ProgressRebuildApplyReq>(json),
        "packet_view" => check::<PacketViewReq>(json),
        "packet_normalize_apply" => check::<PacketNormalizeApplyReq>(json),
        "packet_signature_confirm" => check::<PacketSignatureConfirmReq>(json),
        "packet_repair_prepare" => check::<PacketRepairPrepareReq>(json),
        "metrics_export" => check::<MetricsExportReq>(json),
        "metrics_view" => check::<MetricsViewReq>(json),
        "runs_clear" => check::<RunsClearReq>(json),
        "runs_search" => check::<RunsSearchReq>(json),
        "gate_view" => check::<GateViewReq>(json),
        "gate_item_attest" => check::<GateItemAttestReq>(json),
        "gate_import_results" => check::<GateImportResultsReq>(json),
        "gate_import_confirm" => check::<GateImportConfirmReq>(json),
        "gate_fix_start" => check::<GateFixStartReq>(json),
        "sign_preview" => check::<SignPreviewReq>(json),
        "sign_apply" => check::<SignApplyReq>(json),
        "sign_retry_commit" => check::<SignRetryCommitReq>(json),
        "tamper_restore" => check::<TamperRestoreReq>(json),
        "lessons_status" => check::<LessonsStatusReq>(json),
        "manifest_view" => check::<ManifestViewReq>(json),
        "manifest_mark" => check::<ManifestMarkReq>(json),
        "assembler_view" => check::<AssemblerViewReq>(json),
        "assembler_paste_preview" => check::<AssemblerPastePreviewReq>(json),
        "assembler_paste_apply" => check::<AssemblerPasteApplyReq>(json),
        "desktop_open" => check::<DesktopOpenReq>(json),
        "desktop_handoff" => check::<DesktopHandoffReq>(json),
        "add_step" => check::<AddStepReq>(json),
        "add_item" => check::<AddItemReq>(json),
        "run_sheet_export" => check::<RunSheetExportReq>(json),
        other => Err(format!("unknown command {other}")),
    }
}

/// Round-trips a response fixture through the command's response type.
pub fn response(cmd: &str, json: &str) -> Result<(), String> {
    match cmd {
        "app_bootstrap" => roundtrip::<AppBootstrap>(json),
        "settings_get" => roundtrip::<SettingsView>(json),
        "settings_update" => roundtrip::<SettingsView>(json),
        "settings_pick_tool" => roundtrip::<ToolPaths>(json),
        "today_get" => roundtrip::<TodayView>(json),
        "do_next" => roundtrip::<DoNextOutcome>(json),
        "blueprint_pick_repo" => roundtrip::<BlueprintRepoInfo>(json),
        "template_health" => roundtrip::<TemplateHealth>(json),
        "doctor_run" => roundtrip::<DoctorReport>(json),
        "doctor_capture_login_path" => roundtrip::<LoginPathPreview>(json),
        "doctor_login_path_apply" => roundtrip::<SettingsView>(json),
        "probe_folder_open" => roundtrip::<RunStarted>(json),
        "probe_run" => roundtrip::<ContractResult>(json),
        "projects_list" => roundtrip::<Vec<ProjectSummary>>(json),
        "setup_pick_folder" => roundtrip::<FolderPick>(json),
        "setup_preview" => roundtrip::<SetupPreview>(json),
        "setup_apply" => roundtrip::<ProjectSummary>(json),
        "project_open" => roundtrip::<ProjectView>(json),
        "project_url_set" => roundtrip::<Rev>(json),
        "condition_set" => roundtrip::<ProjectView>(json),
        "variable_set" => roundtrip::<ProjectView>(json),
        "blueprint_repin" => roundtrip::<ProjectView>(json),
        "project_remove" => roundtrip::<Vec<ProjectSummary>>(json),
        "project_pin_set" => roundtrip::<Vec<ProjectSummary>>(json),
        "step_view" => roundtrip::<StepView>(json),
        "run_input_set" => roundtrip::<StepView>(json),
        "optional_part_set" => roundtrip::<StepView>(json),
        "run_override_set" => roundtrip::<StepView>(json),
        "step_mark_done" => roundtrip::<StepView>(json),
        "step_skip" => roundtrip::<ProjectView>(json),
        "action_preview" => roundtrip::<ActionPreview>(json),
        "action_apply" => roundtrip::<StepView>(json),
        "wait_start" => roundtrip::<StepView>(json),
        "clipboard_copy" => roundtrip::<CopyResult>(json),
        "chat_copy_and_open" => roundtrip::<ChatCopyResult>(json),
        "chat_link_set" => roundtrip::<Rev>(json),
        "paste_back_preview" => roundtrip::<PastePreview>(json),
        "paste_back_apply" => roundtrip::<PasteApplyResult>(json),
        "launch_prepare" => roundtrip::<PrepareOutcome>(json),
        "chain_resume_prepare" => roundtrip::<PrepareOutcome>(json),
        "launch_start" => roundtrip::<RunStarted>(json),
        "config_ack" => roundtrip::<()>(json),
        "auth_key_allowlist_add" => roundtrip::<SettingsView>(json),
        "project_env_approve" => roundtrip::<()>(json),
        "pty_attach" => roundtrip::<AttachInfo>(json),
        "pty_write" => roundtrip::<()>(json),
        "pty_paste" => roundtrip::<PasteText>(json),
        "pty_resize" => roundtrip::<()>(json),
        "pty_flow" => roundtrip::<()>(json),
        "pty_interrupt" => roundtrip::<()>(json),
        "pty_terminate" => roundtrip::<()>(json),
        "rearm_start" => roundtrip::<RearmOutcome>(json),
        "rearm_continue" => roundtrip::<RearmOutcome>(json),
        "terminal_link_open" => roundtrip::<LinkOutcome>(json),
        "file_open" => roundtrip::<()>(json),
        "file_reveal" => roundtrip::<()>(json),
        "terminal_open_here" => roundtrip::<()>(json),
        "pending_changes_list" => roundtrip::<Vec<PendingChange>>(json),
        "pending_change_accept" => roundtrip::<ProjectView>(json),
        "pending_change_reject" => roundtrip::<ProjectView>(json),
        "item_kind_confirm" => roundtrip::<GateView>(json),
        "progress_rebuild_preview" => roundtrip::<RebuildPreview>(json),
        "progress_rebuild_apply" => roundtrip::<ProjectView>(json),
        "packet_view" => roundtrip::<PacketView>(json),
        "packet_normalize_apply" => roundtrip::<PacketView>(json),
        "packet_signature_confirm" => roundtrip::<PacketView>(json),
        "packet_repair_prepare" => roundtrip::<PrepareOutcome>(json),
        "metrics_export" => roundtrip::<ExportResult>(json),
        "metrics_view" => roundtrip::<MetricsView>(json),
        "runs_clear" => roundtrip::<()>(json),
        "runs_search" => roundtrip::<RunsPage>(json),
        "gate_view" => roundtrip::<GateView>(json),
        "gate_item_attest" => roundtrip::<GateView>(json),
        "gate_import_results" => roundtrip::<Vec<ImportProposal>>(json),
        "gate_import_confirm" => roundtrip::<GateView>(json),
        "gate_fix_start" => roundtrip::<PrepareOutcome>(json),
        "sign_preview" => roundtrip::<SignPreview>(json),
        "sign_apply" => roundtrip::<SignResult>(json),
        "sign_retry_commit" => roundtrip::<SignResult>(json),
        "tamper_restore" => roundtrip::<PacketView>(json),
        "lessons_status" => roundtrip::<LessonsStatus>(json),
        "manifest_view" => roundtrip::<Manifest>(json),
        "manifest_mark" => roundtrip::<Manifest>(json),
        "assembler_view" => roundtrip::<Assembler>(json),
        "assembler_paste_preview" => roundtrip::<PastePreview>(json),
        "assembler_paste_apply" => roundtrip::<Assembler>(json),
        "desktop_open" => roundtrip::<DesktopOpened>(json),
        "desktop_handoff" => roundtrip::<()>(json),
        "add_step" => roundtrip::<ProjectView>(json),
        "add_item" => roundtrip::<GateView>(json),
        "run_sheet_export" => roundtrip::<ExportResult>(json),
        other => Err(format!("unknown command {other}")),
    }
}

/// Commands that take Channel arguments, with their argument names (§5).
pub fn channels(cmd: &str) -> &'static [&'static str] {
    match cmd {
        "probe_folder_open" => &["output"],
        "probe_run" => &["progress"],
        "launch_prepare" => &["progress"],
        "chain_resume_prepare" => &["progress"],
        "launch_start" => &["output"],
        "pty_attach" => &["output"],
        "rearm_start" => &["output", "progress"],
        "rearm_continue" => &["output", "progress"],
        "packet_repair_prepare" => &["progress"],
        "gate_fix_start" => &["progress"],
        "sign_apply" => &["progress"],
        "sign_retry_commit" => &["progress"],
        "desktop_open" => &["progress"],
        _ => &[],
    }
}
