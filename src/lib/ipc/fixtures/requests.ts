// Request fixtures for all 91 commands (ipc.md §7.1), type-checked against the contract with
// `satisfies`. `fixtures.test.ts` writes each to `requests/<command>.json`, the file Rust's
// `ipc::fixture_requests_deserialize` reads, and fails if the JSON drifts from this source.
import type { Requests } from '../contract.ts';

const TL = '7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11';
const SW = '5e1f2a3b-4c5d-4e6f-8a7b-9c0d1e2f3a4b';
const CHAIN = '7f3a9c2e-51d4-4b8e-9e0a-2c61f0b4d871';
const RUN = 'a0000000-0000-4000-8000-000000000023';
const PREVIEW = 'e0000000-0000-4000-8000-000000000001';
const PLAN = 'f0000000-0000-4000-8000-000000000001';
const SHA = 'd4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3';

const patch = {
	driName: 'Billy Ribeiro',
	editor: null,
	theme: null,
	terminal: null,
	envPassthrough: null,
	globalVariables: null,
	resetTool: null,
	clearLoginPath: null,
	removeAllowlistEntry: null,
	screenReaderMode: null,
	osDisplay: null
};

export const requests = {
	app_bootstrap: {},
	settings_get: {},
	settings_update: { expectRev: 7, patch },
	settings_pick_tool: { tool: 'claude' },
	today_get: {},
	do_next: {},
	blueprint_pick_repo: { mode: 'autoDetect' },
	template_health: { projectId: null },
	doctor_run: { scope: 'p1' },
	doctor_capture_login_path: {},
	doctor_login_path_apply: { previewId: PREVIEW },
	probe_folder_open: {},
	probe_run: {},
	projects_list: {},
	setup_pick_folder: {},
	setup_preview: {
		pickId: 'd1aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
		name: 'TradeLog',
		slug: 'tradelog',
		variables: {
			ROUGH_IDEA: 'A trading journal that imports fills from my broker.',
			TARGET_USERS: 'Active traders'
		},
		conditions: { idea_known: 'yes', commercial: 'no', payments: 'undecided' },
		chatProjectUrl: 'https://claude.ai/project/01a7c4e2'
	},
	setup_apply: { previewId: PREVIEW },
	project_open: { projectId: TL },
	project_url_set: { projectId: TL, url: 'https://claude.ai/project/01a7c4e2' },
	condition_set: {
		projectId: TL,
		condition: 'payments',
		to: 'no',
		note: 'Free tool for the trading room'
	},
	variable_set: { projectId: TL, name: 'QUALITY_BAR', value: 'Production-ready' },
	blueprint_repin: { projectId: TL },
	project_remove: { projectId: TL },
	project_pin_set: { projectId: TL, slot: 1 },
	step_view: { projectId: TL, step: '2.4' },
	run_input_set: {
		projectId: TL,
		step: 'x-2-1',
		name: 'notes',
		value: 'Spoke with 6 members of the trading room.'
	},
	optional_part_set: { projectId: TL, step: 'x-2-1', partId: 'rerank', include: false },
	run_override_set: {
		projectId: TL,
		step: '2.4',
		over: { model: null, effort: 'xhigh', mode: null }
	},
	step_mark_done: { projectId: TL, step: '2.4' },
	step_skip: { projectId: TL, step: 'x-2-1', reason: 'Covered by the council' },
	action_preview: { projectId: TL, step: 'council-setup' },
	action_apply: { previewId: PREVIEW },
	wait_start: { projectId: TL, step: '2.4', until: '2026-10-26' },
	clipboard_copy: {
		projectId: TL,
		target: 'prompt',
		step: '2.4',
		chain: null,
		path: null,
		stage: null
	},
	chat_copy_and_open: {
		projectId: 'f0e1d2c3-b4a5-4697-8879-6a5b4c3d2e1f',
		step: '4.5',
		entry: 'stageView'
	},
	chat_link_set: {
		projectId: TL,
		chainStart: '1.1',
		source: { kind: 'text', url: 'https://claude.ai/chat/7c1e9a4b' }
	},
	paste_back_preview: { projectId: TL, step: '1.4', kind: 'auto', source: 'clipboard' },
	paste_back_apply: { previewId: PREVIEW },
	launch_prepare: { projectId: TL, step: '2.4', entry: 'stageView', mode: 'normal' },
	chain_resume_prepare: { chain: CHAIN },
	launch_start: { planId: PLAN },
	config_ack: {
		projectId: TL,
		cwd: 'spec',
		fingerprint: 'abababababababababababababababababababababababababababababababab'
	},
	auth_key_allowlist_add: { hit: 'd1aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' },
	project_env_approve: { projectId: TL, cwd: 'spec', sha256: SHA },
	pty_attach: { runId: RUN },
	pty_write: { runId: RUN, data: 'hello' },
	pty_paste: { runId: RUN },
	pty_resize: { runId: RUN, cols: 120, rows: 32 },
	pty_flow: { runId: RUN, paused: false },
	pty_interrupt: { runId: RUN },
	pty_terminate: { runId: RUN },
	rearm_start: { planId: PLAN },
	rearm_continue: { chain: CHAIN },
	terminal_link_open: { runId: RUN, url: 'https://code.claude.com/docs' },
	file_open: { projectId: TL, path: 'docs/handoff/stage-01.md' },
	file_reveal: { projectId: TL, path: 'docs/handoff/stage-01.md' },
	terminal_open_here: { projectId: TL },
	pending_changes_list: { projectId: TL },
	pending_change_accept: {
		projectId: TL,
		change: 'c0000000-0000-4000-8000-000000000001',
		seenSha256: SHA
	},
	pending_change_reject: {
		projectId: TL,
		change: 'c0000000-0000-4000-8000-000000000001',
		seenSha256: SHA
	},
	item_kind_confirm: { projectId: TL, item: 'stack', textSha256: SHA },
	progress_rebuild_preview: { projectId: TL },
	progress_rebuild_apply: { previewId: PREVIEW },
	packet_view: { projectId: TL, stage: 1 },
	packet_normalize_apply: { projectId: TL, stage: 1, seenSha256: SHA },
	packet_signature_confirm: { projectId: TL, stage: 1, seenSha256: SHA },
	packet_repair_prepare: { projectId: TL, stage: 1 },
	metrics_export: {},
	metrics_view: { projectId: null },
	runs_clear: {},
	runs_search: { query: 'council', projectId: null, cursor: null, limit: 50 },
	gate_view: { projectId: TL, stage: 2 },
	gate_item_attest: {
		projectId: TL,
		stage: 2,
		item: 'stack',
		outcome: {
			kind: 'done',
			note: 'Read r7-stack.md layer by layer and recorded my choice',
			link: null
		}
	},
	gate_import_results: { projectId: TL, stage: 2 },
	gate_import_confirm: {
		projectId: TL,
		stage: 2,
		item: 'stack',
		result: 'pass',
		evidence: 'r7-stack.md has every layer'
	},
	gate_fix_start: { projectId: TL, stage: 2, item: 'stack' },
	sign_preview: {
		projectId: SW,
		stage: 7,
		branch: { kind: 'current' },
		name: 'Billy Ribeiro',
		date: '2026-10-19'
	},
	sign_apply: { previewId: PREVIEW },
	sign_retry_commit: { projectId: SW, stage: 7 },
	tamper_restore: { projectId: TL, stage: 1, seenSha256: SHA },
	lessons_status: { projectId: TL, stage: 1 },
	manifest_view: { projectId: TL },
	manifest_mark: { projectId: TL, path: 'docs/handoff/stage-01.md', inProject: true },
	assembler_view: { projectId: TL },
	assembler_paste_preview: { projectId: TL, path: 'docs/tickets.md' },
	assembler_paste_apply: { previewId: PREVIEW },
	desktop_open: { projectId: TL, step: '2.4' },
	desktop_handoff: { chain: CHAIN },
	add_step: {
		projectId: TL,
		stage: 2,
		after: '2.4',
		copyConfigFrom: '2.4',
		title: 'Check the stack against the NFRs',
		text: 'Compare r7-stack.md with the NFR draft and list conflicts.',
		config: {
			surface: 'cli',
			cwd: 'spec',
			start: { model: 'opus', effort: 'high', mode: 'plan' },
			out: []
		},
		tag: 'optional'
	},
	add_item: { projectId: TL, stage: 2, text: 'The stack table names a runner-up for every layer' },
	run_sheet_export: { projectId: TL, stage: 2 }
} as const satisfies { [K in keyof Requests]: Requests[K] };

/** Extra request variants named in ipc.md (§4.5, §4.6, §4.10). */
export const requestVariants = {
	'launch_prepare.fresh': { ...requests.launch_prepare, mode: 'freshWithContext' },
	'paste_back_preview.sensor': { ...requests.paste_back_preview, source: 'sensor' },
	'metrics_view.project': { projectId: TL }
} as const satisfies {
	'launch_prepare.fresh': Requests['launch_prepare'];
	'paste_back_preview.sensor': Requests['paste_back_preview'];
	'metrics_view.project': Requests['metrics_view'];
};
