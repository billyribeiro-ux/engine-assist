//! The 91 Tauri command entry points (`docs/contracts/ipc.md` §4), one thin wrapper per command:
//! the request is checked by [`Req`] (IPC-R1), then the handler in `ipc::commands::<group>` runs
//! on the blocking pool with panics caught (IPC-R3). `generate_handler!` in `lib.rs` lists exactly
//! these (`capabilities::every_command_declared_and_granted`).
#![allow(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]

use crate::ipc::dto::app::{
    AppBootstrap, BlueprintRepoInfo, DoNextOutcome, FolderPick, ProjectSummary, ProjectView,
    SettingsView, SetupPreview, TodayView, ToolPaths,
};
use crate::ipc::dto::doctor::{ContractResult, DoctorReport, LoginPathPreview, TemplateHealth};
use crate::ipc::dto::gates::{
    Assembler, DesktopOpened, GateView, ImportProposal, LessonsStatus, Manifest, SignPreview,
    SignResult,
};
use crate::ipc::dto::launch::{
    AttachInfo, LaunchProgress, LinkOutcome, PasteText, PrepareOutcome, RearmOutcome, RunStarted,
    SignProgress,
};
use crate::ipc::dto::records::{
    ExportResult, MetricsView, PacketView, PendingChange, RebuildPreview, RunsPage,
};
use crate::ipc::dto::requests::{
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
use crate::ipc::dto::steps::{
    ActionPreview, ChatCopyResult, CopyResult, PasteApplyResult, PastePreview, StepView,
};
use crate::ipc::types::Rev;
use crate::ipc::{IpcError, Req, Shared, call, commands};

/// `app_bootstrap`.
#[tauri::command]
pub async fn app_bootstrap(
    state: tauri::State<'_, Shared>,
    req: Req<AppBootstrapReq>,
) -> Result<AppBootstrap, IpcError> {
    call(&state, req, commands::app::app_bootstrap).await
}

/// `settings_get`.
#[tauri::command]
pub async fn settings_get(
    state: tauri::State<'_, Shared>,
    req: Req<SettingsGetReq>,
) -> Result<SettingsView, IpcError> {
    call(&state, req, commands::settings::settings_get).await
}

/// `settings_update`.
#[tauri::command]
pub async fn settings_update(
    state: tauri::State<'_, Shared>,
    req: Req<SettingsUpdateReq>,
) -> Result<SettingsView, IpcError> {
    call(&state, req, commands::settings::settings_update).await
}

/// `settings_pick_tool`.
#[tauri::command]
pub async fn settings_pick_tool(
    state: tauri::State<'_, Shared>,
    req: Req<SettingsPickToolReq>,
) -> Result<ToolPaths, IpcError> {
    call(&state, req, commands::settings::settings_pick_tool).await
}

/// `today_get`.
#[tauri::command]
pub async fn today_get(
    state: tauri::State<'_, Shared>,
    req: Req<TodayGetReq>,
) -> Result<TodayView, IpcError> {
    call(&state, req, commands::app::today_get).await
}

/// `do_next`.
#[tauri::command]
pub async fn do_next(
    state: tauri::State<'_, Shared>,
    req: Req<DoNextReq>,
) -> Result<DoNextOutcome, IpcError> {
    call(&state, req, commands::app::do_next).await
}

/// `blueprint_pick_repo`.
#[tauri::command]
pub async fn blueprint_pick_repo(
    state: tauri::State<'_, Shared>,
    req: Req<BlueprintPickRepoReq>,
) -> Result<BlueprintRepoInfo, IpcError> {
    call(&state, req, commands::first_run::blueprint_pick_repo).await
}

/// `template_health`.
#[tauri::command]
pub async fn template_health(
    state: tauri::State<'_, Shared>,
    req: Req<TemplateHealthReq>,
) -> Result<TemplateHealth, IpcError> {
    call(&state, req, commands::first_run::template_health).await
}

/// `doctor_run`.
#[tauri::command]
pub async fn doctor_run(
    state: tauri::State<'_, Shared>,
    req: Req<DoctorRunReq>,
) -> Result<DoctorReport, IpcError> {
    call(&state, req, commands::doctor::doctor_run).await
}

/// `doctor_capture_login_path`.
#[tauri::command]
pub async fn doctor_capture_login_path(
    state: tauri::State<'_, Shared>,
    req: Req<DoctorCaptureLoginPathReq>,
) -> Result<LoginPathPreview, IpcError> {
    call(&state, req, commands::doctor::doctor_capture_login_path).await
}

/// `doctor_login_path_apply`.
#[tauri::command]
pub async fn doctor_login_path_apply(
    state: tauri::State<'_, Shared>,
    req: Req<DoctorLoginPathApplyReq>,
) -> Result<SettingsView, IpcError> {
    call(&state, req, commands::doctor::doctor_login_path_apply).await
}

/// `probe_folder_open`.
#[tauri::command]
pub async fn probe_folder_open(
    state: tauri::State<'_, Shared>,
    req: Req<ProbeFolderOpenReq>,
    output: tauri::ipc::Channel<tauri::ipc::Response>,
) -> Result<RunStarted, IpcError> {
    call(&state, req, move |a, r| {
        commands::doctor::probe_folder_open(a, r, output)
    })
    .await
}

/// `probe_run`.
#[tauri::command]
pub async fn probe_run(
    state: tauri::State<'_, Shared>,
    req: Req<ProbeRunReq>,
    progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<ContractResult, IpcError> {
    call(&state, req, move |a, r| {
        commands::doctor::probe_run(a, r, progress)
    })
    .await
}

/// `projects_list`.
#[tauri::command]
pub async fn projects_list(
    state: tauri::State<'_, Shared>,
    req: Req<ProjectsListReq>,
) -> Result<Vec<ProjectSummary>, IpcError> {
    call(&state, req, commands::projects::projects_list).await
}

/// `setup_pick_folder`.
#[tauri::command]
pub async fn setup_pick_folder(
    state: tauri::State<'_, Shared>,
    req: Req<SetupPickFolderReq>,
) -> Result<FolderPick, IpcError> {
    call(&state, req, commands::projects::setup_pick_folder).await
}

/// `setup_preview`.
#[tauri::command]
pub async fn setup_preview(
    state: tauri::State<'_, Shared>,
    req: Req<SetupPreviewReq>,
) -> Result<SetupPreview, IpcError> {
    call(&state, req, commands::projects::setup_preview).await
}

/// `setup_apply`.
#[tauri::command]
pub async fn setup_apply(
    state: tauri::State<'_, Shared>,
    req: Req<SetupApplyReq>,
) -> Result<ProjectSummary, IpcError> {
    call(&state, req, commands::projects::setup_apply).await
}

/// `project_open`.
#[tauri::command]
pub async fn project_open(
    state: tauri::State<'_, Shared>,
    req: Req<ProjectOpenReq>,
) -> Result<ProjectView, IpcError> {
    call(&state, req, commands::projects::project_open).await
}

/// `project_url_set`.
#[tauri::command]
pub async fn project_url_set(
    state: tauri::State<'_, Shared>,
    req: Req<ProjectUrlSetReq>,
) -> Result<Rev, IpcError> {
    call(&state, req, commands::projects::project_url_set).await
}

/// `condition_set`.
#[tauri::command]
pub async fn condition_set(
    state: tauri::State<'_, Shared>,
    req: Req<ConditionSetReq>,
) -> Result<ProjectView, IpcError> {
    call(&state, req, commands::projects::condition_set).await
}

/// `variable_set`.
#[tauri::command]
pub async fn variable_set(
    state: tauri::State<'_, Shared>,
    req: Req<VariableSetReq>,
) -> Result<ProjectView, IpcError> {
    call(&state, req, commands::projects::variable_set).await
}

/// `blueprint_repin`.
#[tauri::command]
pub async fn blueprint_repin(
    state: tauri::State<'_, Shared>,
    req: Req<BlueprintRepinReq>,
) -> Result<ProjectView, IpcError> {
    call(&state, req, commands::projects::blueprint_repin).await
}

/// `project_remove`.
#[tauri::command]
pub async fn project_remove(
    state: tauri::State<'_, Shared>,
    req: Req<ProjectRemoveReq>,
) -> Result<Vec<ProjectSummary>, IpcError> {
    call(&state, req, commands::projects::project_remove).await
}

/// `project_pin_set`.
#[tauri::command]
pub async fn project_pin_set(
    state: tauri::State<'_, Shared>,
    req: Req<ProjectPinSetReq>,
) -> Result<Vec<ProjectSummary>, IpcError> {
    call(&state, req, commands::projects::project_pin_set).await
}

/// `step_view`.
#[tauri::command]
pub async fn step_view(
    state: tauri::State<'_, Shared>,
    req: Req<StepViewReq>,
) -> Result<StepView, IpcError> {
    call(&state, req, commands::steps::step_view).await
}

/// `run_input_set`.
#[tauri::command]
pub async fn run_input_set(
    state: tauri::State<'_, Shared>,
    req: Req<RunInputSetReq>,
) -> Result<StepView, IpcError> {
    call(&state, req, commands::steps::run_input_set).await
}

/// `optional_part_set`.
#[tauri::command]
pub async fn optional_part_set(
    state: tauri::State<'_, Shared>,
    req: Req<OptionalPartSetReq>,
) -> Result<StepView, IpcError> {
    call(&state, req, commands::steps::optional_part_set).await
}

/// `run_override_set`.
#[tauri::command]
pub async fn run_override_set(
    state: tauri::State<'_, Shared>,
    req: Req<RunOverrideSetReq>,
) -> Result<StepView, IpcError> {
    call(&state, req, commands::steps::run_override_set).await
}

/// `step_mark_done`.
#[tauri::command]
pub async fn step_mark_done(
    state: tauri::State<'_, Shared>,
    req: Req<StepMarkDoneReq>,
) -> Result<StepView, IpcError> {
    call(&state, req, commands::steps::step_mark_done).await
}

/// `step_skip`.
#[tauri::command]
pub async fn step_skip(
    state: tauri::State<'_, Shared>,
    req: Req<StepSkipReq>,
) -> Result<ProjectView, IpcError> {
    call(&state, req, commands::steps::step_skip).await
}

/// `action_preview`.
#[tauri::command]
pub async fn action_preview(
    state: tauri::State<'_, Shared>,
    req: Req<ActionPreviewReq>,
) -> Result<ActionPreview, IpcError> {
    call(&state, req, commands::steps::action_preview).await
}

/// `action_apply`.
#[tauri::command]
pub async fn action_apply(
    state: tauri::State<'_, Shared>,
    req: Req<ActionApplyReq>,
) -> Result<StepView, IpcError> {
    call(&state, req, commands::steps::action_apply).await
}

/// `wait_start`.
#[tauri::command]
pub async fn wait_start(
    state: tauri::State<'_, Shared>,
    req: Req<WaitStartReq>,
) -> Result<StepView, IpcError> {
    call(&state, req, commands::steps::wait_start).await
}

/// `clipboard_copy`.
#[tauri::command]
pub async fn clipboard_copy(
    state: tauri::State<'_, Shared>,
    req: Req<ClipboardCopyReq>,
) -> Result<CopyResult, IpcError> {
    call(&state, req, commands::steps::clipboard_copy).await
}

/// `chat_copy_and_open`.
#[tauri::command]
pub async fn chat_copy_and_open(
    state: tauri::State<'_, Shared>,
    req: Req<ChatCopyAndOpenReq>,
) -> Result<ChatCopyResult, IpcError> {
    call(&state, req, commands::chat::chat_copy_and_open).await
}

/// `chat_link_set`.
#[tauri::command]
pub async fn chat_link_set(
    state: tauri::State<'_, Shared>,
    req: Req<ChatLinkSetReq>,
) -> Result<Rev, IpcError> {
    call(&state, req, commands::chat::chat_link_set).await
}

/// `paste_back_preview`.
#[tauri::command]
pub async fn paste_back_preview(
    state: tauri::State<'_, Shared>,
    req: Req<PasteBackPreviewReq>,
) -> Result<PastePreview, IpcError> {
    call(&state, req, commands::chat::paste_back_preview).await
}

/// `paste_back_apply`.
#[tauri::command]
pub async fn paste_back_apply(
    state: tauri::State<'_, Shared>,
    req: Req<PasteBackApplyReq>,
) -> Result<PasteApplyResult, IpcError> {
    call(&state, req, commands::chat::paste_back_apply).await
}

/// `launch_prepare`.
#[tauri::command]
pub async fn launch_prepare(
    state: tauri::State<'_, Shared>,
    req: Req<LaunchPrepareReq>,
    progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<PrepareOutcome, IpcError> {
    call(&state, req, move |a, r| {
        commands::launch::launch_prepare(a, r, progress)
    })
    .await
}

/// `chain_resume_prepare`.
#[tauri::command]
pub async fn chain_resume_prepare(
    state: tauri::State<'_, Shared>,
    req: Req<ChainResumePrepareReq>,
    progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<PrepareOutcome, IpcError> {
    call(&state, req, move |a, r| {
        commands::launch::chain_resume_prepare(a, r, progress)
    })
    .await
}

/// `launch_start`.
#[tauri::command]
pub async fn launch_start(
    state: tauri::State<'_, Shared>,
    req: Req<LaunchStartReq>,
    output: tauri::ipc::Channel<tauri::ipc::Response>,
) -> Result<RunStarted, IpcError> {
    call(&state, req, move |a, r| {
        commands::launch::launch_start(a, r, output)
    })
    .await
}

/// `config_ack`.
#[tauri::command]
pub async fn config_ack(
    state: tauri::State<'_, Shared>,
    req: Req<ConfigAckReq>,
) -> Result<(), IpcError> {
    call(&state, req, commands::launch::config_ack).await
}

/// `auth_key_allowlist_add`.
#[tauri::command]
pub async fn auth_key_allowlist_add(
    state: tauri::State<'_, Shared>,
    req: Req<AuthKeyAllowlistAddReq>,
) -> Result<SettingsView, IpcError> {
    call(&state, req, commands::launch::auth_key_allowlist_add).await
}

/// `project_env_approve`.
#[tauri::command]
pub async fn project_env_approve(
    state: tauri::State<'_, Shared>,
    req: Req<ProjectEnvApproveReq>,
) -> Result<(), IpcError> {
    call(&state, req, commands::launch::project_env_approve).await
}

/// `pty_attach`.
#[tauri::command]
pub async fn pty_attach(
    state: tauri::State<'_, Shared>,
    req: Req<PtyAttachReq>,
    output: tauri::ipc::Channel<tauri::ipc::Response>,
) -> Result<AttachInfo, IpcError> {
    call(&state, req, move |a, r| {
        commands::terminal::pty_attach(a, r, output)
    })
    .await
}

/// `pty_write`.
#[tauri::command]
pub async fn pty_write(
    state: tauri::State<'_, Shared>,
    req: Req<PtyWriteReq>,
) -> Result<(), IpcError> {
    call(&state, req, commands::terminal::pty_write).await
}

/// `pty_paste`.
#[tauri::command]
pub async fn pty_paste(
    state: tauri::State<'_, Shared>,
    req: Req<PtyPasteReq>,
) -> Result<PasteText, IpcError> {
    call(&state, req, commands::terminal::pty_paste).await
}

/// `pty_resize`.
#[tauri::command]
pub async fn pty_resize(
    state: tauri::State<'_, Shared>,
    req: Req<PtyResizeReq>,
) -> Result<(), IpcError> {
    call(&state, req, commands::terminal::pty_resize).await
}

/// `pty_flow`.
#[tauri::command]
pub async fn pty_flow(
    state: tauri::State<'_, Shared>,
    req: Req<PtyFlowReq>,
) -> Result<(), IpcError> {
    call(&state, req, commands::terminal::pty_flow).await
}

/// `pty_interrupt`.
#[tauri::command]
pub async fn pty_interrupt(
    state: tauri::State<'_, Shared>,
    req: Req<PtyInterruptReq>,
) -> Result<(), IpcError> {
    call(&state, req, commands::terminal::pty_interrupt).await
}

/// `pty_terminate`.
#[tauri::command]
pub async fn pty_terminate(
    state: tauri::State<'_, Shared>,
    req: Req<PtyTerminateReq>,
) -> Result<(), IpcError> {
    call(&state, req, commands::terminal::pty_terminate).await
}

/// `rearm_start`.
#[tauri::command]
pub async fn rearm_start(
    state: tauri::State<'_, Shared>,
    req: Req<RearmStartReq>,
    output: tauri::ipc::Channel<tauri::ipc::Response>,
    progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<RearmOutcome, IpcError> {
    call(&state, req, move |a, r| {
        commands::terminal::rearm_start(a, r, output, progress)
    })
    .await
}

/// `rearm_continue`.
#[tauri::command]
pub async fn rearm_continue(
    state: tauri::State<'_, Shared>,
    req: Req<RearmContinueReq>,
    output: tauri::ipc::Channel<tauri::ipc::Response>,
    progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<RearmOutcome, IpcError> {
    call(&state, req, move |a, r| {
        commands::terminal::rearm_continue(a, r, output, progress)
    })
    .await
}

/// `terminal_link_open`.
#[tauri::command]
pub async fn terminal_link_open(
    state: tauri::State<'_, Shared>,
    req: Req<TerminalLinkOpenReq>,
) -> Result<LinkOutcome, IpcError> {
    call(&state, req, commands::terminal::terminal_link_open).await
}

/// `file_open`.
#[tauri::command]
pub async fn file_open(
    state: tauri::State<'_, Shared>,
    req: Req<FileOpenReq>,
) -> Result<(), IpcError> {
    call(&state, req, commands::files::file_open).await
}

/// `file_reveal`.
#[tauri::command]
pub async fn file_reveal(
    state: tauri::State<'_, Shared>,
    req: Req<FileRevealReq>,
) -> Result<(), IpcError> {
    call(&state, req, commands::files::file_reveal).await
}

/// `terminal_open_here`.
#[tauri::command]
pub async fn terminal_open_here(
    state: tauri::State<'_, Shared>,
    req: Req<TerminalOpenHereReq>,
) -> Result<(), IpcError> {
    call(&state, req, commands::files::terminal_open_here).await
}

/// `pending_changes_list`.
#[tauri::command]
pub async fn pending_changes_list(
    state: tauri::State<'_, Shared>,
    req: Req<PendingChangesListReq>,
) -> Result<Vec<PendingChange>, IpcError> {
    call(&state, req, commands::decisions::pending_changes_list).await
}

/// `pending_change_accept`.
#[tauri::command]
pub async fn pending_change_accept(
    state: tauri::State<'_, Shared>,
    req: Req<PendingChangeAcceptReq>,
) -> Result<ProjectView, IpcError> {
    call(&state, req, commands::decisions::pending_change_accept).await
}

/// `pending_change_reject`.
#[tauri::command]
pub async fn pending_change_reject(
    state: tauri::State<'_, Shared>,
    req: Req<PendingChangeRejectReq>,
) -> Result<ProjectView, IpcError> {
    call(&state, req, commands::decisions::pending_change_reject).await
}

/// `item_kind_confirm`.
#[tauri::command]
pub async fn item_kind_confirm(
    state: tauri::State<'_, Shared>,
    req: Req<ItemKindConfirmReq>,
) -> Result<GateView, IpcError> {
    call(&state, req, commands::decisions::item_kind_confirm).await
}

/// `progress_rebuild_preview`.
#[tauri::command]
pub async fn progress_rebuild_preview(
    state: tauri::State<'_, Shared>,
    req: Req<ProgressRebuildPreviewReq>,
) -> Result<RebuildPreview, IpcError> {
    call(&state, req, commands::decisions::progress_rebuild_preview).await
}

/// `progress_rebuild_apply`.
#[tauri::command]
pub async fn progress_rebuild_apply(
    state: tauri::State<'_, Shared>,
    req: Req<ProgressRebuildApplyReq>,
) -> Result<ProjectView, IpcError> {
    call(&state, req, commands::decisions::progress_rebuild_apply).await
}

/// `packet_view`.
#[tauri::command]
pub async fn packet_view(
    state: tauri::State<'_, Shared>,
    req: Req<PacketViewReq>,
) -> Result<PacketView, IpcError> {
    call(&state, req, commands::packets::packet_view).await
}

/// `packet_normalize_apply`.
#[tauri::command]
pub async fn packet_normalize_apply(
    state: tauri::State<'_, Shared>,
    req: Req<PacketNormalizeApplyReq>,
) -> Result<PacketView, IpcError> {
    call(&state, req, commands::packets::packet_normalize_apply).await
}

/// `packet_signature_confirm`.
#[tauri::command]
pub async fn packet_signature_confirm(
    state: tauri::State<'_, Shared>,
    req: Req<PacketSignatureConfirmReq>,
) -> Result<PacketView, IpcError> {
    call(&state, req, commands::packets::packet_signature_confirm).await
}

/// `packet_repair_prepare`.
#[tauri::command]
pub async fn packet_repair_prepare(
    state: tauri::State<'_, Shared>,
    req: Req<PacketRepairPrepareReq>,
    progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<PrepareOutcome, IpcError> {
    call(&state, req, move |a, r| {
        commands::packets::packet_repair_prepare(a, r, progress)
    })
    .await
}

/// `metrics_export`.
#[tauri::command]
pub async fn metrics_export(
    state: tauri::State<'_, Shared>,
    req: Req<MetricsExportReq>,
) -> Result<ExportResult, IpcError> {
    call(&state, req, commands::packets::metrics_export).await
}

/// `metrics_view`.
#[tauri::command]
pub async fn metrics_view(
    state: tauri::State<'_, Shared>,
    req: Req<MetricsViewReq>,
) -> Result<MetricsView, IpcError> {
    call(&state, req, commands::packets::metrics_view).await
}

/// `runs_clear`.
#[tauri::command]
pub async fn runs_clear(
    state: tauri::State<'_, Shared>,
    req: Req<RunsClearReq>,
) -> Result<(), IpcError> {
    call(&state, req, commands::packets::runs_clear).await
}

/// `runs_search`.
#[tauri::command]
pub async fn runs_search(
    state: tauri::State<'_, Shared>,
    req: Req<RunsSearchReq>,
) -> Result<RunsPage, IpcError> {
    call(&state, req, commands::packets::runs_search).await
}

/// `gate_view`.
#[tauri::command]
pub async fn gate_view(
    state: tauri::State<'_, Shared>,
    req: Req<GateViewReq>,
) -> Result<GateView, IpcError> {
    call(&state, req, commands::gates::gate_view).await
}

/// `gate_item_attest`.
#[tauri::command]
pub async fn gate_item_attest(
    state: tauri::State<'_, Shared>,
    req: Req<GateItemAttestReq>,
) -> Result<GateView, IpcError> {
    call(&state, req, commands::gates::gate_item_attest).await
}

/// `gate_import_results`.
#[tauri::command]
pub async fn gate_import_results(
    state: tauri::State<'_, Shared>,
    req: Req<GateImportResultsReq>,
) -> Result<Vec<ImportProposal>, IpcError> {
    call(&state, req, commands::gates::gate_import_results).await
}

/// `gate_import_confirm`.
#[tauri::command]
pub async fn gate_import_confirm(
    state: tauri::State<'_, Shared>,
    req: Req<GateImportConfirmReq>,
) -> Result<GateView, IpcError> {
    call(&state, req, commands::gates::gate_import_confirm).await
}

/// `gate_fix_start`.
#[tauri::command]
pub async fn gate_fix_start(
    state: tauri::State<'_, Shared>,
    req: Req<GateFixStartReq>,
    progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<PrepareOutcome, IpcError> {
    call(&state, req, move |a, r| {
        commands::gates::gate_fix_start(a, r, progress)
    })
    .await
}

/// `sign_preview`.
#[tauri::command]
pub async fn sign_preview(
    state: tauri::State<'_, Shared>,
    req: Req<SignPreviewReq>,
) -> Result<SignPreview, IpcError> {
    call(&state, req, commands::signing::sign_preview).await
}

/// `sign_apply`.
#[tauri::command]
pub async fn sign_apply(
    state: tauri::State<'_, Shared>,
    req: Req<SignApplyReq>,
    progress: tauri::ipc::Channel<SignProgress>,
) -> Result<SignResult, IpcError> {
    call(&state, req, move |a, r| {
        commands::signing::sign_apply(a, r, progress)
    })
    .await
}

/// `sign_retry_commit`.
#[tauri::command]
pub async fn sign_retry_commit(
    state: tauri::State<'_, Shared>,
    req: Req<SignRetryCommitReq>,
    progress: tauri::ipc::Channel<SignProgress>,
) -> Result<SignResult, IpcError> {
    call(&state, req, move |a, r| {
        commands::signing::sign_retry_commit(a, r, progress)
    })
    .await
}

/// `tamper_restore`.
#[tauri::command]
pub async fn tamper_restore(
    state: tauri::State<'_, Shared>,
    req: Req<TamperRestoreReq>,
) -> Result<PacketView, IpcError> {
    call(&state, req, commands::signing::tamper_restore).await
}

/// `lessons_status`.
#[tauri::command]
pub async fn lessons_status(
    state: tauri::State<'_, Shared>,
    req: Req<LessonsStatusReq>,
) -> Result<LessonsStatus, IpcError> {
    call(&state, req, commands::assembler::lessons_status).await
}

/// `manifest_view`.
#[tauri::command]
pub async fn manifest_view(
    state: tauri::State<'_, Shared>,
    req: Req<ManifestViewReq>,
) -> Result<Manifest, IpcError> {
    call(&state, req, commands::assembler::manifest_view).await
}

/// `manifest_mark`.
#[tauri::command]
pub async fn manifest_mark(
    state: tauri::State<'_, Shared>,
    req: Req<ManifestMarkReq>,
) -> Result<Manifest, IpcError> {
    call(&state, req, commands::assembler::manifest_mark).await
}

/// `assembler_view`.
#[tauri::command]
pub async fn assembler_view(
    state: tauri::State<'_, Shared>,
    req: Req<AssemblerViewReq>,
) -> Result<Assembler, IpcError> {
    call(&state, req, commands::assembler::assembler_view).await
}

/// `assembler_paste_preview`.
#[tauri::command]
pub async fn assembler_paste_preview(
    state: tauri::State<'_, Shared>,
    req: Req<AssemblerPastePreviewReq>,
) -> Result<PastePreview, IpcError> {
    call(&state, req, commands::assembler::assembler_paste_preview).await
}

/// `assembler_paste_apply`.
#[tauri::command]
pub async fn assembler_paste_apply(
    state: tauri::State<'_, Shared>,
    req: Req<AssemblerPasteApplyReq>,
) -> Result<Assembler, IpcError> {
    call(&state, req, commands::assembler::assembler_paste_apply).await
}

/// `desktop_open`.
#[tauri::command]
pub async fn desktop_open(
    state: tauri::State<'_, Shared>,
    req: Req<DesktopOpenReq>,
    progress: tauri::ipc::Channel<LaunchProgress>,
) -> Result<DesktopOpened, IpcError> {
    call(&state, req, move |a, r| {
        commands::desktop::desktop_open(a, r, progress)
    })
    .await
}

/// `desktop_handoff`.
#[tauri::command]
pub async fn desktop_handoff(
    state: tauri::State<'_, Shared>,
    req: Req<DesktopHandoffReq>,
) -> Result<(), IpcError> {
    call(&state, req, commands::desktop::desktop_handoff).await
}

/// `add_step`.
#[tauri::command]
pub async fn add_step(
    state: tauri::State<'_, Shared>,
    req: Req<AddStepReq>,
) -> Result<ProjectView, IpcError> {
    call(&state, req, commands::desktop::add_step).await
}

/// `add_item`.
#[tauri::command]
pub async fn add_item(
    state: tauri::State<'_, Shared>,
    req: Req<AddItemReq>,
) -> Result<GateView, IpcError> {
    call(&state, req, commands::desktop::add_item).await
}

/// `run_sheet_export`.
#[tauri::command]
pub async fn run_sheet_export(
    state: tauri::State<'_, Shared>,
    req: Req<RunSheetExportReq>,
) -> Result<ExportResult, IpcError> {
    call(&state, req, commands::desktop::run_sheet_export).await
}
