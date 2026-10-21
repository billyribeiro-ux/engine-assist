// The typed IPC contract (ADR-004, docs/contracts/ipc.md §4): one method per command, 91 in all.
// `TauriIpcClient` and `MockIpcClient` both implement it (tsc enforces). Methods resolve with the
// response type or reject with an `IpcFailure` (never a bare string). Commands that stream take the
// Channel's callback as a second argument; components get callbacks, never `Channel` objects (§5).
import type { ActionApplyReq } from './bindings/ActionApplyReq.ts';
import type { ActionPreviewReq } from './bindings/ActionPreviewReq.ts';
import type { AddItemReq } from './bindings/AddItemReq.ts';
import type { AddStepReq } from './bindings/AddStepReq.ts';
import type { AppBootstrapReq } from './bindings/AppBootstrapReq.ts';
import type { AssemblerPasteApplyReq } from './bindings/AssemblerPasteApplyReq.ts';
import type { AssemblerPastePreviewReq } from './bindings/AssemblerPastePreviewReq.ts';
import type { AssemblerViewReq } from './bindings/AssemblerViewReq.ts';
import type { AuthKeyAllowlistAddReq } from './bindings/AuthKeyAllowlistAddReq.ts';
import type { BlueprintPickRepoReq } from './bindings/BlueprintPickRepoReq.ts';
import type { BlueprintRepinReq } from './bindings/BlueprintRepinReq.ts';
import type { ChainResumePrepareReq } from './bindings/ChainResumePrepareReq.ts';
import type { ChatCopyAndOpenReq } from './bindings/ChatCopyAndOpenReq.ts';
import type { ChatLinkSetReq } from './bindings/ChatLinkSetReq.ts';
import type { ClipboardCopyReq } from './bindings/ClipboardCopyReq.ts';
import type { ConditionSetReq } from './bindings/ConditionSetReq.ts';
import type { ConfigAckReq } from './bindings/ConfigAckReq.ts';
import type { DesktopHandoffReq } from './bindings/DesktopHandoffReq.ts';
import type { DesktopOpenReq } from './bindings/DesktopOpenReq.ts';
import type { DoNextReq } from './bindings/DoNextReq.ts';
import type { DoctorCaptureLoginPathReq } from './bindings/DoctorCaptureLoginPathReq.ts';
import type { DoctorLoginPathApplyReq } from './bindings/DoctorLoginPathApplyReq.ts';
import type { DoctorRunReq } from './bindings/DoctorRunReq.ts';
import type { FileOpenReq } from './bindings/FileOpenReq.ts';
import type { FileRevealReq } from './bindings/FileRevealReq.ts';
import type { GateFixStartReq } from './bindings/GateFixStartReq.ts';
import type { GateImportConfirmReq } from './bindings/GateImportConfirmReq.ts';
import type { GateImportResultsReq } from './bindings/GateImportResultsReq.ts';
import type { GateItemAttestReq } from './bindings/GateItemAttestReq.ts';
import type { GateViewReq } from './bindings/GateViewReq.ts';
import type { ItemKindConfirmReq } from './bindings/ItemKindConfirmReq.ts';
import type { LaunchPrepareReq } from './bindings/LaunchPrepareReq.ts';
import type { LaunchStartReq } from './bindings/LaunchStartReq.ts';
import type { LessonsStatusReq } from './bindings/LessonsStatusReq.ts';
import type { ManifestMarkReq } from './bindings/ManifestMarkReq.ts';
import type { ManifestViewReq } from './bindings/ManifestViewReq.ts';
import type { MetricsExportReq } from './bindings/MetricsExportReq.ts';
import type { MetricsViewReq } from './bindings/MetricsViewReq.ts';
import type { OptionalPartSetReq } from './bindings/OptionalPartSetReq.ts';
import type { PacketNormalizeApplyReq } from './bindings/PacketNormalizeApplyReq.ts';
import type { PacketRepairPrepareReq } from './bindings/PacketRepairPrepareReq.ts';
import type { PacketSignatureConfirmReq } from './bindings/PacketSignatureConfirmReq.ts';
import type { PacketViewReq } from './bindings/PacketViewReq.ts';
import type { PasteBackApplyReq } from './bindings/PasteBackApplyReq.ts';
import type { PasteBackPreviewReq } from './bindings/PasteBackPreviewReq.ts';
import type { PendingChangeAcceptReq } from './bindings/PendingChangeAcceptReq.ts';
import type { PendingChangeRejectReq } from './bindings/PendingChangeRejectReq.ts';
import type { PendingChangesListReq } from './bindings/PendingChangesListReq.ts';
import type { ProbeFolderOpenReq } from './bindings/ProbeFolderOpenReq.ts';
import type { ProbeRunReq } from './bindings/ProbeRunReq.ts';
import type { ProgressRebuildApplyReq } from './bindings/ProgressRebuildApplyReq.ts';
import type { ProgressRebuildPreviewReq } from './bindings/ProgressRebuildPreviewReq.ts';
import type { ProjectEnvApproveReq } from './bindings/ProjectEnvApproveReq.ts';
import type { ProjectOpenReq } from './bindings/ProjectOpenReq.ts';
import type { ProjectPinSetReq } from './bindings/ProjectPinSetReq.ts';
import type { ProjectRemoveReq } from './bindings/ProjectRemoveReq.ts';
import type { ProjectUrlSetReq } from './bindings/ProjectUrlSetReq.ts';
import type { ProjectsListReq } from './bindings/ProjectsListReq.ts';
import type { PtyAttachReq } from './bindings/PtyAttachReq.ts';
import type { PtyFlowReq } from './bindings/PtyFlowReq.ts';
import type { PtyInterruptReq } from './bindings/PtyInterruptReq.ts';
import type { PtyPasteReq } from './bindings/PtyPasteReq.ts';
import type { PtyResizeReq } from './bindings/PtyResizeReq.ts';
import type { PtyTerminateReq } from './bindings/PtyTerminateReq.ts';
import type { PtyWriteReq } from './bindings/PtyWriteReq.ts';
import type { RearmContinueReq } from './bindings/RearmContinueReq.ts';
import type { RearmStartReq } from './bindings/RearmStartReq.ts';
import type { RunInputSetReq } from './bindings/RunInputSetReq.ts';
import type { RunOverrideSetReq } from './bindings/RunOverrideSetReq.ts';
import type { RunSheetExportReq } from './bindings/RunSheetExportReq.ts';
import type { RunsClearReq } from './bindings/RunsClearReq.ts';
import type { RunsSearchReq } from './bindings/RunsSearchReq.ts';
import type { SettingsGetReq } from './bindings/SettingsGetReq.ts';
import type { SettingsPickToolReq } from './bindings/SettingsPickToolReq.ts';
import type { SettingsUpdateReq } from './bindings/SettingsUpdateReq.ts';
import type { SetupApplyReq } from './bindings/SetupApplyReq.ts';
import type { SetupPickFolderReq } from './bindings/SetupPickFolderReq.ts';
import type { SetupPreviewReq } from './bindings/SetupPreviewReq.ts';
import type { SignApplyReq } from './bindings/SignApplyReq.ts';
import type { SignPreviewReq } from './bindings/SignPreviewReq.ts';
import type { SignRetryCommitReq } from './bindings/SignRetryCommitReq.ts';
import type { StepMarkDoneReq } from './bindings/StepMarkDoneReq.ts';
import type { StepSkipReq } from './bindings/StepSkipReq.ts';
import type { StepViewReq } from './bindings/StepViewReq.ts';
import type { TamperRestoreReq } from './bindings/TamperRestoreReq.ts';
import type { TemplateHealthReq } from './bindings/TemplateHealthReq.ts';
import type { TerminalLinkOpenReq } from './bindings/TerminalLinkOpenReq.ts';
import type { TerminalOpenHereReq } from './bindings/TerminalOpenHereReq.ts';
import type { TodayGetReq } from './bindings/TodayGetReq.ts';
import type { VariableSetReq } from './bindings/VariableSetReq.ts';
import type { WaitStartReq } from './bindings/WaitStartReq.ts';
import type * as T from './types.ts';

/** Request types: the Rust request structs, generated by ts-rs (ADR-004). */
export interface Requests {
	app_bootstrap: AppBootstrapReq;
	settings_get: SettingsGetReq;
	settings_update: SettingsUpdateReq;
	settings_pick_tool: SettingsPickToolReq;
	today_get: TodayGetReq;
	do_next: DoNextReq;
	blueprint_pick_repo: BlueprintPickRepoReq;
	template_health: TemplateHealthReq;
	doctor_run: DoctorRunReq;
	doctor_capture_login_path: DoctorCaptureLoginPathReq;
	doctor_login_path_apply: DoctorLoginPathApplyReq;
	probe_folder_open: ProbeFolderOpenReq;
	probe_run: ProbeRunReq;
	projects_list: ProjectsListReq;
	setup_pick_folder: SetupPickFolderReq;
	setup_preview: SetupPreviewReq;
	setup_apply: SetupApplyReq;
	project_open: ProjectOpenReq;
	project_url_set: ProjectUrlSetReq;
	condition_set: ConditionSetReq;
	variable_set: VariableSetReq;
	blueprint_repin: BlueprintRepinReq;
	project_remove: ProjectRemoveReq;
	project_pin_set: ProjectPinSetReq;
	step_view: StepViewReq;
	run_input_set: RunInputSetReq;
	optional_part_set: OptionalPartSetReq;
	run_override_set: RunOverrideSetReq;
	step_mark_done: StepMarkDoneReq;
	step_skip: StepSkipReq;
	action_preview: ActionPreviewReq;
	action_apply: ActionApplyReq;
	wait_start: WaitStartReq;
	clipboard_copy: ClipboardCopyReq;
	chat_copy_and_open: ChatCopyAndOpenReq;
	chat_link_set: ChatLinkSetReq;
	paste_back_preview: PasteBackPreviewReq;
	paste_back_apply: PasteBackApplyReq;
	launch_prepare: LaunchPrepareReq;
	chain_resume_prepare: ChainResumePrepareReq;
	launch_start: LaunchStartReq;
	config_ack: ConfigAckReq;
	auth_key_allowlist_add: AuthKeyAllowlistAddReq;
	project_env_approve: ProjectEnvApproveReq;
	pty_attach: PtyAttachReq;
	pty_write: PtyWriteReq;
	pty_paste: PtyPasteReq;
	pty_resize: PtyResizeReq;
	pty_flow: PtyFlowReq;
	pty_interrupt: PtyInterruptReq;
	pty_terminate: PtyTerminateReq;
	rearm_start: RearmStartReq;
	rearm_continue: RearmContinueReq;
	terminal_link_open: TerminalLinkOpenReq;
	file_open: FileOpenReq;
	file_reveal: FileRevealReq;
	terminal_open_here: TerminalOpenHereReq;
	pending_changes_list: PendingChangesListReq;
	pending_change_accept: PendingChangeAcceptReq;
	pending_change_reject: PendingChangeRejectReq;
	item_kind_confirm: ItemKindConfirmReq;
	progress_rebuild_preview: ProgressRebuildPreviewReq;
	progress_rebuild_apply: ProgressRebuildApplyReq;
	packet_view: PacketViewReq;
	packet_normalize_apply: PacketNormalizeApplyReq;
	packet_signature_confirm: PacketSignatureConfirmReq;
	packet_repair_prepare: PacketRepairPrepareReq;
	metrics_export: MetricsExportReq;
	metrics_view: MetricsViewReq;
	runs_clear: RunsClearReq;
	runs_search: RunsSearchReq;
	gate_view: GateViewReq;
	gate_item_attest: GateItemAttestReq;
	gate_import_results: GateImportResultsReq;
	gate_import_confirm: GateImportConfirmReq;
	gate_fix_start: GateFixStartReq;
	sign_preview: SignPreviewReq;
	sign_apply: SignApplyReq;
	sign_retry_commit: SignRetryCommitReq;
	tamper_restore: TamperRestoreReq;
	lessons_status: LessonsStatusReq;
	manifest_view: ManifestViewReq;
	manifest_mark: ManifestMarkReq;
	assembler_view: AssemblerViewReq;
	assembler_paste_preview: AssemblerPastePreviewReq;
	assembler_paste_apply: AssemblerPasteApplyReq;
	desktop_open: DesktopOpenReq;
	desktop_handoff: DesktopHandoffReq;
	add_step: AddStepReq;
	add_item: AddItemReq;
	run_sheet_export: RunSheetExportReq;
}

/** Response types (`()` is `null` in TS). */
export interface Responses {
	app_bootstrap: T.AppBootstrap;
	settings_get: T.SettingsView;
	settings_update: T.SettingsView;
	settings_pick_tool: T.ToolPaths;
	today_get: T.TodayView;
	do_next: T.DoNextOutcome;
	blueprint_pick_repo: T.BlueprintRepoInfo;
	template_health: T.TemplateHealth;
	doctor_run: T.DoctorReport;
	doctor_capture_login_path: T.LoginPathPreview;
	doctor_login_path_apply: T.SettingsView;
	probe_folder_open: T.RunStarted;
	probe_run: T.ContractResult;
	projects_list: T.ProjectSummary[];
	setup_pick_folder: T.FolderPick;
	setup_preview: T.SetupPreview;
	setup_apply: T.ProjectSummary;
	project_open: T.ProjectView;
	project_url_set: T.Rev;
	condition_set: T.ProjectView;
	variable_set: T.ProjectView;
	blueprint_repin: T.ProjectView;
	project_remove: T.ProjectSummary[];
	project_pin_set: T.ProjectSummary[];
	step_view: T.StepView;
	run_input_set: T.StepView;
	optional_part_set: T.StepView;
	run_override_set: T.StepView;
	step_mark_done: T.StepView;
	step_skip: T.ProjectView;
	action_preview: T.ActionPreview;
	action_apply: T.StepView;
	wait_start: T.StepView;
	clipboard_copy: T.CopyResult;
	chat_copy_and_open: T.ChatCopyResult;
	chat_link_set: T.Rev;
	paste_back_preview: T.PastePreview;
	paste_back_apply: T.PasteApplyResult;
	launch_prepare: T.PrepareOutcome;
	chain_resume_prepare: T.PrepareOutcome;
	launch_start: T.RunStarted;
	config_ack: null;
	auth_key_allowlist_add: T.SettingsView;
	project_env_approve: null;
	pty_attach: T.AttachInfo;
	pty_write: null;
	pty_paste: T.PasteText;
	pty_resize: null;
	pty_flow: null;
	pty_interrupt: null;
	pty_terminate: null;
	rearm_start: T.RearmOutcome;
	rearm_continue: T.RearmOutcome;
	terminal_link_open: T.LinkOutcome;
	file_open: null;
	file_reveal: null;
	terminal_open_here: null;
	pending_changes_list: T.PendingChange[];
	pending_change_accept: T.ProjectView;
	pending_change_reject: T.ProjectView;
	item_kind_confirm: T.GateView;
	progress_rebuild_preview: T.RebuildPreview;
	progress_rebuild_apply: T.ProjectView;
	packet_view: T.PacketView;
	packet_normalize_apply: T.PacketView;
	packet_signature_confirm: T.PacketView;
	packet_repair_prepare: T.PrepareOutcome;
	metrics_export: T.ExportResult;
	metrics_view: T.MetricsView;
	runs_clear: null;
	runs_search: T.RunsPage;
	gate_view: T.GateView;
	gate_item_attest: T.GateView;
	gate_import_results: T.ImportProposal[];
	gate_import_confirm: T.GateView;
	gate_fix_start: T.PrepareOutcome;
	sign_preview: T.SignPreview;
	sign_apply: T.SignResult;
	sign_retry_commit: T.SignResult;
	tamper_restore: T.PacketView;
	lessons_status: T.LessonsStatus;
	manifest_view: T.Manifest;
	manifest_mark: T.Manifest;
	assembler_view: T.Assembler;
	assembler_paste_preview: T.PastePreview;
	assembler_paste_apply: T.Assembler;
	desktop_open: T.DesktopOpened;
	desktop_handoff: null;
	add_step: T.ProjectView;
	add_item: T.GateView;
	run_sheet_export: T.ExportResult;
}

/** Rust command name → IpcClient method name. Its keys are §4.15's 91 commands. */
export const COMMAND_MAP = {
	app_bootstrap: 'appBootstrap',
	settings_get: 'settingsGet',
	settings_update: 'settingsUpdate',
	settings_pick_tool: 'settingsPickTool',
	today_get: 'todayGet',
	do_next: 'doNext',
	blueprint_pick_repo: 'blueprintPickRepo',
	template_health: 'templateHealth',
	doctor_run: 'doctorRun',
	doctor_capture_login_path: 'doctorCaptureLoginPath',
	doctor_login_path_apply: 'doctorLoginPathApply',
	probe_folder_open: 'probeFolderOpen',
	probe_run: 'probeRun',
	projects_list: 'projectsList',
	setup_pick_folder: 'setupPickFolder',
	setup_preview: 'setupPreview',
	setup_apply: 'setupApply',
	project_open: 'projectOpen',
	project_url_set: 'projectUrlSet',
	condition_set: 'conditionSet',
	variable_set: 'variableSet',
	blueprint_repin: 'blueprintRepin',
	project_remove: 'projectRemove',
	project_pin_set: 'projectPinSet',
	step_view: 'stepView',
	run_input_set: 'runInputSet',
	optional_part_set: 'optionalPartSet',
	run_override_set: 'runOverrideSet',
	step_mark_done: 'stepMarkDone',
	step_skip: 'stepSkip',
	action_preview: 'actionPreview',
	action_apply: 'actionApply',
	wait_start: 'waitStart',
	clipboard_copy: 'clipboardCopy',
	chat_copy_and_open: 'chatCopyAndOpen',
	chat_link_set: 'chatLinkSet',
	paste_back_preview: 'pasteBackPreview',
	paste_back_apply: 'pasteBackApply',
	launch_prepare: 'launchPrepare',
	chain_resume_prepare: 'chainResumePrepare',
	launch_start: 'launchStart',
	config_ack: 'configAck',
	auth_key_allowlist_add: 'authKeyAllowlistAdd',
	project_env_approve: 'projectEnvApprove',
	pty_attach: 'ptyAttach',
	pty_write: 'ptyWrite',
	pty_paste: 'ptyPaste',
	pty_resize: 'ptyResize',
	pty_flow: 'ptyFlow',
	pty_interrupt: 'ptyInterrupt',
	pty_terminate: 'ptyTerminate',
	rearm_start: 'rearmStart',
	rearm_continue: 'rearmContinue',
	terminal_link_open: 'terminalLinkOpen',
	file_open: 'fileOpen',
	file_reveal: 'fileReveal',
	terminal_open_here: 'terminalOpenHere',
	pending_changes_list: 'pendingChangesList',
	pending_change_accept: 'pendingChangeAccept',
	pending_change_reject: 'pendingChangeReject',
	item_kind_confirm: 'itemKindConfirm',
	progress_rebuild_preview: 'progressRebuildPreview',
	progress_rebuild_apply: 'progressRebuildApply',
	packet_view: 'packetView',
	packet_normalize_apply: 'packetNormalizeApply',
	packet_signature_confirm: 'packetSignatureConfirm',
	packet_repair_prepare: 'packetRepairPrepare',
	metrics_export: 'metricsExport',
	metrics_view: 'metricsView',
	runs_clear: 'runsClear',
	runs_search: 'runsSearch',
	gate_view: 'gateView',
	gate_item_attest: 'gateItemAttest',
	gate_import_results: 'gateImportResults',
	gate_import_confirm: 'gateImportConfirm',
	gate_fix_start: 'gateFixStart',
	sign_preview: 'signPreview',
	sign_apply: 'signApply',
	sign_retry_commit: 'signRetryCommit',
	tamper_restore: 'tamperRestore',
	lessons_status: 'lessonsStatus',
	manifest_view: 'manifestView',
	manifest_mark: 'manifestMark',
	assembler_view: 'assemblerView',
	assembler_paste_preview: 'assemblerPastePreview',
	assembler_paste_apply: 'assemblerPasteApply',
	desktop_open: 'desktopOpen',
	desktop_handoff: 'desktopHandoff',
	add_step: 'addStep',
	add_item: 'addItem',
	run_sheet_export: 'runSheetExport'
} as const satisfies { [K in keyof Requests]: string };

export type CommandName = keyof typeof COMMAND_MAP;
export type MethodName = (typeof COMMAND_MAP)[CommandName];
type CommandOf<M extends MethodName> = {
	[K in CommandName]: (typeof COMMAND_MAP)[K] extends M ? K : never;
}[CommandName];

/** Commands that take the PTY output Channel (§5). */
export type PtyCommands = 'launch_start' | 'pty_attach' | 'probe_folder_open';
/** Commands that take the launch-progress Channel (§5). */
export type ProgressCommands =
	| 'launch_prepare'
	| 'chain_resume_prepare'
	| 'packet_repair_prepare'
	| 'gate_fix_start'
	| 'desktop_open'
	| 'probe_run';
/** The re-arm commands take both (the one command pair with two Channels, §4.7). */
export type RearmCommands = 'rearm_start' | 'rearm_continue';

type Method<C extends CommandName> = C extends PtyCommands
	? (req: Requests[C], output?: T.PtyOutput) => Promise<Responses[C]>
	: C extends ProgressCommands
		? (req: Requests[C], progress?: T.OnProgress) => Promise<Responses[C]>
		: C extends RearmCommands
			? (req: Requests[C], output?: T.PtyOutput, progress?: T.OnProgress) => Promise<Responses[C]>
			: C extends 'sign_apply' | 'sign_retry_commit'
				? (req: Requests[C], progress?: T.OnSignProgress) => Promise<Responses[C]>
				: (req: Requests[C]) => Promise<Responses[C]>;

export type IpcCommands = { [M in MethodName]: Method<CommandOf<M>> };

export interface IpcClient extends IpcCommands {
	/** Subscribes to a Rust → webview event (§6). Events carry identifiers and revisions only. */
	listen<E extends T.EventName>(
		event: E,
		handler: (payload: T.EventPayloads[E]) => void
	): Promise<T.Unlisten>;
}

/** §6.2: `pty_write` data over 64 KB is split at code-point boundaries into ≤ 64 KB writes. */
export const PTY_WRITE_MAX = 64 * 1024;
export function splitPtyData(data: string, max = PTY_WRITE_MAX): string[] {
	const enc = new TextEncoder();
	const chunks: string[] = [];
	let cur = '';
	let bytes = 0;
	for (const ch of data) {
		const n = enc.encode(ch).length;
		if (bytes + n > max && cur) {
			chunks.push(cur);
			cur = '';
			bytes = 0;
		}
		cur += ch;
		bytes += n;
	}
	if (cur || chunks.length === 0) chunks.push(cur);
	return chunks;
}
