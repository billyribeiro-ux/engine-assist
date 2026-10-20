// The typed IPC contract (ADR-004, docs/contracts/ipc.md §4): one method per command, 91 in all.
// `TauriIpcClient` and `MockIpcClient` both implement it (tsc enforces). Methods resolve with the
// response type or reject with an `IpcFailure` (never a bare string). Commands that stream take the
// Channel's callback as a second argument; components get callbacks, never `Channel` objects (§5).
import type { AppBootstrapReq } from './bindings/AppBootstrapReq.ts';
import type * as T from './types.ts';

type P = T.ProjectId;

/** Request types, exactly as ipc.md §4 gives them. */
export interface Requests {
	app_bootstrap: AppBootstrapReq;
	settings_get: T.Empty;
	settings_update: { expectRev: number; patch: T.SettingsPatch };
	settings_pick_tool: { tool: T.ToolKind };
	today_get: T.Empty;
	do_next: T.Empty;
	blueprint_pick_repo: { mode: 'autoDetect' | 'choose' };
	template_health: { projectId: P | null };
	doctor_run: { scope: T.DoctorScope };
	doctor_capture_login_path: T.Empty;
	doctor_login_path_apply: { previewId: string };
	probe_folder_open: T.Empty;
	probe_run: T.Empty;
	projects_list: T.Empty;
	setup_pick_folder: T.Empty;
	setup_preview: {
		pickId: string;
		name: string;
		slug: string;
		variables: Record<string, string>;
		conditions: Partial<Record<T.ConditionId, T.TriState>>;
		chatProjectUrl: string | null;
	};
	setup_apply: { previewId: string };
	project_open: { projectId: P };
	project_url_set: { projectId: P; url: string | null };
	condition_set: { projectId: P; condition: T.ConditionId; to: T.TriState; note: string };
	variable_set: { projectId: P; name: string; value: string };
	blueprint_repin: { projectId: P };
	project_remove: { projectId: P };
	project_pin_set: { projectId: P; slot: number | null };
	step_view: { projectId: P; step: string };
	run_input_set: { projectId: P; step: string; name: string; value: string };
	optional_part_set: { projectId: P; step: string; partId: string; include: boolean };
	run_override_set: { projectId: P; step: string; over: T.RunOverride | null };
	step_mark_done: { projectId: P; step: string };
	step_skip: { projectId: P; step: string; reason: string };
	action_preview: { projectId: P; step: string };
	action_apply: { previewId: string };
	wait_start: { projectId: P; step: string; until: string };
	clipboard_copy: {
		projectId: P;
		target: T.CopyTarget;
		step: string | null;
		chain: string | null;
		path: string | null;
		stage: number | null;
	};
	chat_copy_and_open: { projectId: P; step: string; entry: T.EntryPoint };
	chat_link_set: { projectId: P; chainStart: string; source: T.LinkSource };
	paste_back_preview: { projectId: P; step: string; kind: T.PasteKind; source: T.PasteSource };
	paste_back_apply: { previewId: string };
	launch_prepare: { projectId: P; step: string; entry: T.EntryPoint; mode: T.PrepareMode };
	chain_resume_prepare: { chain: string };
	launch_start: { planId: string };
	config_ack: { projectId: P; cwd: T.CwdKind; fingerprint: string };
	auth_key_allowlist_add: { hit: string };
	project_env_approve: { projectId: P; cwd: T.CwdKind; sha256: string };
	pty_attach: { runId: string };
	pty_write: { runId: string; data: string };
	pty_paste: { runId: string };
	pty_resize: { runId: string; cols: number; rows: number };
	pty_flow: { runId: string; paused: boolean };
	pty_interrupt: { runId: string };
	pty_terminate: { runId: string };
	rearm_start: { planId: string };
	rearm_continue: { chain: string };
	terminal_link_open: { runId: string; url: string };
	file_open: { projectId: P; path: string };
	file_reveal: { projectId: P; path: string };
	terminal_open_here: { projectId: P };
	pending_changes_list: { projectId: P };
	pending_change_accept: { projectId: P; change: string; seenSha256: string };
	pending_change_reject: { projectId: P; change: string; seenSha256: string };
	item_kind_confirm: { projectId: P; item: string; textSha256: string };
	progress_rebuild_preview: { projectId: P };
	progress_rebuild_apply: { previewId: string };
	packet_view: { projectId: P; stage: number };
	packet_normalize_apply: { projectId: P; stage: number; seenSha256: string };
	packet_signature_confirm: { projectId: P; stage: number; seenSha256: string };
	packet_repair_prepare: { projectId: P; stage: number };
	metrics_export: T.Empty;
	metrics_view: { projectId: P | null };
	runs_clear: T.Empty;
	runs_search: { query: string; projectId: P | null; cursor: string | null; limit: number };
	gate_view: { projectId: P; stage: number };
	gate_item_attest: { projectId: P; stage: number; item: string; outcome: T.AttestOutcome };
	gate_import_results: { projectId: P; stage: number };
	gate_import_confirm: {
		projectId: P;
		stage: number;
		item: string;
		result: 'pass' | 'fail';
		evidence: string;
	};
	gate_fix_start: { projectId: P; stage: number; item: string };
	sign_preview: {
		projectId: P;
		stage: number;
		branch: T.SignBranch;
		name: string;
		date: string;
	};
	sign_apply: { previewId: string };
	sign_retry_commit: { projectId: P; stage: number };
	tamper_restore: { projectId: P; stage: number; seenSha256: string };
	lessons_status: { projectId: P; stage: number };
	manifest_view: { projectId: P };
	manifest_mark: { projectId: P; path: string; inProject: boolean };
	assembler_view: { projectId: P };
	assembler_paste_preview: { projectId: P; path: string };
	assembler_paste_apply: { previewId: string };
	desktop_open: { projectId: P; step: string };
	desktop_handoff: { chain: string };
	add_step: {
		projectId: P;
		stage: number;
		after: string;
		copyConfigFrom: string;
		title: string;
		text: string;
		config: T.AddedStepConfig;
		tag: 'optional' | 'core';
	};
	add_item: { projectId: P; stage: number; text: string };
	run_sheet_export: { projectId: P; stage: number };
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
