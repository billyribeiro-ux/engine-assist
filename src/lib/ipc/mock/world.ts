// The mock's in-memory world (ipc.md §7.2 rules 2–4): TradeLog, Foreman and Swings, at the
// moments the mockups draw, selected by scenario. Deterministic: a fixed clock and fixed IDs.
import { visibleSpans } from '#lib/render/escape.ts';

import type {
	CapabilityRow,
	ConditionId,
	DoctorCheck,
	DoctorLevel,
	GateItemView,
	InboxItem,
	PacketFieldView,
	PendingChange,
	ProbeOutcome,
	ReadOnly,
	RunInputView,
	RunSummary,
	SettingsView,
	SheetStatus,
	SignedInfo,
	StepState,
	TextSpan,
	TriState
} from '../types.ts';
import { CONDITIONS, STAGE_TITLES, TEMPLATE, UNANNOTATED, type TemplateStep } from './template.ts';

export const NOW = '2026-10-19T09:00:00Z';

/** What Billy copies from claude.ai after 1.4: the Stage 1 packet, then the press release. */
export const PASTE_STAGE1 = [
	'=== HANDOFF PACKET · Stage 1 → Stage 2 ===',
	'PROJECT: TradeLog',
	'APPROVED BY: ',
	'PROBLEM: Active traders keep journals in spreadsheets and stop after a losing day.',
	'USERS: Members of a trading education community.',
	'V1 FEATURES: Broker CSV import, setup tags, expectancy by setup.',
	'OPEN ITEMS: none',
	"NEXT: Stage 2 · Architect's Design Studio · Claude Code · Fable 5.1",
	'PRESS RELEASE',
	'TradeLog shows traders which setups pay.',
	'FAQ',
	'Q: Which brokers? A: Any broker that exports fills as CSV.'
].join('\n');
export const IDS = {
	tradelog: '7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11',
	foreman: 'f0e1d2c3-b4a5-4697-8879-6a5b4c3d2e1f',
	swings: '5e1f2a3b-4c5d-4e6f-8a7b-9c0d1e2f3a4b',
	oldnotes: '0d9c8b7a-6f5e-4d3c-8b2a-190817263544',
	chain22: '7f3a9c2e-51d4-4b8e-9e0a-2c61f0b4d871',
	run23: 'a0000000-0000-4000-8000-000000000023',
	runSwingsGate: 'a0000000-0000-4000-8000-000000000007',
	change1: 'c0000000-0000-4000-8000-000000000001'
} as const;

export type Scenario =
	| 'happy'
	| 'firstRun'
	| 'empty'
	| 'blockedVersion'
	| 'blockedAuth'
	| 'configChanged'
	| 'conflict'
	| 'readOnly'
	| 'brokenChain'
	| 'outsideChange'
	| 'tamper'
	| 'stage1'
	| 'stage4'
	| 'stage5'
	| 'invalidPacket'
	| 'annotationErrors';
export const SCENARIOS: readonly Scenario[] = [
	'happy',
	'firstRun',
	'empty',
	'blockedVersion',
	'blockedAuth',
	'configChanged',
	'conflict',
	'readOnly',
	'brokenChain',
	'outsideChange',
	'tamper',
	'stage1',
	'stage4',
	'stage5',
	'invalidPacket',
	'annotationErrors'
];

export interface MStep {
	tpl: TemplateStep;
	state: StepState;
	stateText: string;
	why: string | null;
	conditionNote: string | null;
	run: string | null;
	next: boolean;
	inputs: RunInputView[];
	parts: { partId: string; label: string; included: boolean; locked: boolean }[];
	copiedAt: string | null;
	chain: string | null;
}
export interface MStage {
	stage: number;
	status: SheetStatus;
	statusWhy: string | null;
	signed: SignedInfo | null;
	steps: MStep[];
	packetExists: boolean;
	packetProblems: number;
}
export interface MPacket {
	stage: number;
	line: string;
	fields: PacketFieldView[];
	raw: { line: number; text: string; bad: boolean }[];
	signedBy: string | null;
	writtenBy: string;
	sha: string;
	tampered: boolean;
}
export interface MProject {
	id: string;
	name: string;
	slug: string;
	displayPath: string;
	rev: number;
	readOnly: ReadOnly | null;
	pinned: number | null;
	branch: string;
	dri: string;
	chatProjectUrl: string | null;
	conditions: Record<ConditionId, TriState>;
	lockedConditions: boolean;
	variables: Record<string, string>;
	stages: MStage[];
	current: number;
	pending: PendingChange[];
	configNotice: 'warn' | 'blockAuth' | null;
	configAcked: boolean;
	packets: Record<number, MPacket>;
	gates: Record<number, GateItemView[]>;
	firstLaunchDone: boolean;
}
export interface MRun {
	summary: RunSummary;
	script: string[];
	live: boolean;
	written: number;
}
export interface World {
	scenario: Scenario;
	firstRun: boolean;
	settings: SettingsView;
	doctor: {
		level: DoctorLevel;
		checks: DoctorCheck[];
		capabilities: CapabilityRow[];
		probes: ProbeOutcome[];
	};
	projects: MProject[];
	runs: MRun[];
	inbox: InboxItem[];
	claudeVersion: string;
	floorAll: string | null;
	clipboard: string;
}

const sp = (text: string): TextSpan[] => visibleSpans(text);

// ---------------------------------------------------------------- settings and doctor
function settings(firstRun: boolean): SettingsView {
	return {
		rev: firstRun ? 0 : 7,
		driName: firstRun ? null : 'Billy Ribeiro',
		tools: {
			claude: {
				path: '/opt/homebrew/bin/claude',
				version: '2.1.293',
				autoDetected: true,
				problem: null
			},
			git: { path: '/usr/bin/git', version: '2.51.0', autoDetected: true, problem: null }
		},
		editorBundleId: firstRun ? null : 'com.microsoft.VSCode',
		blueprintRepo: firstRun
			? null
			: {
					displayPath: '~/claude-blueprint',
					commit: 'a3f9c1',
					version: '2.2',
					grammarVersion: 1,
					dirty: false,
					health: { runnable: '0 to 5', needsAnnotation: '6 to 18', errors: 1, warnings: 2 },
					variables: [],
					conditions: []
				},
		theme: 'system',
		terminal: { fontSize: 12, scrollback: 10000, macOptionIsMeta: true },
		envPassthrough: ['HTTPS_PROXY', 'NODE_EXTRA_CA_CERTS'],
		loginPath: firstRun
			? null
			: {
					capturedAt: '2026-10-12T10:20:00Z',
					entries: [
						'/opt/homebrew/bin',
						'/Users/billy/.cargo/bin',
						'/Users/billy/Library/pnpm',
						'/usr/bin',
						'/bin'
					]
				},
		globalVariables: {
			PREFERRED_STACK: 'SvelteKit + Rust/Axum + Postgres',
			QUALITY_BAR: 'Principal-engineer strict'
		},
		authKeyAllowlist: [],
		screenReaderMode: false,
		runHistory: { runs: 212, megabytes: 38 }
	};
}

function check(
	id: string,
	group: 'claude' | 'toolchain',
	name: string,
	level: DoctorLevel,
	detail: TextSpan[],
	found: string | null,
	foundMono = true,
	summary: string | null = null
): DoctorCheck {
	return { id, group, name, level, summary, detail, found, foundMono, action: null };
}
const code = (text: string): TextSpan => ({ kind: 'code', text });
const plain = (text: string): TextSpan => ({ kind: 'plain', text });
const strong = (text: string): TextSpan => ({ kind: 'strong', children: [plain(text)] });

export function doctorFor(version: string, firstRun: boolean): World['doctor'] {
	const below = version < '2.1.293';
	const checks: DoctorCheck[] = [
		check(
			'version',
			'claude',
			'Version',
			below ? 'failing' : 'ok',
			below
				? [
						strong(
							`Claude Code ${version} found. Haiku 5.5 in the Stage 5 test-runner agent needs 2.1.293 or later.`
						),
						plain(' Run '),
						code('claude update'),
						plain(', then check again.')
					]
				: [plain('Meets every model and flag the blueprint uses.')],
			version
		),
		check(
			'path',
			'claude',
			'Found at an allowed path',
			'ok',
			[plain('Foreman only launches this exact file.')],
			'/opt/homebrew/bin/claude'
		),
		check(
			'auth',
			'claude',
			'Signed in with your plan',
			'ok',
			[
				code('claude auth status'),
				plain(' in ~/Code/tradelog/spec reports '),
				strong('claude.ai'),
				plain('. No API key or helper in any config Foreman checked.')
			],
			'claude.ai',
			false
		)
	];
	if (!firstRun) {
		checks.push(
			check(
				'cleanup',
				'claude',
				'Transcripts kept',
				'warning',
				[
					plain(
						"cleanupPeriodDays is 30. Chains idle longer than that break. Raise it in ~/.claude/settings.json if your stages run long; Foreman doesn't edit that file."
					)
				],
				'30 days'
			),
			check(
				'hooks',
				'claude',
				'Hooks enabled',
				'ok',
				[plain("disableAllHooks isn't set, so the session sensor can run.")],
				'on',
				false
			),
			check(
				'allow',
				'claude',
				'Broad allow rules',
				'warning',
				[
					code('Bash(*)'),
					plain(' in user settings. No effect in v1; it matters for headless gates in v1.1.')
				],
				'1 rule',
				false
			),
			check(
				'toolchain',
				'toolchain',
				'git, node, pnpm, cargo',
				'ok',
				[plain('Found at fixed paths with '), code('--version'), plain(' only.')],
				'2.51.0 · 22.17.1 · 12.14.0 · 1.90.0'
			),
			{
				...check(
					'loginPath',
					'toolchain',
					'Login shell PATH',
					'unknown',
					[
						plain(
							"Not captured. Capturing runs your login shell once to read PATH, shows you the result, and stores it. It's the only shell Foreman ever starts."
						)
					],
					null
				),
				action: 'captureLoginPath'
			}
		);
	} else {
		checks.push(
			check(
				'git',
				'toolchain',
				'git',
				'ok',
				[plain('Found at a fixed path.')],
				'2.51.0 at /usr/bin/git',
				false
			)
		);
	}
	const caps: CapabilityRow[] = [
		['Fable 5.1', false, '2.1.257', null],
		['--permission-prompts', true, '2.1.259', null],
		['Opus 5.5', false, '2.1.280', null],
		['--agents file form', true, '2.1.281', null],
		['Sonnet 5.5', false, '2.1.284', null],
		['--desktop', true, '2.1.285', null],
		['Haiku 5.5', false, '2.1.293', 'Stage 5 Gate A (test-runner)']
	].map(([feature, codeF, minimum, blocks]) => ({
		feature: feature as string,
		code: codeF as boolean,
		minimum: minimum as string,
		found: version >= (minimum as string),
		blocks:
			version >= (minimum as string) ? null : ((blocks as string | null) ?? 'Steps that use it')
	}));
	const probes: ProbeOutcome[] = [
		{
			probe: [plain('Flags parse and per-launch hooks fire ('), code('--init-only'), plain(')')],
			result: 'pass',
			text: `Pass on ${version}`,
			detail: null
		},
		{
			probe: [code('--resume'), plain(' parses')],
			result: 'inconclusive',
			text: 'Inconclusive',
			detail: 'Timed out after 20 s. Keeping the last verified state, 2.1.284.'
		}
	];
	const level: DoctorLevel = checks.some((c) => c.level === 'failing')
		? 'failing'
		: checks.some((c) => c.level === 'warning')
			? 'warning'
			: 'ok';
	return { level, checks, capabilities: caps, probes };
}

// ---------------------------------------------------------------- steps and stages
const NA_REASON: Record<string, string> = {
	'0.1': 'you already know the idea',
	'0.2': 'you already know the idea',
	'1.3': "there's no demand to test for a free tool",
	'2.1': "{name} isn't sold, so there's no market to tear down",
	'3.1': "{name} isn't sold and takes no payments",
	'4.5': "there's no user interface to design",
	'5.5': "it doesn't run on a server"
};

function applies(
	tpl: TemplateStep,
	c: Record<ConditionId, TriState>
): { v: TriState; note: string | null } {
	if (tpl.key.startsWith('0.')) {
		const v = c.idea_known === 'yes' ? 'no' : c.idea_known === 'no' ? 'yes' : 'undecided';
		return { v, note: `idea_known = ${c.idea_known}` };
	}
	if (!tpl.when) return { v: 'yes', note: null };
	const vals = tpl.when.map((id) => c[id]);
	const note = tpl.when.map((id) => `${id} = ${c[id]}`).join(', ');
	if (vals.includes('yes')) return { v: 'yes', note: null };
	if (vals.includes('undecided')) return { v: 'undecided', note };
	return { v: 'no', note };
}

function input(
	name: string,
	label: string,
	token: string,
	value: string,
	remember: boolean
): RunInputView {
	return { name, label, token, multiline: true, remember, required: true, value, warning: null };
}

function makeStep(
	tpl: TemplateStep,
	p: { name: string; conditions: Record<ConditionId, TriState> }
): MStep {
	const a = applies(tpl, p.conditions);
	const st: MStep = {
		tpl,
		state: 'notStarted',
		stateText: 'Not started',
		why: null,
		conditionNote: null,
		run: null,
		next: false,
		inputs: [],
		parts: [],
		copiedAt: null,
		chain: null
	};
	if (tpl.tag === 'optional' && a.v === 'yes') st.stateText = 'Not started, optional';
	if (tpl.key === '2.6') st.stateText = 'Not started, writes the packet';
	if (tpl.kind === 'lessons') st.stateText = 'After the inspection items';
	if (tpl.key === 'x-2-1') {
		st.stateText = 'Not started, optional';
		st.inputs = [
			input(
				'notes',
				'Interview notes',
				'[interview notes]',
				'Spoke with 6 members of the trading room on 16 Oct. 4 of 6 keep their journal in a spreadsheet; all 6 skip it after a losing day.',
				true
			)
		];
		st.parts = [
			{
				partId: 'rerank',
				label: 'ask user-advocate to re-rank the pain map',
				included: true,
				locked: false
			}
		];
	}
	if (tpl.key === '1.4') {
		st.inputs = [
			input(
				'numbers',
				'Validation results',
				'[paste numbers]',
				"No paid test (1.3 didn't apply). Poll in the trading room, 14 Oct: 41 of 60 members would log trades weekly if import were automatic.",
				false
			)
		];
		st.parts = [
			{
				partId: 'press-release',
				label: 'launch-day press release and FAQ',
				included: true,
				locked: false
			}
		];
	}
	if (a.v === 'no') {
		st.state = 'notApplicable';
		st.stateText = 'Not applicable';
		st.why = `Not applicable: ${(NA_REASON[tpl.key] ?? 'its condition is false').replace('{name}', p.name)}`;
		st.conditionNote = a.note;
	} else if (a.v === 'undecided') {
		const cond =
			(tpl.when ?? ['idea_known']).find((id) => p.conditions[id] === 'undecided') ?? 'idea_known';
		st.state = 'undecided';
		st.stateText = 'Undecided';
		st.why = `Decide ${cond} to continue`;
		st.conditionNote = `${cond} = undecided`;
	}
	return st;
}

function done(st: MStep, text = 'Done'): void {
	if (st.state === 'notApplicable' || st.state === 'undecided') return;
	st.state = 'done';
	st.stateText = text;
}

function buildStages(
	p: { name: string; conditions: Record<ConditionId, TriState> },
	signedUpTo: number,
	current: number,
	signedDates: Record<number, string>
): MStage[] {
	const out: MStage[] = [];
	for (let n = 0; n < 19; n++) {
		const tpls = TEMPLATE[n];
		let steps: MStep[];
		if (tpls) steps = tpls.map((t) => makeStep(t, p));
		else
			steps = (UNANNOTATED[n] ?? []).map((title, i) => ({
				tpl: {
					key: `${n}.${i + 1}`,
					number: `${n}.${i + 1}`,
					title,
					kind: 'prompt',
					surface: 'cli',
					tag: 'core',
					when: null,
					where: 'Not annotated',
					model: null,
					effort: null,
					mode: null,
					agents: [],
					note: null
				},
				state: 'needsAnnotation',
				stateText: 'Needs annotation',
				why: 'Not runnable: needs annotation. Blueprint 2.2 annotates stages 0 to 5.',
				conditionNote: null,
				run: null,
				next: false,
				inputs: [],
				parts: [],
				copiedAt: null,
				chain: null
			}));
		let status: SheetStatus;
		let why: string | null = null;
		const stageNa =
			(n === 0 && p.conditions.idea_known === 'yes') ||
			(n === 10 && p.conditions.payments === 'no');
		if (stageNa) {
			status = 'notApplicable';
			why = n === 0 ? 'idea_known = yes' : 'payments = no';
			for (const s of steps) {
				s.state = 'notApplicable';
				s.stateText = 'Not applicable';
				s.why =
					n === 0
						? 'Not applicable: you already know the idea'
						: "Not applicable: this app doesn't take payments";
				s.conditionNote = why;
			}
		} else if (n === 10 && p.conditions.payments === 'undecided') {
			status = 'undecided';
			why = 'payments = undecided';
		} else if (n > 5) {
			status = 'needsAnnotation';
			why = 'blueprint 2.2 annotates stages 0 to 5';
		} else if (n <= signedUpTo) {
			status = 'signed';
			why = signedDates[n] ?? null;
			for (const s of steps) done(s);
		} else if (n === current) {
			status = 'inProgress';
		} else {
			status = 'locked';
			why = n === signedUpTo + 1 || n === current + 1 ? `until Stage ${n - 1} is signed` : null;
		}
		const signed: SignedInfo | null =
			status === 'signed'
				? {
						date: signedDates[n] ?? '2026-10-12',
						commit: '4e1b2c7',
						branch: `stage/0${n}`,
						merged: true
					}
				: null;
		out.push({
			stage: n,
			status,
			statusWhy: why,
			signed,
			steps,
			packetExists: status === 'signed',
			packetProblems: 0
		});
	}
	return out;
}

export function stepOf(p: MProject, key: string): MStep | undefined {
	for (const s of p.stages) {
		const st = s.steps.find((x) => x.tpl.key === key);
		if (st) return st;
	}
	return undefined;
}
function setStep(p: MProject, key: string, patch: Partial<MStep>): void {
	const st = stepOf(p, key);
	if (st) Object.assign(st, patch);
}

// ---------------------------------------------------------------- packets
function field(
	key: string,
	label: string,
	value: string,
	extra: Partial<PacketFieldView> = {}
): PacketFieldView {
	return {
		key,
		label,
		value: value ? [{ kind: 'paragraph', spans: sp(value) }] : [],
		missing: false,
		problems: [],
		note: null,
		...extra
	};
}

function stage1Packet(name: string): MPacket {
	return {
		stage: 1,
		line: '=== HANDOFF PACKET · Stage 1 → Stage 2 ===',
		fields: [
			field('PROJECT', 'Project', name),
			field('APPROVED BY', 'Approved by', 'Billy Ribeiro on 2026-10-12'),
			field(
				'PROBLEM',
				'Problem',
				'Active traders keep journals in spreadsheets and stop after a losing day, so they never learn which setups pay.'
			),
			field(
				'USERS',
				'Users',
				'Members of a trading education community who trade US equities and options.'
			),
			field('V1 FEATURES', 'V1 features', 'Broker CSV import, setup tags, expectancy by setup.'),
			field('OPEN ITEMS', 'Open items', 'none'),
			field('NEXT', 'Next', "Stage 2 · Architect's Design Studio · Claude Code · Fable 5.1")
		],
		raw: [
			'=== HANDOFF PACKET · Stage 1 → Stage 2 ===',
			`PROJECT: ${name}`,
			'APPROVED BY: Billy Ribeiro on 2026-10-12',
			'PROBLEM: Active traders keep journals in spreadsheets…',
			'USERS: Members of a trading education community…',
			'V1 FEATURES: Broker CSV import, setup tags, expectancy…',
			'OPEN ITEMS: none',
			"NEXT: Stage 2 · Architect's Design Studio · …"
		].map((text, i) => ({ line: i + 1, text, bad: false })),
		signedBy: 'Billy Ribeiro on 2026-10-12',
		writtenBy: 'Written by 1.4 on 12 Oct; pasted back from the chat.',
		sha: '9c41e07a3b5d6f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6e07a',
		tampered: false
	};
}

function stage2Packet(): MPacket {
	const problem = (message: string, line: number | null) => ({
		rule: 'packet-field',
		line,
		message,
		fix: ['repair', 'openInEditor'] as ('repair' | 'openInEditor')[]
	});
	return {
		stage: 2,
		line: '=== HANDOFF PACKET · Stage 2 → Stage 3 ===',
		fields: [
			field('PROJECT', 'Project', 'TradeLog'),
			field('APPROVED BY', 'Approved by', 'Unsigned', {
				note: [
					plain('(the file says '),
					code('<sign here>'),
					plain('; Foreman treats it as unsigned and writes your name at signing)')
				]
			}),
			field(
				'POSITIONING',
				'Positioning',
				'TradeLog turns broker fills into a journal you actually keep: every trade imported, tagged by setup, and scored by expectancy, so a trader sees which setups pay within a week.'
			),
			{
				...field('V1 FEATURES', 'V1 features', ''),
				value: [
					{
						kind: 'list',
						ordered: false,
						items: [
							[{ kind: 'paragraph', spans: sp('F-001 Broker CSV import with fill matching') }],
							[{ kind: 'paragraph', spans: sp('F-002 Setup tags and playbooks') }],
							[
								{ kind: 'paragraph', spans: sp('F-003 Expectancy by setup, time of day, and size') }
							],
							[{ kind: 'paragraph', spans: [{ kind: 'emph', children: [plain('and 11 more')] }] }]
						]
					}
				]
			},
			field('STACK (FINAL)', 'Stack (final)', 'SvelteKit 3 · Rust/Axum · Postgres 18 · Fly.io', {
				note: [plain('(r7-stack.md, your choice recorded)')]
			}),
			field('DATA WE HANDLE', 'Data we handle', '', {
				missing: true,
				problems: [
					problem(
						"The field DATA WE HANDLE is required for Stage 2 and isn't in the file. Stage 3's security and privacy requirements read it.",
						null
					)
				]
			}),
			{
				...field('OPEN ITEMS', 'Open items', ''),
				value: [
					{
						kind: 'codeBlock',
						info: '',
						text: 'Confirm the tier names with the trading room before Stage 3'
					}
				],
				problems: [
					problem(
						'OPEN ITEMS must be exactly "none". Resolve this item, or move it to KNOWN LIMITS with a ticket ID.',
						27
					)
				]
			},
			field('NEXT', 'Next', 'Stage 3 · Permits & Zoning · Claude chat · Opus 5.5', {
				note: [plain('(matches the next applicable stage)')]
			}),
			{
				...field('FILES', 'Files', ''),
				value: [
					{
						kind: 'paragraph',
						spans: [code('spec/MASTER-SPEC.md'), plain(', '), code('spec/council/r1…r8')]
					}
				]
			}
		],
		raw: [
			[1, '=== HANDOFF PACKET · Stage 2 → Stage 3 ==='],
			[2, 'PROJECT: TradeLog'],
			[3, 'APPROVED BY: <sign here>'],
			[4, 'POSITIONING: TradeLog turns broker fills…'],
			[18, 'STACK (FINAL): SvelteKit 3 · Rust/Axum …'],
			[23, 'NFRs: p95 import < 3 s for 10k fills …'],
			[26, 'PRICING SIGNALS: none (commercial = no)'],
			[27, 'OPEN ITEMS: Confirm the tier names with …'],
			[28, 'NEXT: Stage 3 · Permits & Zoning · …'],
			[29, 'FILES: MASTER-SPEC.md, council/r1…r8']
		].map(([line, text]) => ({ line: line as number, text: text as string, bad: line === 27 })),
		signedBy: null,
		writtenBy:
			'Written by 2.6 at 15:41. Foreman checks it every time the file changes; nothing here edits it.',
		sha: 'b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90a1',
		tampered: false
	};
}

// ---------------------------------------------------------------- gates
function gateItem(id: string, text: string, extra: Partial<GateItemView> = {}): GateItemView {
	return {
		id,
		text,
		kind: 'attest',
		how: null,
		status: 'waiting',
		evidence: null,
		proposed: null,
		note: null,
		waitsFor: null,
		artifacts: [],
		failures: 0,
		needsReconfirm: false,
		session: null,
		...extra
	};
}

function stage2Gate(): GateItemView[] {
	return [
		gateItem('feature-matrix', 'The feature matrix covers every serious competitor, with sources', {
			status: 'notApplicable',
			waitsFor: "Not applicable: 2.1 didn't run (commercial = no)"
		}),
		gateItem(
			'beats-competitor',
			"Every v1 feature beats a named competitor's version or is table stakes",
			{
				waitsFor: 'Attest, waits for 2.6'
			}
		),
		gateItem('red-team', 'Every fatal or serious red-team objection is resolved in writing', {
			waitsFor: 'Attest, waits for 2.6'
		}),
		gateItem(
			'stack',
			'The stack recommendation exists for every layer, plus your recorded final choice',
			{
				waitsFor: 'Attest, waits for 2.4'
			}
		),
		gateItem('frozen', 'Open questions: zero. Status: FROZEN v1.0', {
			waitsFor: 'Attest, waits for 2.6'
		})
	];
}

function stage5Gate(): GateItemView[] {
	return [
		gateItem(
			'ci-gates',
			'CI is green, and an intentional type error, a planted fake secret, and a known-vulnerable dependency each fail it',
			{
				how: 'Attest. Prove each gate fails, then revert.',
				status: 'fail',
				failures: 2,
				session: 'fm-tradelog-s05-5-gate-a',
				evidence: [
					plain('The planted secret in '),
					code('apps/api/.env.example'),
					plain(' passed CI: the gitleaks job is skipped for pull requests ('),
					code('.github/workflows/ci.yml:41'),
					plain(', '),
					code("if: github.event_name == 'push'"),
					plain('). Type error and vulnerable dependency failed as expected.')
				],
				artifacts: [
					{ kind: 'path', text: '.github/workflows/ci.yml:41' },
					{ kind: 'command', text: 'gh run view 11872 --log-failed' }
				]
			}
		),
		gateItem(
			'staging',
			'Staging deploys automatically, and a rollback has actually been run once',
			{
				proposed: 'pass',
				evidence: sp(
					'Import proposes PASS: deploy run #48 green on main; rollback to v0.1.2 at 14:20 in the deploy log.'
				),
				note: 'Rolled staging back from v0.1.3 to v0.1.2 with `pnpm deploy:rollback`; steps in docs/runbook-deploy.md §4.'
			}
		),
		gateItem('adr-check', 'The ADR reality check has no CONTRADICTED left open', {
			proposed: 'pass',
			evidence: sp('Import proposes PASS: 9 VALID, 1 NEEDS AMENDMENT (ADR-004, amended 18 Oct).'),
			note: 'Looks fine'
		}),
		gateItem('fresh-clone', 'A fresh clone, then install and dev, works with zero manual steps', {
			status: 'pass',
			note: 'Fresh clone on a clean user account, pnpm install && pnpm dev'
		})
	];
}

// ---------------------------------------------------------------- projects
const ALL_YES = Object.fromEntries(CONDITIONS.map((c) => [c.id, 'yes'])) as Record<
	ConditionId,
	TriState
>;

function baseProject(
	id: string,
	name: string,
	path: string,
	conditions: Record<ConditionId, TriState>,
	signedUpTo: number,
	current: number,
	dates: Record<number, string>
): MProject {
	return {
		id,
		name,
		slug: name.toLowerCase(),
		displayPath: path,
		rev: 41,
		readOnly: null,
		pinned: null,
		branch: 'main',
		dri: 'Billy Ribeiro',
		chatProjectUrl: 'https://claude.ai/project/01a7c4e2',
		conditions,
		lockedConditions: signedUpTo >= 2,
		variables: {
			APP_NAME: name,
			ROUGH_IDEA:
				'A trading journal that imports fills from my broker and shows which setups actually make money.',
			TARGET_USERS: 'Active traders in my trading room',
			PREFERRED_STACK: 'SvelteKit + Rust/Axum + Postgres',
			QUALITY_BAR: 'Principal-engineer strict'
		},
		stages: buildStages({ name, conditions }, signedUpTo, current, dates),
		current,
		pending: [],
		configNotice: null,
		configAcked: false,
		packets: {},
		gates: {},
		firstLaunchDone: true
	};
}

type Snap = 's1' | 's2' | 's2p' | 's4' | 's5';

function tradelog(snap: Snap): MProject {
	const c: Record<ConditionId, TriState> = {
		...ALL_YES,
		commercial: 'no',
		payments: snap === 's4' || snap === 's5' ? 'no' : 'undecided',
		has_database: snap === 's5' ? 'yes' : 'undecided',
		has_uploads: snap === 's5' ? 'no' : 'undecided',
		has_email: snap === 's5' ? 'no' : 'undecided',
		has_integrations: snap === 's1' || snap === 's2' || snap === 's2p' ? 'undecided' : 'no'
	};
	const signedUpTo = { s1: 0, s2: 1, s2p: 1, s4: 3, s5: 4 }[snap];
	const current = { s1: 1, s2: 2, s2p: 2, s4: 4, s5: 5 }[snap];
	const p = baseProject(IDS.tradelog, 'TradeLog', '~/Code/tradelog', c, signedUpTo, current, {
		1: '2026-10-12',
		2: '2026-10-14',
		3: '2026-10-16',
		4: '2026-10-18'
	});
	p.pinned = 1;
	p.branch = {
		s1: 'main',
		s2: 'stage/02-design',
		s2p: 'stage/02-design',
		s4: 'stage/04-drawings',
		s5: 'stage/05-site-prep'
	}[snap];
	if (signedUpTo >= 1) p.packets[1] = stage1Packet('TradeLog');
	p.gates[2] = stage2Gate();
	p.gates[5] = stage5Gate();
	if (snap === 's1') {
		setStep(p, '1.1', { state: 'done', stateText: 'Done, chat linked', chain: 'c-1-1' });
		setStep(p, '1.2', { state: 'done', stateText: 'Done', chain: 'c-1-1' });
		setStep(p, '1.4', {
			state: 'copied',
			stateText: 'Copied 6 min ago, waiting for the packet',
			copiedAt: '2026-10-19T08:54:00Z',
			next: true
		});
		p.lockedConditions = false;
		const s1 = p.stages[1];
		if (s1) s1.status = 'inProgress';
	}
	if (snap === 's2' || snap === 's2p') {
		p.lockedConditions = false;
		setStep(p, 'council-setup', { state: 'done', stateText: 'Done, wrote 4 agents' });
		setStep(p, '2.2', { state: 'done', stateText: 'Done, 8 files', chain: IDS.chain22 });
		setStep(p, '2.3', {
			state: 'running',
			stateText: 'Running in fm-tradelog-s02-2-3',
			run: IDS.run23,
			chain: IDS.chain22
		});
		setStep(p, '2.4', {
			stateText: 'Next, waits for 2.3',
			next: true,
			why: "Waits for 2.3: Claude is still working in this session. Run re-arms it with Accept edits when it's idle."
		});
		setStep(p, 'x-2-1', { why: 'Added steps always confirm before running.' });
		p.configNotice = 'warn';
		p.pending = [pendingChange()];
		if (snap === 's2p') {
			setStep(p, '2.3', { state: 'done', stateText: 'Done, 412 lines', run: null });
			for (const k of ['2.4', 'x-2-1', '2.5', '2.6'])
				setStep(p, k, { state: 'done', stateText: 'Done', next: false, why: null });
			setStep(p, '2.6', { stateText: 'Done, wrote the packet' });
			p.packets[2] = stage2Packet();
			const s2 = p.stages[2];
			if (s2) {
				s2.packetExists = true;
				s2.packetProblems = 2;
				s2.status = 'inspecting';
			}
			p.configNotice = null;
			p.pending = [];
		}
	}
	if (snap === 's4') {
		const s4 = p.stages[4];
		if (s4) {
			for (const st of s4.steps.slice(0, 8)) done(st);
			s4.packetExists = true;
			s4.statusWhy = 'assembling the build package';
		}
		setStep(p, '4:F.2', { stateText: 'After the inspection items' });
	}
	if (snap === 's5') {
		const s5 = p.stages[5];
		if (s5) {
			s5.status = 'inspecting';
			s5.statusWhy = 'gate in progress';
			for (const st of s5.steps.slice(0, 5)) done(st);
			s5.packetExists = false;
		}
		setStep(p, '5:gate.A', { state: 'done', stateText: 'Done, 1 failed' });
	}
	return p;
}

function foreman(): MProject {
	const c: Record<ConditionId, TriState> = Object.fromEntries(
		CONDITIONS.map((x) => [x.id, 'no'])
	) as Record<ConditionId, TriState>;
	c.idea_known = 'yes';
	c.has_ui = 'yes';
	const p = baseProject(IDS.foreman, 'Foreman', '~/Code/engine-assist', c, 3, 4, {
		1: '2026-10-02',
		2: '2026-10-09',
		3: '2026-10-11'
	});
	p.slug = 'foreman';
	p.pinned = 2;
	p.branch = 'stage/04-drawings';
	p.variables.APP_NAME = 'Foreman';
	p.chatProjectUrl = 'https://claude.ai/project/01b9f00d';
	const s4 = p.stages[4];
	if (s4) for (const st of s4.steps.slice(0, 4)) done(st);
	setStep(p, '4.5', { next: true });
	// Foreman's own policy: fable → opus@xhigh (AC-F005-03).
	return p;
}

function swings(): MProject {
	const p = baseProject(
		IDS.swings,
		'Swings',
		'~/Code/swings-platform',
		{ ...ALL_YES, commercial: 'yes' },
		6,
		7,
		{}
	);
	p.slug = 'swings';
	p.branch = 'stage/07-framing';
	for (const s of p.stages) {
		if (s.stage <= 6 && s.stage > 0) {
			s.status = 'signed';
			s.signed = {
				date: '2026-09-28',
				commit: '1b2c3d4',
				branch: `stage/0${s.stage}`,
				merged: true
			};
		}
		if (s.stage === 7) {
			s.status = 'waiting';
			s.statusWhy = 'packet waiting for your signature';
			s.packetExists = true;
		}
		if (s.stage > 7) {
			s.status = 'locked';
			s.statusWhy = null;
		}
	}
	const s0 = p.stages[0];
	if (s0) s0.status = 'signed';
	return p;
}

function pendingChange(): PendingChange {
	return {
		id: IDS.change1,
		file: 'overlay',
		detectedAt: '2026-10-19T14:02:00Z',
		seenSha256: 'd4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3',
		diff: {
			source: '.foreman/overlay.json',
			entries: [
				{
					keyPath: 'skips',
					change: 'added',
					before: null,
					after: '{ "step": "2.5", "reason": "NFRs are covered in r6-feature-specs.md" }'
				}
			]
		},
		effect: 'noEffect',
		consequences: [
			{
				glyph: 'lock',
				text: "2.5 Non-functional requirements is core. Accepting records the skip, but Foreman ignores it and flags it: core steps can't be skipped."
			}
		],
		context: {
			changed: 'Today at 14:02, while fm-tradelog-s02-2-3 was running in spec/',
			git: 'Uncommitted; no commit touches these files since your last approval',
			approved: 'sha256 9c41…e07a, approved 12 Oct'
		}
	};
}

export function projectFilePending(): PendingChange {
	return {
		id: 'c0000000-0000-4000-8000-000000000002',
		file: 'project',
		detectedAt: '2026-10-19T14:02:00Z',
		seenSha256: 'e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4',
		diff: {
			source: '.foreman/project.json',
			entries: [
				{
					keyPath: 'conditions.payments',
					change: 'valueChanged',
					before: '"undecided"',
					after: '"no"'
				},
				{
					keyPath: 'variables.QUALITY_BAR',
					change: 'valueChanged',
					before: '"Principal-engineer strict"',
					after: '"Production-ready"'
				}
			]
		},
		effect: 'changesPath',
		consequences: [
			{
				glyph: 'info',
				text: 'payments: no makes Stage 10, Electrical, not applicable and 3.1 pricing not applicable. Foreman adds the audit line "changed outside Foreman, accepted by you".'
			},
			{
				glyph: 'check',
				text: 'QUALITY_BAR is a variable, not a decision. It already reloaded; prompts that use it show the new value.'
			}
		],
		context: pendingChange().context
	};
}

// ---------------------------------------------------------------- runs
export const TRANSCRIPT_23 = [
	'\x1b[2m── 2.3 Feature specifications · Fable 5.1 · high · Accept edits · permission grants reset ──\x1b[0m\r\n',
	'\x1b[2m> \x1b[0multrathink. For each v1 feature, write a specification a senior engineer could build from…\r\n',
	'\r\n',
	'\x1b[32m⏺\x1b[0m Read(\x1b[34mcouncil/r5-resolution.md\x1b[0m) \x1b[2m⎿ 618 lines\x1b[0m\r\n',
	'\x1b[32m⏺\x1b[0m Write(\x1b[34mcouncil/r6-feature-specs.md\x1b[0m) \x1b[2m⎿ Wrote 412 lines\x1b[0m\r\n',
	'\x1b[33m✻\x1b[0m Specifying F-014 Import trades from broker CSVs… \x1b[2m(3m 12s · esc to interrupt)\x1b[0m\r\n',
	'\r\n',
	'\x1b[2m╭──────────────────────────────────────────────────────────────╮\x1b[0m\r\n',
	'\x1b[2m│\x1b[0m > \r\n',
	'\x1b[2m╰──────────────────────────────────────────────────────────────╯\x1b[0m\r\n',
	'  \x1b[2m⏵⏵ accept edits on (shift+tab to cycle)\x1b[0m\r\n'
];

export function launchTranscript(tab: string, title: string, rearm: string | null): string[] {
	const lines = rearm ? [`\x1b[2m── ${rearm} ──\x1b[0m\r\n`] : [];
	return [
		...lines,
		`\x1b[2mfake-claude · ${tab}\x1b[0m\r\n`,
		`\x1b[2m> \x1b[0m${title}\r\n`,
		'\r\n',
		'\x1b[32m⏺\x1b[0m Read(\x1b[34mdocs/handoff/stage-01.md\x1b[0m) \x1b[2m⎿ 64 lines\x1b[0m\r\n',
		'\x1b[33m✻\x1b[0m Working… \x1b[2m(esc to interrupt)\x1b[0m\r\n',
		'\r\n',
		'\x1b[2m│\x1b[0m > '
	];
}

function run(
	id: string,
	project: MProject,
	step: string,
	stepTitle: string,
	tab: string,
	ranWith: string,
	status: RunSummary['status'],
	startedAt: string,
	tts: number,
	extra: Partial<RunSummary> = {}
): RunSummary {
	return {
		runId: id,
		chain: IDS.chain22,
		project: project.id,
		projectName: project.name,
		step,
		stepTitle,
		tabName: tab,
		ranWith,
		substituted: null,
		status,
		exitCode: status === 'exited' ? 0 : null,
		error: null,
		startedAt,
		timeToStartMs: tts,
		argv: [
			'/opt/homebrew/bin/claude',
			'--session-id',
			IDS.chain22,
			'--model',
			'fable',
			'--effort',
			'high',
			'--permission-mode',
			'plan',
			'--name',
			tab,
			'--settings',
			'…/runs/r_0188/settings.json',
			'<prompt sha256:3f9a…c21e>'
		],
		resumable: status !== 'interrupted',
		whyNotResumable: null,
		...extra
	};
}

function runsFor(tl: MProject, fm: MProject, sw: MProject): MRun[] {
	const r = (summary: RunSummary, script: string[] = [], live = false): MRun => ({
		summary,
		script,
		live,
		written: 0
	});
	return [
		r(
			run(
				IDS.run23,
				tl,
				'2.3',
				'Feature specifications',
				'fm-tradelog-s02-2-3',
				'Fable 5.1, high, Accept edits',
				'running',
				'2026-10-19T14:06:00Z',
				4100
			),
			TRANSCRIPT_23,
			true
		),
		r(
			run(
				'a0000000-0000-4000-8000-000000000022',
				tl,
				'2.2',
				'Convene the council',
				'fm-tradelog-s02-2-2',
				'Fable 5.1, high, Plan',
				'exited',
				'2026-10-19T11:20:00Z',
				7800
			)
		),
		r(
			run(
				'a0000000-0000-4000-8000-000000000122',
				fm,
				'2.2',
				'Convene the council',
				'fm-foreman-s02-2-2',
				'Opus 5.5, xhigh, Plan',
				'exited',
				'2026-10-16T09:12:00Z',
				5300,
				{
					substituted: 'substituted for Fable',
					chain: '8a4b0d3f-62e5-4c9f-8f1b-3d72a1c5e982'
				}
			)
		),
		r(
			run(
				'a0000000-0000-4000-8000-000000000071',
				sw,
				'7.1',
				'Skeleton plan',
				'fm-swings-s07-7-1',
				'Opus 5.5, high, Plan',
				'interrupted',
				'2026-09-02T15:40:00Z',
				6000,
				{
					resumable: false,
					whyNotResumable: 'Transcript removed after 30 days',
					chain: '9b5c1e4a-73f6-4d0a-9a2c-4e83b2d6f093'
				}
			)
		),
		r(
			run(
				IDS.runSwingsGate,
				sw,
				'6:gate.A',
				'Gate A',
				'fm-swings-s06-6-gate-a',
				'Opus 5.5, high, Plan',
				'errored',
				'2026-09-01T10:02:00Z',
				3200,
				{
					error: 'Child exited during spawn: unknown option --agents',
					exitCode: 2,
					resumable: false,
					chain: '0c6d2f5b-8407-4e1b-8b3d-5f94c3e7a104'
				}
			),
			['\x1b[2mfm-swings-s06-6-gate-a\x1b[0m\r\n', 'error: unknown option --agents\r\n'],
			false
		)
	];
}

function inboxFor(tl: MProject, fm: MProject, sw: MProject): InboxItem[] {
	const item = (
		kind: InboxItem['kind'],
		p: MProject,
		title: string,
		detail: string,
		extra: Partial<InboxItem> = {}
	): InboxItem => ({
		kind,
		project: p.id,
		step: null,
		stage: null,
		at: NOW,
		projectName: p.name,
		title,
		detail,
		chain: null,
		run: null,
		...extra
	});
	return [
		item(
			'sessionBlocked',
			tl,
			'Claude is asking for permission in 2.3 Feature specifications',
			'Bash(pnpm view @sveltejs/kit version) in fm-tradelog-s02-2-3, 2 min ago',
			{ step: '2.3', stage: 2, run: IDS.run23 }
		),
		item(
			'packetInvalid',
			fm,
			'Stage 4 packet has 2 problems',
			'OPEN ITEMS isn\'t "none"; BLUEPRINT VERSION says 2.1, the lock says 2.2',
			{
				stage: 4
			}
		),
		item(
			'gateSignable',
			sw,
			'Stage 7 packet is valid and ready to sign',
			'docs/handoff/stage-07.md, gate passed 4 of 4, lessons found (L-031)',
			{
				stage: 7
			}
		),
		item(
			'outsideChange',
			tl,
			'.foreman/overlay.json changed outside Foreman',
			'1 skip added while fm-tradelog-s02-2-3 was running. It has no effect until you accept it.'
		),
		item(
			'interruptedSession',
			sw,
			"7.1 Skeleton plan can't resume",
			'Its conversation no longer exists: Claude Code removes transcripts after 30 days (cleanupPeriodDays)',
			{ step: '7.1', stage: 7, chain: '9b5c1e4a-73f6-4d0a-9a2c-4e83b2d6f093' }
		)
	];
}

// ---------------------------------------------------------------- scenarios
export function buildWorld(scenario: Scenario): World {
	const firstRun = scenario === 'firstRun';
	const snap: Snap =
		scenario === 'stage1'
			? 's1'
			: scenario === 'stage4'
				? 's4'
				: scenario === 'stage5'
					? 's5'
					: scenario === 'invalidPacket'
						? 's2p'
						: 's2';
	const version = scenario === 'blockedVersion' ? '2.1.270' : firstRun ? '2.1.293' : '2.1.288';
	const w: World = {
		scenario,
		firstRun,
		settings: settings(firstRun),
		doctor: doctorFor(version, firstRun),
		projects: [],
		runs: [],
		inbox: [],
		claudeVersion: version,
		floorAll: scenario === 'blockedVersion' ? '2.1.280' : null,
		clipboard: ''
	};
	w.settings.tools.claude.version = version;
	if (firstRun || scenario === 'empty') {
		if (scenario === 'empty') w.doctor = doctorFor('2.1.293', true);
		return w;
	}
	const tl = tradelog(snap);
	const fm = foreman();
	const sw = swings();
	w.projects = [tl, fm, sw];
	w.runs = runsFor(tl, fm, sw);
	w.inbox = inboxFor(tl, fm, sw);
	if (snap !== 's2') {
		w.inbox = w.inbox.filter((i) => i.kind !== 'outsideChange' && i.kind !== 'sessionBlocked');
		const r23 = w.runs[0];
		if (r23) {
			r23.live = false;
			r23.summary.status = 'exited';
			r23.summary.exitCode = 0;
		}
	}
	if (scenario === 'blockedAuth') tl.configNotice = 'blockAuth';
	if (scenario === 'outsideChange') tl.pending = [pendingChange(), projectFilePending()];
	if (scenario === 'readOnly') {
		tl.readOnly = {
			reason: 'lockMismatch',
			detail:
				"The blueprint file at a3f9c1 doesn't match the hash in .foreman/blueprint.lock.json. Restore it from the blueprint repo or re-pin before Stage 1 is signed."
		};
		w.projects.push({
			...baseProject(IDS.oldnotes, 'Old Notes', '~/Code/old-notes', ALL_YES, 1, 2, {}),
			readOnly: { reason: 'folderMissing', detail: 'Project folder not found: ~/Code/old-notes' }
		});
	}
	if (scenario === 'tamper') {
		const pk = tl.packets[1];
		if (pk) pk.tampered = true;
		const st = tl.stages[1];
		if (st) st.packetProblems = 1;
	}
	if (scenario === 'brokenChain') {
		setStep(tl, '2.3', { state: 'done', stateText: 'Done, 412 lines', run: null });
		setStep(tl, '2.4', { why: null });
		const r23 = w.runs[0];
		if (r23) {
			r23.live = false;
			r23.summary.status = 'interrupted';
			r23.summary.resumable = false;
			r23.summary.whyNotResumable = 'Transcript removed after 30 days';
		}
	}
	return w;
}

export { CONDITIONS, STAGE_TITLES };
