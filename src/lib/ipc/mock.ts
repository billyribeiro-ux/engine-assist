// MockIpcClient (ADR-004, docs/contracts/ipc.md §7.2): browser preview (`pnpm dev:mock`), Vitest
// and Playwright. Never bundled in release (ipc::mock_not_in_release greps for the marker below).
//
// Rules mirrored from Rust (§7.2): every command answers from in-memory, deterministic state;
// unknown request fields are `Invalid`; one-time preview and plan IDs expire; native dialogs are
// scripted (`setDialogAnswer`), so the webview never approves anything itself (SR-24a); mutations
// emit the same events with the next `rev`; PTY output is raw bytes through the same callback.
import { isHidden, visibleSpans, visibleText } from '#lib/render/escape.ts';

import type { IpcClient, Requests, Responses } from './contract.ts';
import { COMMAND_MAP, type CommandName } from './contract.ts';
import { IPC_ERROR_TEXT, IpcFailure } from './errors.ts';
import { requests as REQUEST_FIXTURES } from './fixtures/requests.ts';
import { PROMPTS, DEFAULT_PROMPT } from './mock/prompts.ts';
import { CONDITIONS, PHASES, STAGE_SUBTITLES, STAGE_TITLES, VARIABLES } from './mock/template.ts';
import {
	buildWorld,
	IDS,
	launchTranscript,
	NOW,
	SCENARIOS,
	stepOf,
	type MProject,
	type MStage,
	type MStep,
	type Scenario,
	type World
} from './mock/world.ts';
import type * as T from './types.ts';

export const MOCK_IPC_MARKER = '__FOREMAN_MOCK_IPC__';
export type DialogAnswer = 'ok' | 'cancel';
export { SCENARIOS, type Scenario };

type Handler = (payload: unknown) => void;
interface Plan {
	project: string;
	step: string | null;
	chain: string | null;
	confirm: T.ConfirmReason[];
	rearm: T.RearmNeed | null;
	created: number;
	used: boolean;
	tab: string;
	title: string;
	fresh: boolean;
	rearmText: string | null;
}
interface Preview {
	kind: string;
	project: string;
	data: unknown;
	used: boolean;
	created: number;
}

const MODEL_ENUM: Record<string, T.Model> = {
	'Fable 5.1': 'fable',
	'Opus 5.5': 'opus',
	'Sonnet 5.5': 'sonnet',
	'Haiku 5.5': 'haiku'
};
const MODE_ENUM: Record<string, T.Mode> = {
	Plan: 'plan',
	'Plan, then Accept edits': 'plan',
	'Accept edits': 'acceptEdits',
	Manual: 'default',
	Auto: 'auto',
	"Don't ask": 'dontAsk'
};
const EFFORTS: readonly T.Effort[] = [
	'low',
	'medium',
	'high',
	'xhigh',
	'max',
	'ultracode',
	'adaptive'
];

function fail(code: T.IpcErrorCode, detail: T.IpcErrorDetail | null = null): IpcFailure {
	return new IpcFailure({ code, message: IPC_ERROR_TEXT[code], detail });
}
const notAllowed = (rule: string) => fail('notAllowed', { kind: 'notAllowed', rule });
const notFound = () => fail('notFound');
const cancelled = () => fail('cancelled');
const invalidField = (field: string, rule: string) =>
	fail('invalid', { kind: 'invalid', field, rule });

/** SR-28: controls, bidi and zero-width characters are removed before text reaches a session. */
function stripControls(text: string): { text: string; removed: number } {
	let out = '';
	let removed = 0;
	for (const ch of text) {
		if (isHidden(ch.codePointAt(0) ?? 0)) removed += 1;
		else out += ch;
	}
	return { text: out, removed };
}

function clone<T>(v: T): T {
	return structuredClone(v);
}
const enc = new TextEncoder();

export class MockIpcClient implements IpcClient {
	readonly marker = MOCK_IPC_MARKER;
	world: World;
	private dialog: DialogAnswer = 'ok';
	private handlers = new Map<string, Set<Handler>>();
	private counter = 0;
	private plans = new Map<string, Plan>();
	private previews = new Map<string, Preview>();
	private outputs = new Map<string, Set<T.PtyOutput>>();
	private lineBuf = new Map<string, string>();
	private paused = new Set<string>();
	private picks = 0;
	private pickIds = new Map<string, string>();
	/** Scripted output delay; 0 delivers synchronously after a microtask (tests). */
	streamDelayMs = 8;

	constructor(scenario: Scenario = 'happy') {
		this.world = buildWorld(scenario);
	}

	// ---------------------------------------------------------------- test and preview controls
	setScenario(s: Scenario): void {
		this.world = buildWorld(s);
		this.plans.clear();
		this.previews.clear();
		this.counter = 0;
		this.picks = 0;
		this.overrides.clear();
		this.outputs.clear();
		this.emit('projects:changed', { rev: ++this.revCounter });
		this.emit('settings:changed', { rev: this.world.settings.rev });
	}
	setDialogAnswer(a: DialogAnswer): void {
		this.dialog = a;
	}
	setClipboard(text: string): void {
		this.world.clipboard = text;
	}
	getClipboard(): string {
		return this.world.clipboard;
	}

	// ---------------------------------------------------------------- plumbing
	private revCounter = 100;
	private uuid(prefix = 'b'): string {
		this.counter += 1;
		return `${prefix}0000000-0000-4000-8000-${this.counter.toString(16).padStart(12, '0')}`;
	}
	private guard(cmd: CommandName, req: unknown): void {
		if (typeof req !== 'object' || req === null || Array.isArray(req))
			throw invalidField('req', 'shape');
		const allowed = Object.keys(REQUEST_FIXTURES[cmd]);
		for (const k of Object.keys(req))
			if (!allowed.includes(k)) throw invalidField('req', 'unknownField');
	}
	private async run<C extends CommandName>(
		cmd: C,
		req: Requests[C],
		body: () => Responses[C]
	): Promise<Responses[C]> {
		await Promise.resolve();
		this.guard(cmd, req);
		return clone(body());
	}
	private ask(): void {
		if (this.dialog === 'cancel') throw cancelled();
	}
	private emit<E extends T.EventName>(event: E, payload: T.EventPayloads[E]): void {
		const set = this.handlers.get(event);
		if (!set) return;
		queueMicrotask(() => {
			for (const h of set) h(payload);
		});
	}
	listen<E extends T.EventName>(
		event: E,
		handler: (p: T.EventPayloads[E]) => void
	): Promise<T.Unlisten> {
		let set = this.handlers.get(event);
		if (!set) {
			set = new Set();
			this.handlers.set(event, set);
		}
		const h = handler as Handler;
		set.add(h);
		return Promise.resolve(() => {
			set.delete(h);
		});
	}
	private project(id: string): MProject {
		const p = this.world.projects.find((x) => x.id === id);
		if (!p) throw notFound();
		return p;
	}
	private writable(p: MProject): void {
		if (p.readOnly) throw fail('readOnly', { kind: 'readOnly', ...p.readOnly });
		if (this.world.scenario === 'conflict')
			throw fail('conflict', { kind: 'conflict', path: '.foreman/project.json', written: [] });
	}
	private touch(p: MProject): void {
		p.rev += 1;
		this.emit('project:changed', { projectId: p.id, rev: p.rev });
	}
	private step(p: MProject, key: string): MStep {
		const st = stepOf(p, key);
		if (!st) throw notFound();
		return st;
	}
	private stageOf(p: MProject, key: string): MStage {
		const s = p.stages.find((x) => x.steps.some((st) => st.tpl.key === key));
		if (!s) throw notFound();
		return s;
	}
	private preview(kind: string, project: string, data: unknown): string {
		const id = this.uuid('e');
		this.previews.set(id, { kind, project, data, used: false, created: performance.now() });
		return id;
	}
	private take(id: string, kind: string): Preview {
		const pv = this.previews.get(id);
		if (pv?.kind !== kind) throw notFound();
		if (pv.used || performance.now() - pv.created > 600_000) throw fail('expired');
		pv.used = true;
		return pv;
	}

	// ---------------------------------------------------------------- views
	private summary(p: MProject): T.ProjectSummary {
		return {
			id: p.id,
			name: p.name,
			displayPath: p.displayPath,
			rev: p.rev,
			readOnly: p.readOnly,
			currentStage: p.current,
			pinned: p.pinned
		};
	}
	private explained(
		value: string | null,
		foreman: boolean,
		kind: 'model' | 'effort'
	): T.Explained<string> | null {
		if (value === null) return null;
		if (foreman && kind === 'model' && value === 'Fable 5.1')
			return {
				value: 'Opus 5.5',
				origin: { kind: 'substitution', rule: 'fable → opus@xhigh (Max)' },
				from: 'Fable 5.1'
			};
		return { value, origin: { kind: 'template' }, from: null };
	}
	private specLine(p: MProject, st: MStep): T.SpecLine | null {
		const t = st.tpl;
		if (st.state === 'needsAnnotation') return null;
		const foreman = p.id === IDS.foreman;
		const model = this.explained(t.model, foreman, 'model');
		let effort = this.explained(t.effort, foreman, 'effort');
		if (model?.origin.kind === 'substitution' && effort)
			effort = {
				value: 'xhigh',
				origin: { kind: 'substitution', rule: 'fable → opus@xhigh (Max)' },
				from: effort.value
			};
		const over = this.overrides.get(`${p.id}/${t.key}`);
		const mode = t.mode
			? { value: t.mode, origin: { kind: 'template' } as T.ValueOrigin, from: null }
			: null;
		const line: T.SpecLine = {
			surface: t.surface,
			where: t.where,
			model,
			effort,
			mode,
			agents: t.agents,
			droppedAgents: t.key === '2.4' && p.conditions.commercial === 'no' ? ['market-analyst'] : [],
			note: t.note
		};
		if (over?.effort && line.effort)
			line.effort = { value: over.effort, origin: { kind: 'override' }, from: line.effort.value };
		if (over?.model && line.model) {
			const name = Object.entries(MODEL_ENUM).find(([, v]) => v === over.model)?.[0] ?? over.model;
			line.model = { value: name, origin: { kind: 'override' }, from: line.model.value };
		}
		return line;
	}
	private stepSummary(p: MProject, st: MStep): T.StepSummary {
		return {
			key: st.tpl.key,
			number: st.tpl.number,
			title: st.tpl.title,
			state: st.state,
			tag: st.tpl.tag,
			surface: st.tpl.surface === 'action' ? null : st.tpl.surface,
			specLine: this.specLine(p, st),
			whyNotRunnable: st.why,
			stateText: st.stateText,
			conditionNote: st.conditionNote,
			kind: st.tpl.kind,
			run: st.run,
			next: st.next
		};
	}
	private stageView(p: MProject, s: MStage): T.StageView {
		const applicable: T.Applicability =
			s.status === 'notApplicable'
				? { kind: 'notApplicable', reason: s.statusWhy ?? '' }
				: s.status === 'undecided'
					? { kind: 'undecided', condition: 'payments' }
					: s.status === 'needsAnnotation'
						? { kind: 'needsAnnotation' }
						: { kind: 'applies' };
		const pk = p.packets[s.stage];
		return {
			stage: s.stage,
			title: STAGE_TITLES[s.stage] ?? `Stage ${s.stage}`,
			applicable,
			signed: s.signed,
			steps: s.steps.map((st) => this.stepSummary(p, st)),
			packet:
				s.stage <= 17
					? {
							path: `docs/handoff/stage-${String(s.stage).padStart(2, '0')}.md`,
							exists: s.packetExists || !!pk,
							problems: s.packetProblems,
							signed: s.status === 'signed'
						}
					: null,
			subtitle: STAGE_SUBTITLES[s.stage] ?? '',
			status: s.status,
			statusWhy: s.statusWhy
		};
	}
	private conditionViews(p: MProject): Partial<Record<T.ConditionId, T.ConditionView>> {
		const out: Partial<Record<T.ConditionId, T.ConditionView>> = {};
		for (const c of CONDITIONS) {
			const lateLocked = c.late && p.stages[4]?.status === 'signed';
			const earlyLocked = !c.late && p.lockedConditions;
			out[c.id] = {
				value: p.conditions[c.id],
				locked: earlyLocked || lateLocked,
				lockRule: earlyLocked
					? 'condition-locked-after-stage-2'
					: lateLocked
						? 'late-condition-locked-after-stage-4'
						: null,
				label: c.label,
				effect: c.effect,
				late: c.late
			};
		}
		return out;
	}
	private configNotice(p: MProject): T.ConfigChangedNotice | null {
		if (!p.configNotice || (p.configNotice === 'warn' && p.configAcked)) return null;
		if (p.configNotice === 'blockAuth')
			return {
				cwd: 'spec',
				fingerprint: 'f1'.repeat(32),
				blocking: true,
				title: 'apiKeyHelper in spec/.claude/settings.json would move billing off your plan.',
				detail: 'Remove it, then check again.',
				diffs: []
			};
		return {
			cwd: 'spec',
			fingerprint: 'ab'.repeat(32),
			blocking: false,
			title: 'Claude Code configuration in spec/ changed.',
			detail: 'principal-engineer.md was edited at 13:52. Review it before 2.4 launches.',
			diffs: [
				{
					source: 'spec/.claude/agents/principal-engineer.md',
					entries: [
						{
							keyPath: 'tools',
							change: 'valueChanged',
							before: '"Read, Grep, Glob"',
							after: '"Read, Grep, Glob, Bash"'
						},
						{ keyPath: 'model', change: 'valueChanged', before: '"fable"', after: '"opus"' }
					]
				}
			]
		};
	}
	private projectView(p: MProject): T.ProjectView {
		return {
			summary: this.summary(p),
			conditions: this.conditionViews(p),
			stages: p.stages.map((s) => this.stageView(p, s)),
			pendingChanges: p.pending.length,
			readOnly: p.readOnly,
			branch: p.branch,
			blueprint: { version: '2.2', commit: 'a3f9c1' },
			dri: p.dri,
			chatProjectUrl: p.chatProjectUrl,
			configChanged: this.configNotice(p)
		};
	}

	// prompt resolution (the resolver's output, DR-080 ff.)
	private overrides = new Map<string, T.RunOverride>();
	private promptSegments(p: MProject, st: MStep): T.PromptSegment[] {
		const src = PROMPTS[st.tpl.key] ?? DEFAULT_PROMPT(st.tpl.title);
		const out: T.PromptSegment[] = [];
		const re =
			/\{\{([A-Z_]+)\|([^|}]*)\|([^}]*)\}\}|\[\[input:([a-z]+)\]\]|<<part:([a-z-]+)\|([\s\S]*?)>>/g;
		let last = 0;
		for (const m of src.matchAll(re)) {
			if (m.index > last) out.push({ kind: 'text', spans: visibleSpans(src.slice(last, m.index)) });
			if (m[1]) {
				const value = m[1] === 'APP_NAME' ? p.name : (p.variables[m[1]] ?? m[2] ?? '');
				out.push({
					kind: 'resolved',
					token: `[${m[1]}]`,
					class: 'projectVariable',
					text: value,
					note: m[3] ?? null
				});
			} else if (m[4]) {
				const inp = st.inputs.find((i) => i.name === m[4]);
				if (inp?.value)
					out.push({
						kind: 'resolved',
						token: inp.token,
						class: 'runInput',
						text: inp.value,
						note: 'Run input'
					});
				else
					out.push({
						kind: 'unresolved',
						token: inp ? inp.token : `[${m[4]}]`,
						class: 'runInput',
						input: m[4]
					});
			} else if (m[5]) {
				const part = st.parts.find((x) => x.partId === m[5]);
				if (part?.included === false)
					out.push({ kind: 'removedPart', expr: m[5], reason: 'Optional part turned off' });
				else out.push({ kind: 'optionalPart', partId: m[5], included: true, text: m[6] ?? '' });
			}
			last = m.index + m[0].length;
		}
		if (last < src.length) out.push({ kind: 'text', spans: visibleSpans(src.slice(last)) });
		return out;
	}
	private promptText(p: MProject, st: MStep): string {
		return this.promptSegments(p, st)
			.map((s) => {
				switch (s.kind) {
					case 'text':
						return s.spans.map((x) => ('text' in x ? x.text : '')).join('');
					case 'resolved':
					case 'optionalPart':
						return s.text;
					case 'unresolved':
						return s.token;
					case 'removedPart':
						return '';
				}
			})
			.join('');
	}
	private start(p: MProject, st: MStep): T.Explained<T.Start> | null {
		const t = st.tpl;
		if (!t.model || !t.effort) return null;
		const line = this.specLine(p, st);
		const model = MODEL_ENUM[line?.model?.value ?? t.model] ?? 'opus';
		const effort = (EFFORTS as readonly string[]).includes(line?.effort?.value ?? '')
			? (line?.effort?.value as T.Effort)
			: 'high';
		const mode = t.surface === 'chat' ? null : (MODE_ENUM[t.mode ?? 'Manual'] ?? 'default');
		const value: T.Start = { model, effort, mode };
		if (line?.model?.origin.kind === 'substitution')
			return {
				value,
				origin: line.model.origin,
				from: {
					model: MODEL_ENUM[t.model] ?? 'fable',
					effort: (t.effort as T.Effort | undefined) ?? 'high',
					mode
				}
			};
		return { value, origin: { kind: 'template' }, from: null };
	}
	private tabName(p: MProject, st: MStep): string {
		const stage = this.stageOf(p, st.tpl.key).stage;
		return `fm-${p.slug}-s${String(stage).padStart(2, '0')}-${st.tpl.key.replace(/[.:]/g, '-').toLowerCase()}`;
	}
	private chainRun(chain: string | null): World['runs'][number] | undefined {
		if (!chain) return undefined;
		return [...this.world.runs].reverse().find((r) => r.summary.chain === chain);
	}
	private continues(p: MProject, st: MStep): MStep | undefined {
		const m = /continues (\S+)/.exec(st.tpl.where);
		return m?.[1] ? stepOf(p, m[1]) : undefined;
	}
	private argv(p: MProject, st: MStep): string[] | null {
		const s = this.start(p, st);
		if (st.tpl.surface !== 'cli' || !s) return null;
		const parent = this.continues(p, st);
		const chain = parent?.chain ?? null;
		const claude = this.world.settings.tools.claude.path ?? 'claude';
		const head = chain ? [claude, '--resume', chain] : [claude, '--session-id', '<new uuid>'];
		const agents = st.tpl.agents.length ? ['--agents', `.foreman/agents/${st.tpl.key}.json`] : [];
		return [
			...head,
			'--model',
			s.value.model,
			'--effort',
			s.value.effort,
			'--permission-mode',
			s.value.mode ?? 'default',
			...agents,
			'--name',
			this.tabName(p, st),
			'--settings',
			'~/Library/Application Support/dev.foreman/runs/r_0193/settings.json',
			this.promptText(p, st)
		];
	}
	private blockers(p: MProject, st: MStep, launching: boolean): T.Blocker[] {
		const out: T.Blocker[] = [];
		if (p.readOnly) out.push({ kind: 'readOnly', ...p.readOnly });
		const nr = (reason: T.NotRunnableReason, text: string) =>
			out.push({ kind: 'notRunnable', reason, text });
		switch (st.state) {
			case 'notApplicable':
				nr('notApplicable', st.why ?? 'Not applicable');
				break;
			case 'undecided':
				nr('undecided', st.why ?? 'A condition is undecided');
				break;
			case 'needsAnnotation':
				nr('needsAnnotation', 'Not runnable: needs annotation');
				break;
			case 'needsNewerForeman':
				nr('needsNewerForeman', 'Needs a newer Foreman');
				break;
			case 'done':
				if (st.tpl.kind !== 'repeatable') nr('done', 'Done. This step runs once.');
				break;
			case 'running':
				nr('running', `Running in ${this.tabName(p, st)}`);
				break;
			default:
				break;
		}
		const stage = this.stageOf(p, st.tpl.key);
		if (stage.status === 'locked') out.push({ kind: 'packetUnsigned', stage: stage.stage - 1 });
		const parent = this.continues(p, st);
		const live = this.chainRun(parent?.chain ?? null);
		if (st.why?.startsWith('Waits for') && live?.live && live.summary.status === 'running')
			nr('running', st.why);
		if (
			st.tpl.surface === 'cli' &&
			this.world.floorAll &&
			this.world.claudeVersion < this.world.floorAll
		)
			out.push({
				kind: 'versionBelowFloor',
				found: this.world.claudeVersion,
				required: this.world.floorAll,
				requirements: [{ what: 'Opus 5.5', version: '2.1.280' }]
			});
		const inSpec = st.tpl.where.includes('spec/') || st.tpl.where.includes('continues 2.');
		if (launching && st.tpl.surface === 'cli' && inSpec) {
			const n = this.configNotice(p);
			if (n?.blocking)
				out.push({
					kind: 'authKeyBlocked',
					file: 'spec/.claude/settings.json',
					key: 'apiKeyHelper',
					hit: 'd0000000-0000-4000-8000-000000000001',
					allowlistable: false
				});
			else if (n)
				out.push({
					kind: 'configChanged',
					cwd: 'spec',
					fingerprint: n.fingerprint,
					diffs: n.diffs
				});
		}
		for (const i of st.inputs)
			if (i.required && !i.value) out.push({ kind: 'runInputMissing', name: i.label });
		return out;
	}
	private viewOfStep(p: MProject, st: MStep): T.StepView {
		const isChat = st.tpl.surface === 'chat';
		const chips: T.ChatChips | null = isChat
			? {
					chips: [
						...(/same chat as (\S+)/.exec(st.tpl.where)
							? [
									{
										id: 'same-chat',
										label: `Same chat as ${/same chat as (\S+)/.exec(st.tpl.where)?.[1] ?? ''}`,
										strong: true
									}
								]
							: [{ id: 'new-chat', label: 'New conversation in the Project', strong: true }]),
						{ id: 'model', label: st.tpl.model ?? 'Opus 5.5', strong: false },
						{ id: 'effort', label: 'Adaptive effort', strong: false },
						...(st.tpl.note === 'Research off'
							? [{ id: 'research', label: 'Research off', strong: false }]
							: [])
					],
					link: st.tpl.key.startsWith('1.')
						? 'https://claude.ai/chat/7c1e9a4b-2d3f-4e5a-8b6c-7d8e9f0a1f04'
						: null,
					linkLabel: st.tpl.key.startsWith('1.') ? 'claude.ai/chat/7c1e…f04' : null
				}
			: null;
		return {
			summary: this.stepSummary(p, st),
			prompt:
				st.state === 'needsAnnotation' || st.tpl.kind === 'action'
					? []
					: this.promptSegments(p, st),
			inputs: st.inputs,
			start: this.start(p, st),
			switches:
				st.tpl.key === '2.4'
					? [{ text: 'Re-arms the council session with Accept edits when Claude is idle.' }]
					: [],
			chips,
			argvPreview: this.argv(p, st),
			envNames:
				st.tpl.surface === 'cli'
					? ['HOME', 'LANG', 'PATH', 'SHELL', 'TERM', 'USER', ...this.world.settings.envPassthrough]
					: [],
			blockers: this.blockers(p, st, false),
			warnings: st.inputs.filter((i) => i.warning).map((i) => i.warning ?? ''),
			remaps: st.tpl.key === '2.4' ? [] : [],
			outs:
				st.tpl.key === '2.4'
					? [{ path: 'spec/council/r7-stack.md', exists: false }]
					: st.tpl.key === '2.3'
						? [{ path: 'spec/council/r6-feature-specs.md', exists: true }]
						: [],
			override: this.overrides.get(`${p.id}/${st.tpl.key}`) ?? null,
			parts: st.parts,
			copiedAt: st.copiedAt
		};
	}

	// ---------------------------------------------------------------- 4.1 App
	appBootstrap(req: Requests['app_bootstrap']): Promise<T.AppBootstrap> {
		return this.run('app_bootstrap', req, () => ({
			firstRun: this.world.firstRun,
			settingsRev: this.world.settings.rev,
			projects: this.world.projects.map((p) => this.summary(p)),
			doctor: this.world.doctor.level
		}));
	}
	settingsGet(req: Requests['settings_get']): Promise<T.SettingsView> {
		return this.run('settings_get', req, () => this.settingsView());
	}
	private settingsView(): T.SettingsView {
		const s = this.world.settings;
		return s.blueprintRepo
			? {
					...s,
					blueprintRepo: {
						...s.blueprintRepo,
						variables: VARIABLES,
						conditions: CONDITIONS.map((c) => ({
							id: c.id,
							label: c.label,
							effect: c.effect,
							late: c.late
						}))
					}
				}
			: s;
	}
	settingsUpdate(req: Requests['settings_update']): Promise<T.SettingsView> {
		return this.run('settings_update', req, () => {
			const s = this.world.settings;
			if (req.expectRev !== s.rev)
				throw fail('stale', { kind: 'stale', what: 'readStamp', fresh: this.settingsView() });
			const pt = req.patch;
			if (pt.envPassthrough !== null || pt.resetTool !== null || pt.removeAllowlistEntry !== null)
				this.ask();
			if (pt.driName !== null) s.driName = pt.driName.trim() || null;
			if (pt.editor) s.editorBundleId = pt.editor.kind === 'set' ? pt.editor.bundleId : null;
			if (pt.theme) s.theme = pt.theme;
			if (pt.terminal) {
				if (pt.terminal.fontSize < 9 || pt.terminal.fontSize > 32)
					throw invalidField('patch.terminal.fontSize', 'range');
				if (pt.terminal.scrollback < 1000 || pt.terminal.scrollback > 100000)
					throw invalidField('patch.terminal.scrollback', 'range');
				s.terminal = pt.terminal;
			}
			if (pt.envPassthrough) s.envPassthrough = pt.envPassthrough;
			if (pt.globalVariables) s.globalVariables = pt.globalVariables;
			if (pt.clearLoginPath) s.loginPath = null;
			if (pt.removeAllowlistEntry)
				s.authKeyAllowlist = s.authKeyAllowlist.filter((e) => e.sha256 !== pt.removeAllowlistEntry);
			if (pt.screenReaderMode !== null) s.screenReaderMode = pt.screenReaderMode;
			if (pt.resetTool)
				s.tools[pt.resetTool] = { ...s.tools[pt.resetTool], autoDetected: true, problem: null };
			const onlyOs = Object.entries(pt).every(([k, v]) => k === 'osDisplay' || v === null);
			if (!onlyOs) {
				s.rev += 1;
				this.emit('settings:changed', { rev: s.rev });
			}
			return this.settingsView();
		});
	}
	settingsPickTool(req: Requests['settings_pick_tool']): Promise<T.ToolPaths> {
		return this.run('settings_pick_tool', req, () => {
			this.ask();
			const s = this.world.settings;
			s.tools[req.tool] = {
				path: req.tool === 'claude' ? '/opt/homebrew/bin/claude' : '/usr/bin/git',
				version: req.tool === 'claude' ? this.world.claudeVersion : '2.51.0',
				autoDetected: false,
				problem: null
			};
			s.rev += 1;
			this.emit('settings:changed', { rev: s.rev });
			return s.tools;
		});
	}
	private nextOf(p: MProject): MStep | null {
		const stage = p.stages[p.current];
		if (!stage || stage.status === 'signed' || stage.status === 'waiting') return null;
		return (
			stage.steps.find((s) => s.next) ??
			stage.steps.find((s) => ['notStarted', 'copied', 'undecided'].includes(s.state)) ??
			null
		);
	}
	private todayRow(p: MProject): T.NextStepRow & { runnable: boolean } {
		const strip = p.stages.map((s) => s.status);
		const stage = p.stages[p.current];
		const base = { project: this.summary(p), strip, stageTitle: STAGE_TITLES[p.current] ?? null };
		if (p.readOnly?.reason === 'folderMissing')
			return { ...base, next: null, whyNone: 'Project folder not found', runnable: false };
		if (p.readOnly)
			return {
				...base,
				next: null,
				whyNone: `Open read-only: ${p.readOnly.detail}`,
				runnable: false
			};
		if (stage?.status === 'waiting')
			return {
				...base,
				next: null,
				whyNone: 'Gate A passed, Gate B wrote the packet, Gate C found lessons.',
				runnable: false
			};
		const st = this.nextOf(p);
		if (!st)
			return {
				...base,
				next: null,
				whyNone: 'Every step in this stage is done. Close out the stage next.',
				runnable: false
			};
		const runnable = this.blockers(p, st, false).length === 0;
		return { ...base, next: this.stepSummary(p, st), whyNone: null, runnable };
	}
	todayGet(req: Requests['today_get']): Promise<T.TodayView> {
		return this.run('today_get', req, () => {
			const rows = this.world.projects.map((p) => this.todayRow(p));
			rows.sort((a, b) => Number(b.runnable) - Number(a.runnable));
			return {
				rows: rows.map((row) => ({
					project: row.project,
					next: row.next,
					whyNone: row.whyNone,
					strip: row.strip,
					stageTitle: row.stageTitle
				})),
				inbox: this.world.inbox
			};
		});
	}
	doNext(req: Requests['do_next']): Promise<T.DoNextOutcome> {
		return this.run('do_next', req, () => {
			const rows = this.world.projects.map((p) => ({ p, row: this.todayRow(p) }));
			const top = rows.find((r) => r.row.runnable) ?? rows[0];
			if (!top)
				return { kind: 'nothing', why: 'No projects yet. Start your first project with ⌘N.' };
			const st = this.nextOf(top.p);
			if (!st) return { kind: 'nothing', why: top.row.whyNone ?? 'Nothing to run.' };
			if (st.tpl.surface === 'chat') {
				const blockers = this.blockers(top.p, st, true);
				return {
					kind: 'chat',
					project: top.p.id,
					step: st.tpl.key,
					ready: blockers.length === 0,
					blockers
				};
			}
			return {
				kind: 'cli',
				project: top.p.id,
				step: st.tpl.key,
				prepare: this.prepare(top.p, st, 'doNext', 'normal')
			};
		});
	}

	// ---------------------------------------------------------------- 4.2 First run and Doctor
	blueprintPickRepo(req: Requests['blueprint_pick_repo']): Promise<T.BlueprintRepoInfo> {
		return this.run('blueprint_pick_repo', req, () => {
			if (req.mode === 'choose') this.ask();
			const info: T.BlueprintRepoInfo = {
				displayPath: req.mode === 'choose' ? '~/Developer/claude-blueprint' : '~/claude-blueprint',
				commit: 'a3f9c1',
				version: '2.2',
				grammarVersion: 1,
				dirty: false,
				health: { runnable: '0 to 5', needsAnnotation: '6 to 18', errors: 0, warnings: 2 },
				variables: VARIABLES,
				conditions: CONDITIONS.map((c) => ({
					id: c.id,
					label: c.label,
					effect: c.effect,
					late: c.late
				}))
			};
			this.world.settings.blueprintRepo = info;
			this.world.settings.driName ??= 'Billy Ribeiro';
			this.world.settings.rev += 1;
			this.emit('settings:changed', { rev: this.world.settings.rev });
			return info;
		});
	}
	templateHealth(req: Requests['template_health']): Promise<T.TemplateHealth> {
		return this.run('template_health', req, () => {
			if (req.projectId) this.project(req.projectId);
			const d = (
				stage: number,
				severity: 'error' | 'warning',
				line: number,
				column: number,
				message: T.TextSpan[]
			): T.Diagnostic => ({
				severity,
				rule: severity === 'error' ? 'unknown-key' : 'prose-mismatch',
				file: 'claude-build-blueprint.md',
				line,
				column,
				message,
				stage
			});
			const c = (text: string): T.TextSpan => ({ kind: 'code', text });
			const t = (text: string): T.TextSpan => ({ kind: 'plain', text });
			return {
				version: '2.2',
				grammarVersion: 1,
				commit: 'a3f9c1',
				file: '~/claude-blueprint/claude-build-blueprint.md',
				counts: { stages: 19, runnableSteps: 31, items: 27, errors: 1, warnings: 2 },
				stages: [
					{ stages: '0', title: 'Land Survey', status: 'runnable', text: 'Runnable', steps: '2' },
					{
						stages: '1',
						title: 'Meet the Architect',
						status: 'runnable',
						text: 'Runnable',
						steps: '5 + F.2'
					},
					{
						stages: '2',
						title: "Architect's Design Studio",
						status: 'runnable',
						text: 'Runnable',
						steps: '8 + F.2'
					},
					{
						stages: '3',
						title: 'Permits & Zoning',
						status: 'runnable',
						text: 'Runnable',
						steps: '2 + F.2'
					},
					{
						stages: '4',
						title: 'Construction Drawings',
						status: 'warning',
						text: 'Runnable, 1 warning',
						steps: '9 + F.2'
					},
					{
						stages: '5',
						title: 'Site Prep & Excavation',
						status: 'partial',
						text: 'Partly: 5.4 has an error',
						steps: '5 + gate'
					},
					{
						stages: '6–18',
						title: 'Foundation to Homeowner Maintenance',
						status: 'needsAnnotation',
						text: 'Needs annotation',
						steps: '—'
					}
				],
				diagnostics: [
					d(5, 'error', 1121, 6, [
						t('5.4: unknown key '),
						c('agent='),
						t(' in '),
						c('fm:step'),
						t('. Did you mean '),
						c('agents='),
						t("? 5.4 can't run until the blueprint is fixed.")
					]),
					d(5, 'warning', 1105, 1, [
						t('5.3: RUN WITH says "same session"; the annotation says '),
						c('session=new'),
						t('. The annotation wins; check which one you meant.')
					]),
					d(4, 'warning', 946, 1, [
						t('4.5: RUN WITH says effort "high"; the annotation says '),
						c('opus@xhigh'),
						t('. The annotation wins.')
					])
				]
			};
		});
	}
	doctorRun(req: Requests['doctor_run']): Promise<T.DoctorReport> {
		return this.run('doctor_run', req, () => {
			const d = this.world.doctor;
			this.emit('doctor:status', { level: d.level });
			return {
				level: d.level,
				checks:
					req.scope === 'p1'
						? d.checks.filter((c) => c.group === 'claude' || c.id === 'git')
						: d.checks,
				capabilities: d.capabilities,
				probes: d.probes,
				probeNote: 'Contract verified on 2.1.284. Probe folder trusted 12 Oct.',
				ranAt: NOW
			};
		});
	}
	doctorCaptureLoginPath(req: Requests['doctor_capture_login_path']): Promise<T.LoginPathPreview> {
		return this.run('doctor_capture_login_path', req, () => {
			this.ask();
			const entries: T.PathEntryCheck[] = [
				{ entry: '/opt/homebrew/bin', ok: true, why: null },
				{ entry: '/Users/billy/.cargo/bin', ok: true, why: null },
				{ entry: '/Users/billy/Library/pnpm', ok: true, why: null },
				{ entry: '.', ok: false, why: 'Relative entries are dropped.' },
				{ entry: '/usr/bin', ok: true, why: null }
			];
			return { previewId: this.preview('loginPath', '', entries), shell: '/bin/zsh', entries };
		});
	}
	doctorLoginPathApply(req: Requests['doctor_login_path_apply']): Promise<T.SettingsView> {
		return this.run('doctor_login_path_apply', req, () => {
			const pv = this.take(req.previewId, 'loginPath');
			this.ask();
			const entries = (pv.data as T.PathEntryCheck[]).filter((e) => e.ok).map((e) => e.entry);
			this.world.settings.loginPath = { capturedAt: NOW, entries };
			this.world.settings.rev += 1;
			const lp = this.world.doctor.checks.find((c) => c.id === 'loginPath');
			if (lp) {
				lp.level = 'ok';
				lp.detail = [{ kind: 'plain', text: `Captured: ${entries.join(':')}` }];
				lp.action = null;
			}
			this.emit('settings:changed', { rev: this.world.settings.rev });
			return this.settingsView();
		});
	}
	probeFolderOpen(req: Requests['probe_folder_open'], output?: T.PtyOutput): Promise<T.RunStarted> {
		return this.run('probe_folder_open', req, () => {
			const runId = this.uuid('a');
			const started: T.RunStarted = { runId, chain: this.uuid('9'), tabName: 'fm-probe' };
			this.addRun(started, null, 'Probe folder', 'Claude Code, no prompt', [
				'\x1b[1mDo you trust the files in this folder?\x1b[0m\r\n',
				'\x1b[2m~/Library/Application Support/dev.foreman/probe\x1b[0m\r\n',
				'\r\n',
				'❯ 1. Yes, proceed\r\n',
				'  2. No, exit\r\n'
			]);
			this.stream(runId, output);
			return started;
		});
	}
	probeRun(req: Requests['probe_run'], progress?: T.OnProgress): Promise<T.ContractResult> {
		return this.run('probe_run', req, () => {
			progress?.({ phase: 'version', found: this.world.claudeVersion });
			progress?.({ phase: 'contract', outcome: 'pass' });
			progress?.({ phase: 'done' });
			return {
				version: this.world.claudeVersion,
				verifiedAt: NOW,
				outcomes: this.world.doctor.probes
			};
		});
	}

	// ---------------------------------------------------------------- 4.3 Projects
	projectsList(req: Requests['projects_list']): Promise<T.ProjectSummary[]> {
		return this.run('projects_list', req, () => this.world.projects.map((p) => this.summary(p)));
	}
	setupPickFolder(req: Requests['setup_pick_folder']): Promise<T.FolderPick> {
		return this.run('setup_pick_folder', req, () => {
			this.ask();
			this.picks += 1;
			const first = this.picks === 1 && this.world.projects.some((p) => p.id === IDS.swings);
			const pickId = this.uuid('d');
			const path = first
				? '~/Code/tradelog'
				: this.picks <= 2
					? '~/Code/tradelog-app'
					: `~/Code/project-${this.picks}`;
			this.pickIds.set(pickId, path);
			return {
				pickId,
				displayPath: path,
				isGitRepo: true,
				hasForeman: null,
				refusal: first
					? '~/Code/tradelog is inside another Foreman project (~/Code/swings-platform). Choose a folder outside it, for example ~/Code/tradelog-app.'
					: null
			};
		});
	}
	setupPreview(req: Requests['setup_preview']): Promise<T.SetupPreview> {
		return this.run('setup_preview', req, () => {
			if (!this.pickIds.has(req.pickId)) throw fail('expired');
			if (!/^[a-z0-9-]{1,40}$/.test(req.slug)) throw invalidField('slug', 'pattern');
			for (const k of Object.keys(req.variables))
				if (!/^[A-Z][A-Z0-9_]*$/.test(k)) throw invalidField(`variables.${k}`, 'name');
			if (req.chatProjectUrl && !req.chatProjectUrl.startsWith('https://claude.ai/'))
				throw invalidField('chatProjectUrl', 'claudeUrl');
			const c = req.conditions;
			const rows: T.ApplicablePathRow[] = [
				c.idea_known === 'no'
					? {
							stages: '0',
							title: 'Land Survey',
							status: 'applies',
							why: "Applies: you don't know the idea yet (idea_known = no)"
						}
					: {
							stages: '0',
							title: 'Land Survey',
							status: 'notApplicable',
							why: 'Not applicable: you already know the idea (idea_known = yes)'
						},
				{
					stages: '1',
					title: 'Meet the Architect',
					status: 'applies',
					why: `Applies. 1.3 demand test is ${c.commercial === 'yes' ? 'included' : 'skipped'} (commercial = ${c.commercial ?? 'undecided'}); press release part included`
				},
				{
					stages: '2',
					title: "Architect's Design Studio",
					status: 'applies',
					why: `Applies. 2.1 teardown and 2 council roles ${c.commercial === 'yes' ? 'included' : 'skipped'} (commercial = ${c.commercial ?? 'undecided'})`
				},
				{
					stages: '3',
					title: 'Permits & Zoning',
					status: 'applies',
					why: `Applies. 3.1 pricing waits on payments; legal part ${c.others_data === 'yes' ? 'included' : 'skipped'} (others_data = ${c.others_data ?? 'undecided'})`
				},
				{
					stages: '4',
					title: 'Construction Drawings',
					status: 'applies',
					why: `Applies, including 4.5 design system (has_ui = ${c.has_ui ?? 'undecided'})`
				},
				{
					stages: '5',
					title: 'Site Prep & Excavation',
					status: 'applies',
					why: `Applies, including the staging pipeline (server = ${c.server ?? 'undecided'})`
				},
				...(c.payments === 'undecided' || !c.payments
					? [
							{
								stages: '10',
								title: 'Electrical',
								status: 'undecided' as const,
								why: 'Decide payments before signing Stage 2'
							}
						]
					: c.payments === 'no'
						? [
								{
									stages: '10',
									title: 'Electrical',
									status: 'notApplicable' as const,
									why: "Not applicable: it doesn't take payments (payments = no)"
								}
							]
						: []),
				{
					stages: '6–18',
					title: 'Foundation to Maintenance',
					status: 'needsAnnotation',
					why: 'Shown but not runnable until a later blueprint annotates them'
				}
			];
			const na = rows.filter((r) => r.status === 'notApplicable').length;
			const und = rows.filter((r) => r.status === 'undecided').map((r) => r.stages);
			const applies = 19 - na - und.length;
			const strip: T.ApplicablePathSummary['strip'] = Array.from({ length: 19 }, (_, n) => {
				if (n === 0)
					return {
						stage: 0,
						status: c.idea_known === 'no' ? 'ready' : 'notApplicable',
						why: c.idea_known === 'no' ? null : 'idea_known = yes'
					};
				if (n === 1)
					return { stage: 1, status: c.idea_known === 'no' ? 'locked' : 'ready', why: null };
				if (n === 10 && (c.payments === 'undecided' || !c.payments))
					return { stage: 10, status: 'undecided', why: 'payments = undecided' };
				if (n === 10 && c.payments === 'no')
					return { stage: 10, status: 'notApplicable', why: 'payments = no' };
				if (n > 5)
					return {
						stage: n,
						status: 'needsAnnotation',
						why: 'blueprint 2.2 annotates stages 0 to 5'
					};
				return { stage: n, status: 'locked', why: null };
			});
			const naList = rows
				.filter((r) => r.status !== 'applies' && r.status !== 'needsAnnotation')
				.map((r) => r.stages);
			const writes: T.PlannedWrite[] = [
				{ path: '.foreman/', action: 'create', reason: 'project, overlay, lock' },
				{ path: 'docs/handoff/', action: 'create', reason: null },
				{ path: 'docs/templates/', action: 'create', reason: null },
				{ path: 'docs/change-requests/', action: 'create', reason: null },
				{ path: 'docs/postmortems/', action: 'create', reason: null },
				{ path: 'spec/', action: 'create', reason: null },
				{
					path: 'docs/lessons.md',
					action: 'leaveAlone',
					reason: 'Already exists; Foreman never overwrites it.'
				},
				{ path: '.git/', action: 'leaveAlone', reason: 'Already a repository on branch main.' },
				{
					path: 'docs/templates/handoff-packet.md',
					action: 'refuse',
					reason: 'Differs from the pinned kit. Move it aside to let Foreman write it.'
				}
			];
			const data = { req, path: this.pickIds.get(req.pickId) ?? '~/Code/project' };
			return {
				previewId: this.preview('setup', '', data),
				writes,
				gitInit: false,
				pin: { version: '2.2', commit: 'a3f9c1', clean: true },
				path: {
					applies,
					total: 19,
					summary: `${applies} of 19 stages apply; ${naList.join(' and ')} don't yet`,
					rows,
					strip
				}
			};
		});
	}
	setupApply(req: Requests['setup_apply']): Promise<T.ProjectSummary> {
		return this.run('setup_apply', req, () => {
			const pv = this.take(req.previewId, 'setup');
			const { req: sreq, path } = pv.data as { req: Requests['setup_preview']; path: string };
			const conditions = Object.fromEntries(
				CONDITIONS.map((c) => [c.id, sreq.conditions[c.id] ?? 'undecided'])
			) as Record<T.ConditionId, T.TriState>;
			const id = this.uuid('1');
			const tmp = buildWorld('happy').projects[0];
			if (!tmp) throw fail('internal');
			const p: MProject = {
				...tmp,
				id,
				name: sreq.name,
				slug: sreq.slug,
				displayPath: path,
				rev: 1,
				pinned: null,
				branch: 'main',
				chatProjectUrl: sreq.chatProjectUrl,
				conditions,
				lockedConditions: false,
				variables: { APP_NAME: sreq.name, ...sreq.variables },
				pending: [],
				configNotice: null,
				packets: {},
				gates: {},
				firstLaunchDone: false
			};
			p.stages = freshStages(p);
			p.current = conditions.idea_known === 'no' ? 0 : 1;
			this.world.projects.push(p);
			this.world.firstRun = false;
			this.emit('projects:changed', { rev: ++this.revCounter });
			return this.summary(p);
		});
	}
	projectOpen(req: Requests['project_open']): Promise<T.ProjectView> {
		return this.run('project_open', req, () => this.projectView(this.project(req.projectId)));
	}
	projectUrlSet(req: Requests['project_url_set']): Promise<number> {
		return this.run('project_url_set', req, () => {
			const p = this.project(req.projectId);
			this.writable(p);
			if (req.url !== null && !/^https:\/\/claude\.ai\/[^@:]*$/.test(req.url))
				throw invalidField('url', 'claudeUrl');
			p.chatProjectUrl = req.url;
			this.touch(p);
			return p.rev;
		});
	}
	conditionSet(req: Requests['condition_set']): Promise<T.ProjectView> {
		return this.run('condition_set', req, () => {
			const p = this.project(req.projectId);
			this.writable(p);
			const view = this.conditionViews(p)[req.condition];
			if (view?.locked) throw notAllowed(view.lockRule ?? 'condition-locked-after-stage-2');
			p.conditions[req.condition] = req.to;
			p.stages = rebuildStages(p);
			this.touch(p);
			return this.projectView(p);
		});
	}
	variableSet(req: Requests['variable_set']): Promise<T.ProjectView> {
		return this.run('variable_set', req, () => {
			const p = this.project(req.projectId);
			if (!(req.name in p.variables)) throw notFound();
			this.writable(p);
			p.variables[req.name] = req.value;
			this.touch(p);
			return this.projectView(p);
		});
	}
	blueprintRepin(req: Requests['blueprint_repin']): Promise<T.ProjectView> {
		return this.run('blueprint_repin', req, () => {
			const p = this.project(req.projectId);
			if (p.stages[1]?.status === 'signed') throw notAllowed('repin-after-stage-1-signed');
			p.readOnly = null;
			this.touch(p);
			return this.projectView(p);
		});
	}
	projectRemove(req: Requests['project_remove']): Promise<T.ProjectSummary[]> {
		return this.run('project_remove', req, () => {
			const p = this.project(req.projectId);
			if (this.world.runs.some((r) => r.live && r.summary.project === p.id))
				throw notAllowed('runs-live');
			this.ask();
			this.world.projects = this.world.projects.filter((x) => x.id !== p.id);
			this.emit('projects:changed', { rev: ++this.revCounter });
			return this.world.projects.map((x) => this.summary(x));
		});
	}
	projectPinSet(req: Requests['project_pin_set']): Promise<T.ProjectSummary[]> {
		return this.run('project_pin_set', req, () => {
			const p = this.project(req.projectId);
			if (req.slot !== null && (req.slot < 1 || req.slot > 9 || !Number.isInteger(req.slot)))
				throw invalidField('slot', 'range');
			for (const x of this.world.projects)
				if (req.slot !== null && x.pinned === req.slot) x.pinned = null;
			p.pinned = req.slot;
			this.emit('projects:changed', { rev: ++this.revCounter });
			return this.world.projects.map((x) => this.summary(x));
		});
	}

	// ---------------------------------------------------------------- 4.4 Steps
	stepView(req: Requests['step_view']): Promise<T.StepView> {
		return this.run('step_view', req, () => {
			const p = this.project(req.projectId);
			return this.stepView_(p, req.step);
		});
	}
	private stepView_(p: MProject, key: string): T.StepView {
		return this.viewOfStep(p, this.step(p, key));
	}
	runInputSet(req: Requests['run_input_set']): Promise<T.StepView> {
		return this.run('run_input_set', req, () => {
			const p = this.project(req.projectId);
			if (p.readOnly) throw fail('readOnly', { kind: 'readOnly', ...p.readOnly });
			const st = this.step(p, req.step);
			const inp = st.inputs.find((i) => i.name === req.name);
			if (!inp) throw notFound();
			if (enc.encode(req.value).length > 1024 * 1024) throw invalidField('value', 'tooLong');
			inp.value = req.value;
			inp.warning =
				enc.encode(req.value).length > 64 * 1024
					? 'Chat paste limits may apply: this input is over 64 KB.'
					: null;
			this.touch(p);
			return this.viewOfStep(p, st);
		});
	}
	optionalPartSet(req: Requests['optional_part_set']): Promise<T.StepView> {
		return this.run('optional_part_set', req, () => {
			const p = this.project(req.projectId);
			const st = this.step(p, req.step);
			const part = st.parts.find((x) => x.partId === req.partId);
			if (!part) throw notFound();
			if (part.locked) throw notAllowed('part-recorded');
			part.included = req.include;
			this.touch(p);
			return this.viewOfStep(p, st);
		});
	}
	runOverrideSet(req: Requests['run_override_set']): Promise<T.StepView> {
		return this.run('run_override_set', req, () => {
			const p = this.project(req.projectId);
			const st = this.step(p, req.step);
			if (req.over?.mode && st.tpl.surface === 'chat') throw notAllowed('mode-on-chat-step');
			if (req.over?.effort === 'adaptive' && st.tpl.surface !== 'chat')
				throw notAllowed('adaptive-only-chat');
			const k = `${p.id}/${st.tpl.key}`;
			if (req.over) this.overrides.set(k, req.over);
			else this.overrides.delete(k);
			this.touch(p);
			return this.viewOfStep(p, st);
		});
	}
	stepMarkDone(req: Requests['step_mark_done']): Promise<T.StepView> {
		return this.run('step_mark_done', req, () => {
			const p = this.project(req.projectId);
			if (p.readOnly) throw fail('readOnly', { kind: 'readOnly', ...p.readOnly });
			const st = this.step(p, req.step);
			if (st.tpl.surface === 'cli' && st.tpl.kind !== 'manual')
				throw notAllowed('cli-finishes-by-exit');
			if (st.tpl.kind === 'wait' && st.state === 'waitingUntil')
				throw notAllowed('wait-before-date');
			if (['notApplicable', 'undecided', 'needsAnnotation'].includes(st.state))
				throw notAllowed('not-runnable');
			this.markDone(p, st);
			return this.viewOfStep(p, st);
		});
	}
	private markDone(p: MProject, st: MStep, text = 'Done'): void {
		st.state = 'done';
		st.stateText = text;
		st.next = false;
		st.why = null;
		const stage = this.stageOf(p, st.tpl.key);
		const idx = stage.steps.indexOf(st);
		const nxt = stage.steps.slice(idx + 1).find((x) => x.state === 'notStarted');
		if (nxt) nxt.next = true;
		this.touch(p);
	}
	stepSkip(req: Requests['step_skip']): Promise<T.ProjectView> {
		return this.run('step_skip', req, () => {
			const p = this.project(req.projectId);
			const st = this.step(p, req.step);
			if (st.tpl.tag === 'core') throw notAllowed('core-step-skip');
			if (st.tpl.tag === 'conditional') throw notAllowed('conditional-step-skip');
			if (!req.reason.trim()) throw notAllowed('reason-required');
			this.writable(p);
			st.state = 'skipped';
			st.stateText = `Skipped: ${visibleText(req.reason, false)}`;
			st.next = false;
			this.touch(p);
			return this.projectView(p);
		});
	}
	actionPreview(req: Requests['action_preview']): Promise<T.ActionPreview> {
		return this.run('action_preview', req, () => {
			const p = this.project(req.projectId);
			if (p.readOnly) throw fail('readOnly', { kind: 'readOnly', ...p.readOnly });
			const st = this.step(p, req.step);
			if (st.tpl.kind !== 'action') throw notAllowed('not-an-action');
			const writes: T.Diff[] = [
				'principal-engineer',
				'security-architect',
				'user-advocate',
				'product-strategist'
			].map((a) => ({
				path: `spec/.claude/agents/${a}.md`,
				exists: false,
				hunks: [
					{
						oldStart: 0,
						newStart: 1,
						lines: [
							{ op: 'add', spans: visibleSpans('---') },
							{ op: 'add', spans: visibleSpans(`name: ${a}`) },
							{ op: 'add', spans: visibleSpans('---') }
						]
					}
				]
			}));
			return { previewId: this.preview('action', p.id, st.tpl.key), action: 'writeBlocks', writes };
		});
	}
	actionApply(req: Requests['action_apply']): Promise<T.StepView> {
		return this.run('action_apply', req, () => {
			const pv = this.take(req.previewId, 'action');
			const p = this.project(pv.project);
			this.writable(p);
			const st = this.step(p, pv.data as string);
			this.markDone(p, st, 'Done, wrote 4 agents');
			return this.viewOfStep(p, st);
		});
	}
	waitStart(req: Requests['wait_start']): Promise<T.StepView> {
		return this.run('wait_start', req, () => {
			const p = this.project(req.projectId);
			const st = this.step(p, req.step);
			if (st.tpl.kind !== 'wait') throw notAllowed('not-a-wait-step');
			if (req.until <= NOW.slice(0, 10)) throw notAllowed('date-not-after-today');
			st.state = 'waitingUntil';
			st.stateText = `Waiting until ${req.until}`;
			this.touch(p);
			return this.viewOfStep(p, st);
		});
	}
	clipboardCopy(req: Requests['clipboard_copy']): Promise<T.CopyResult> {
		return this.run('clipboard_copy', req, () => {
			const p = this.project(req.projectId);
			const need = (v: unknown, f: string) => {
				if (v === null) throw invalidField(f, 'required');
			};
			let text = '';
			let focusRun: string | null = null;
			switch (req.target) {
				case 'prompt':
				case 'command': {
					need(req.step, 'step');
					const st = this.step(p, req.step ?? '');
					if (this.promptSegments(p, st).some((s) => s.kind === 'unresolved'))
						throw notAllowed('unresolved-tokens');
					text =
						req.target === 'prompt' ? this.promptText(p, st) : (this.argv(p, st) ?? []).join(' ');
					if (st.tpl.kind === 'followup' && st.tpl.surface === 'chat') {
						st.state = 'copied';
						st.stateText = 'Copied just now, waiting for the reply';
						this.touch(p);
					}
					const live = this.chainRun(this.continues(p, st)?.chain ?? null);
					if (live?.live) focusRun = live.summary.runId;
					break;
				}
				case 'sessionId':
					need(req.chain, 'chain');
					text = req.chain ?? '';
					break;
				case 'fileContents':
					need(req.path, 'path');
					text = `contents of ${req.path ?? ''}`;
					break;
				case 'fixRequest':
					need(req.stage, 'stage');
					text = `Fix only these problems in docs/handoff/stage-${String(req.stage).padStart(2, '0')}.md: … Do not change anything else.`;
					break;
			}
			const clean = stripControls(text);
			this.world.clipboard = clean.text;
			return { removedControls: clean.removed, focusRun };
		});
	}

	// ---------------------------------------------------------------- 4.5 Chat
	chatCopyAndOpen(req: Requests['chat_copy_and_open']): Promise<T.ChatCopyResult> {
		return this.run('chat_copy_and_open', req, () => {
			const p = this.project(req.projectId);
			if (p.readOnly) throw fail('readOnly', { kind: 'readOnly', ...p.readOnly });
			const st = this.step(p, req.step);
			if (st.tpl.surface !== 'chat') throw notAllowed('not-a-chat-step');
			if (this.promptSegments(p, st).some((s) => s.kind === 'unresolved'))
				throw notAllowed('unresolved-tokens');
			const b = this.blockers(p, st, true).filter(
				(x) => x.kind !== 'notRunnable' || x.reason !== 'done'
			);
			if (b.length) throw notAllowed('not-runnable');
			this.world.clipboard = this.promptText(p, st);
			st.state = 'copied';
			st.stateText = 'Copied just now, waiting for the packet';
			st.copiedAt = NOW;
			const runId = this.uuid('a');
			this.touch(p);
			return {
				runId,
				opened: true,
				missingLink: p.chatProjectUrl ? null : 'project',
				removedControls: 0
			};
		});
	}
	chatLinkSet(req: Requests['chat_link_set']): Promise<number> {
		return this.run('chat_link_set', req, () => {
			const p = this.project(req.projectId);
			const url = req.source.kind === 'text' ? req.source.url : this.world.clipboard.trim();
			if (req.source.kind === 'clipboard' && !url)
				throw fail('clipboard', { kind: 'clipboard', why: 'empty' });
			if (!/^https:\/\/claude\.ai\/[^@\s]*$/.test(url)) throw invalidField('url', 'claudeUrl');
			this.writable(p);
			this.touch(p);
			return p.rev;
		});
	}
	pasteBackPreview(req: Requests['paste_back_preview']): Promise<T.PastePreview> {
		return this.run('paste_back_preview', req, () => {
			const p = this.project(req.projectId);
			if (p.readOnly) throw fail('readOnly', { kind: 'readOnly', ...p.readOnly });
			const st = this.step(p, req.step);
			if (req.source === 'sensor')
				throw fail('unsupported', {
					kind: 'unsupported',
					feature: 'sensor',
					why: 'No session sensor in this build'
				});
			const text = this.world.clipboard;
			if (!text.trim()) throw fail('clipboard', { kind: 'clipboard', why: 'empty' });
			if (enc.encode(text).length > 1024 * 1024)
				throw fail('clipboard', { kind: 'clipboard', why: 'tooLarge' });
			const stage = this.stageOf(p, st.tpl.key).stage;
			const packets = [...text.matchAll(/=== HANDOFF PACKET · Stage (\d+)/g)];
			const writesPacket = st.tpl.note?.includes('packet') ?? false;
			const add = (path: string, lines: string[], exists = false): T.Diff => ({
				path,
				exists,
				hunks: [
					{
						oldStart: 0,
						newStart: 1,
						lines: lines.map((l) => ({ op: 'add' as const, spans: visibleSpans(l) }))
					}
				]
			});
			if (writesPacket || req.kind === 'packet') {
				if (packets.length === 0) throw notAllowed('no-packet');
				if (packets.length > 1) throw notAllowed('two-packets');
				const n = Number(packets[0]?.[1]);
				if (n !== stage) throw notAllowed('wrong-stage-packet');
				const nn = String(stage).padStart(2, '0');
				const split = text.split(/\n(?=PRESS RELEASE|FAQ)/);
				const targets = [add(`docs/handoff/stage-${nn}.md`, (split[0] ?? text).split('\n'))];
				if (split.length > 1)
					targets.push(
						add(`docs/handoff/stage-${nn}-attachments.md`, split.slice(1).join('\n').split('\n'))
					);
				if (stage === 1) targets.push(add('spec/01-app-brief.md', ['# App Brief', '', p.name]));
				return {
					previewId: this.preview('paste', p.id, {
						step: st.tpl.key,
						targets: targets.map((t) => t.path),
						packet: true
					}),
					kind: 'packet',
					targets,
					problems: [],
					notes: ['APPROVED BY <blank> becomes unsigned', '3 curly quotes straightened'],
					packetFields: { fromStage: stage, toStage: stage + 1 }
				};
			}
			const out =
				st.tpl.key === '4.5'
					? 'docs/design/design-system.md'
					: `spec/${st.tpl.key.replace(/[.:]/g, '-')}-output.md`;
			const targets = [add(out, text.split('\n').slice(0, 40))];
			return {
				previewId: this.preview('paste', p.id, { step: st.tpl.key, targets: [out], packet: false }),
				kind: 'output',
				targets,
				problems: [],
				notes: [],
				packetFields: null
			};
		});
	}
	pasteBackApply(req: Requests['paste_back_apply']): Promise<T.PasteApplyResult> {
		return this.run('paste_back_apply', req, () => {
			const pv = this.take(req.previewId, 'paste');
			const p = this.project(pv.project);
			if (this.world.scenario === 'conflict') {
				const d = pv.data as { targets: string[] };
				throw fail('conflict', {
					kind: 'conflict',
					path: d.targets[1] ?? d.targets[0] ?? '',
					written: d.targets.slice(0, 1)
				});
			}
			const d = pv.data as { step: string; targets: string[]; packet: boolean };
			const st = this.step(p, d.step);
			this.markDone(p, st, d.packet ? 'Done, packet written' : 'Done, output written');
			if (d.packet) {
				const stage = this.stageOf(p, st.tpl.key);
				stage.packetExists = true;
				stage.status = 'waiting';
				stage.statusWhy = 'packet waiting for your signature';
				this.emit('packet:changed', { projectId: p.id, stage: stage.stage, rev: p.rev });
			}
			return { written: d.targets, stepDone: true };
		});
	}

	// ---------------------------------------------------------------- 4.6 Launch
	private prepare(
		p: MProject,
		st: MStep,
		entry: T.EntryPoint,
		mode: T.PrepareMode
	): T.PrepareOutcome {
		const parent = this.continues(p, st);
		if (mode === 'freshWithContext' && !parent) throw notAllowed('fresh-not-continuing');
		const blockers = this.blockers(p, st, true);
		if (this.world.scenario === 'brokenChain' && parent && mode === 'normal')
			blockers.push({ kind: 'transcriptMissing', chain: parent.chain ?? IDS.chain22 });
		if (blockers.length) return { kind: 'blocked', blockers, warnings: [] };
		const confirm: T.ConfirmReason[] = [];
		if (st.tpl.tag === 'added') confirm.push({ kind: 'overlayStep' });
		if (!p.firstLaunchDone) confirm.push({ kind: 'firstLaunchInProject' });
		const over = this.overrides.get(`${p.id}/${st.tpl.key}`);
		const start = this.start(p, st);
		if (over && start && (over.model || over.mode))
			confirm.push({
				kind: 'modeOrModelChanged',
				from: start.value,
				to: {
					model: over.model ?? start.value.model,
					effort: over.effort ?? start.value.effort,
					mode: over.mode ?? start.value.mode
				}
			});
		if (start?.value.mode === 'auto' || start?.value.mode === 'dontAsk')
			confirm.push({ kind: 'autoOrDontAsk', mode: start.value.mode });
		const live = this.chainRun(parent?.chain ?? null);
		const rearm: T.RearmNeed | null =
			mode === 'freshWithContext' || !parent?.chain
				? null
				: live?.live
					? { kind: 'idleConfirmation', chain: parent.chain }
					: { kind: 'exited', chain: parent.chain };
		const planId = this.uuid('f');
		const s = start?.value;
		const modeWord = {
			plan: 'Plan',
			default: 'Manual',
			acceptEdits: 'Accept edits',
			auto: 'Auto',
			dontAsk: "Don't ask"
		}[s?.mode ?? 'default'];
		const model = Object.entries(MODEL_ENUM).find(([, v]) => v === s?.model)?.[0] ?? 'Opus 5.5';
		this.plans.set(planId, {
			project: p.id,
			step: st.tpl.key,
			chain: mode === 'freshWithContext' ? null : (parent?.chain ?? null),
			confirm,
			rearm,
			created: performance.now(),
			used: false,
			tab: this.tabName(p, st),
			title: this.promptText(p, st).split('\n')[0] ?? st.tpl.title,
			fresh: mode === 'freshWithContext',
			rearmText: rearm
				? `${st.tpl.number} ${st.tpl.title} · ${model} · ${s?.effort ?? 'high'} · ${modeWord} · permission grants reset`
				: null
		});
		return {
			kind: 'ready',
			planId,
			expiresInMs: 60_000,
			prompt: this.promptSegments(p, st),
			argv: this.argv(p, st) ?? [],
			envNames: [
				'HOME',
				'LANG',
				'PATH',
				'SHELL',
				'TERM',
				'USER',
				...this.world.settings.envPassthrough
			],
			runSettingsSha256: '5a'.repeat(32),
			confirm,
			rearm,
			warnings: [],
			remaps: []
		};
	}
	launchPrepare(
		req: Requests['launch_prepare'],
		progress?: T.OnProgress
	): Promise<T.PrepareOutcome> {
		return this.run('launch_prepare', req, () => {
			const p = this.project(req.projectId);
			const st = this.step(p, req.step);
			progress?.({ phase: 'version', found: this.world.claudeVersion });
			progress?.({ phase: 'fingerprint', firstScan: false });
			progress?.({ phase: 'auth', cached: true });
			progress?.({ phase: 'transcript' });
			progress?.({ phase: 'inputs' });
			const out = this.prepare(p, st, req.entry, req.mode);
			progress?.({ phase: 'done' });
			return out;
		});
	}
	chainResumePrepare(
		req: Requests['chain_resume_prepare'],
		progress?: T.OnProgress
	): Promise<T.PrepareOutcome> {
		return this.run('chain_resume_prepare', req, () => {
			const r = this.chainRun(req.chain);
			if (!r) throw notFound();
			if (r.live) throw notAllowed('chain-live');
			progress?.({ phase: 'version', found: this.world.claudeVersion });
			if (!r.summary.resumable)
				return {
					kind: 'blocked',
					blockers: [{ kind: 'transcriptMissing', chain: req.chain }],
					warnings: []
				};
			const planId = this.uuid('f');
			this.plans.set(planId, {
				project: r.summary.project,
				step: r.summary.step,
				chain: req.chain,
				confirm: [],
				rearm: null,
				created: performance.now(),
				used: false,
				tab: r.summary.tabName,
				title: `Resume ${r.summary.stepTitle}`,
				fresh: false,
				rearmText: null
			});
			progress?.({ phase: 'done' });
			return {
				kind: 'ready',
				planId,
				expiresInMs: 60_000,
				prompt: [],
				argv: [
					this.world.settings.tools.claude.path ?? 'claude',
					'--resume',
					req.chain,
					'--name',
					r.summary.tabName
				],
				envNames: ['HOME', 'LANG', 'PATH', 'SHELL', 'TERM', 'USER'],
				runSettingsSha256: '5a'.repeat(32),
				confirm: [],
				rearm: null,
				warnings: [],
				remaps: []
			};
		});
	}
	private takePlan(id: string): Plan {
		const plan = this.plans.get(id);
		if (!plan) throw notFound();
		if (plan.used || performance.now() - plan.created > 60_000) throw fail('expired');
		plan.used = true;
		return plan;
	}
	private addRun(
		started: T.RunStarted,
		p: MProject | null,
		stepTitle: string,
		ranWith: string,
		script: string[],
		step: string | null = null
	): void {
		this.world.runs.push({
			summary: {
				runId: started.runId,
				chain: started.chain,
				project: p?.id ?? '',
				projectName: p?.name ?? 'Foreman',
				step,
				stepTitle,
				tabName: started.tabName,
				ranWith,
				substituted: null,
				status: 'running',
				exitCode: null,
				error: null,
				startedAt: NOW,
				timeToStartMs: 4200,
				argv: [],
				resumable: true,
				whyNotResumable: null
			},
			script,
			live: true,
			written: 0
		});
		this.emit('run:status', { runId: started.runId, status: 'running', rev: ++this.revCounter });
	}
	private startFromPlan(plan: Plan, output: T.PtyOutput | undefined): T.RunStarted {
		const p = this.project(plan.project);
		const st = plan.step ? stepOf(p, plan.step) : undefined;
		const chain = plan.chain ?? this.uuid('9');
		const runId = this.uuid('a');
		const started: T.RunStarted = { runId, chain, tabName: plan.tab };
		const start = st ? this.start(p, st) : null;
		const ranWith = start
			? `${Object.entries(MODEL_ENUM).find(([, v]) => v === start.value.model)?.[0] ?? start.value.model}, ${start.value.effort}, ${
					{
						plan: 'Plan',
						default: 'Manual',
						acceptEdits: 'Accept edits',
						auto: 'Auto',
						dontAsk: "Don't ask"
					}[start.value.mode ?? 'default']
				}`
			: '';
		this.addRun(
			started,
			p,
			st?.tpl.title ?? plan.title,
			ranWith,
			launchTranscript(plan.tab, plan.title, plan.rearmText),
			plan.step
		);
		if (st) {
			st.state = 'running';
			st.stateText = `Running in ${plan.tab}`;
			st.run = runId;
			st.chain = chain;
			st.next = false;
		}
		p.firstLaunchDone = true;
		this.touch(p);
		this.stream(runId, output);
		return started;
	}
	launchStart(req: Requests['launch_start'], output?: T.PtyOutput): Promise<T.RunStarted> {
		return this.run('launch_start', req, () => {
			const plan = this.takePlan(req.planId);
			if (plan.rearm?.kind === 'idleConfirmation') throw notAllowed('needs-rearm');
			if (plan.confirm.length) this.ask();
			return this.startFromPlan(plan, output);
		});
	}
	configAck(req: Requests['config_ack']): Promise<null> {
		return this.run('config_ack', req, () => {
			const p = this.project(req.projectId);
			const n = this.configNotice(p);
			if (n?.fingerprint !== req.fingerprint)
				throw fail('stale', { kind: 'stale', what: 'fingerprint', fresh: n });
			this.ask();
			p.configAcked = true;
			this.touch(p);
			return null;
		});
	}
	authKeyAllowlistAdd(req: Requests['auth_key_allowlist_add']): Promise<T.SettingsView> {
		return this.run('auth_key_allowlist_add', req, () => {
			if (!req.hit.startsWith('d')) throw fail('expired');
			throw notAllowed('not-user-settings');
		});
	}
	projectEnvApprove(req: Requests['project_env_approve']): Promise<null> {
		return this.run('project_env_approve', req, () => {
			this.project(req.projectId);
			throw fail('unsupported', {
				kind: 'unsupported',
				feature: 'project-env-approval',
				why: 'S-7′ passed: Claude Code ignores project env'
			});
		});
	}

	// ---------------------------------------------------------------- 4.7 Terminal
	private stream(runId: string, output: T.PtyOutput | undefined): void {
		if (output) {
			let set = this.outputs.get(runId);
			if (!set) {
				set = new Set();
				this.outputs.set(runId, set);
			}
			set.add(output);
		}
		const r = this.world.runs.find((x) => x.summary.runId === runId);
		if (!r || !output) return;
		const chunks = r.script.slice(r.written);
		r.written = r.script.length;
		let i = 0;
		const tick = () => {
			const c = chunks[i++];
			if (c === undefined) return;
			this.deliver(runId, c);
			if (this.streamDelayMs > 0) setTimeout(tick, this.streamDelayMs);
			else queueMicrotask(tick);
		};
		queueMicrotask(tick);
	}
	private deliver(runId: string, text: string): void {
		if (this.paused.has(runId)) {
			setTimeout(() => {
				this.deliver(runId, text);
			}, 20);
			return;
		}
		const bytes = enc.encode(text);
		for (const cb of this.outputs.get(runId) ?? []) cb(bytes.slice().buffer);
	}
	private liveRun(runId: string): World['runs'][number] {
		const r = this.world.runs.find((x) => x.summary.runId === runId);
		if (!r) throw notFound();
		if (!r.live) throw notAllowed('pty-not-live');
		return r;
	}
	private endRun(r: World['runs'][number], code: number, status: T.RunStatus = 'exited'): void {
		r.live = false;
		r.summary.status = status;
		r.summary.exitCode = code;
		this.emit('run:status', { runId: r.summary.runId, status, rev: ++this.revCounter });
		const p = this.world.projects.find((x) => x.id === r.summary.project);
		const st = p && r.summary.step ? stepOf(p, r.summary.step) : undefined;
		if (p && st?.run === r.summary.runId) {
			st.run = null;
			if (code === 0) this.markDone(p, st, 'Done');
			else {
				st.state = 'notStarted';
				st.stateText = `Exited ${code}; run it again`;
				this.touch(p);
			}
			// The step that waits for this chain can now re-arm it.
			for (const s of p.stages.flatMap((x) => x.steps))
				if (s.why?.startsWith('Waits for')) s.why = null;
		}
	}
	ptyAttach(req: Requests['pty_attach'], output?: T.PtyOutput): Promise<T.AttachInfo> {
		return this.run('pty_attach', req, () => {
			const r = this.world.runs.find((x) => x.summary.runId === req.runId);
			if (!r) throw notFound();
			if (output) {
				let set = this.outputs.get(req.runId);
				if (!set) {
					set = new Set();
					this.outputs.set(req.runId, set);
				}
				set.add(output);
				const replay = r.script.slice(0, r.written);
				queueMicrotask(() => {
					for (const c of replay) output(enc.encode(c).slice().buffer);
					if (r.written < r.script.length) this.stream(req.runId, output);
				});
				if (r.written === 0) r.written = 0;
			}
			return {
				status: r.summary.status,
				replayedLines: r.script.length,
				tabName: r.summary.tabName
			};
		});
	}
	ptyWrite(req: Requests['pty_write']): Promise<null> {
		return this.run('pty_write', req, () => {
			if (enc.encode(req.data).length > 64 * 1024) throw invalidField('data', 'tooLong');
			const r = this.liveRun(req.runId);
			let buf = (this.lineBuf.get(req.runId) ?? '') + req.data;
			for (const ch of req.data) {
				if (ch === '\x03') {
					this.deliver(req.runId, '^C\r\n\x1b[2mInterrupted by user\x1b[0m\r\n\x1b[2m│\x1b[0m > ');
					buf = '';
				}
			}
			const nl = buf.search(/\r|\n/);
			if (nl >= 0) {
				const line = buf.slice(0, nl).trim();
				buf = '';
				this.deliver(req.runId, '\r\n');
				if (line === '/exit') {
					this.deliver(req.runId, '\x1b[2mGoodbye.\x1b[0m\r\n');
					this.endRun(r, 0);
				} else if (line)
					this.deliver(
						req.runId,
						`\x1b[32m⏺\x1b[0m ${visibleText(line, false)}\r\n\x1b[2m│\x1b[0m > `
					);
			} else {
				const echo = req.data.replaceAll('\x03', '');
				if (echo)
					this.deliver(req.runId, visibleText(echo, false).replace(/<DEL U\+007F>/g, '\b \b'));
			}
			this.lineBuf.set(req.runId, buf);
			return null;
		});
	}
	ptyPaste(req: Requests['pty_paste']): Promise<T.PasteText> {
		return this.run('pty_paste', req, () => {
			this.liveRun(req.runId);
			const text = this.world.clipboard;
			if (!text) throw fail('clipboard', { kind: 'clipboard', why: 'empty' });
			const clean = stripControls(text);
			return { text: clean.text, removedControls: clean.removed };
		});
	}
	ptyResize(req: Requests['pty_resize']): Promise<null> {
		return this.run('pty_resize', req, () => {
			if (req.cols < 2 || req.cols > 1000 || req.rows < 2 || req.rows > 500)
				throw invalidField('cols', 'range');
			this.liveRun(req.runId);
			return null;
		});
	}
	ptyFlow(req: Requests['pty_flow']): Promise<null> {
		return this.run('pty_flow', req, () => {
			if (!this.world.runs.some((x) => x.summary.runId === req.runId)) throw notFound();
			if (req.paused) this.paused.add(req.runId);
			else this.paused.delete(req.runId);
			return null;
		});
	}
	ptyInterrupt(req: Requests['pty_interrupt']): Promise<null> {
		return this.run('pty_interrupt', req, () => {
			this.liveRun(req.runId);
			this.deliver(req.runId, '^C\r\n\x1b[2mInterrupted by user\x1b[0m\r\n\x1b[2m│\x1b[0m > ');
			return null;
		});
	}
	ptyTerminate(req: Requests['pty_terminate']): Promise<null> {
		return this.run('pty_terminate', req, () => {
			const r = this.world.runs.find((x) => x.summary.runId === req.runId);
			if (!r) throw notFound();
			this.ask();
			if (r.live) {
				this.deliver(req.runId, '\r\n\x1b[2mTerminated (SIGTERM)\x1b[0m\r\n');
				this.endRun(r, 143);
			}
			return null;
		});
	}
	rearmStart(
		req: Requests['rearm_start'],
		output?: T.PtyOutput,
		progress?: T.OnProgress
	): Promise<T.RearmOutcome> {
		return this.run('rearm_start', req, () => {
			const plan = this.takePlan(req.planId);
			if (!plan.rearm) throw notAllowed('no-rearm-needed');
			this.ask();
			const old = this.chainRun(plan.chain);
			if (old?.live) {
				progress?.({ phase: 'ending', step: 'exit' });
				this.deliver(old.summary.runId, '/exit\r\n\x1b[2mGoodbye.\x1b[0m\r\n');
				this.endRun(old, 0);
				progress?.({ phase: 'exited', code: 0 });
			}
			progress?.({ phase: 'spawning' });
			const started = this.startFromPlan(plan, output);
			progress?.({ phase: 'done' });
			return { kind: 'relaunched', ...started };
		});
	}
	rearmContinue(
		req: Requests['rearm_continue'],
		output?: T.PtyOutput,
		progress?: T.OnProgress
	): Promise<T.RearmOutcome> {
		return this.run('rearm_continue', req, () => {
			const r = this.chainRun(req.chain);
			if (!r) throw fail('expired');
			if (r.live) throw notAllowed('process-still-alive');
			progress?.({ phase: 'spawning' });
			const runId = this.uuid('a');
			const started: T.RunStarted = { runId, chain: req.chain, tabName: r.summary.tabName };
			this.addRun(
				started,
				this.world.projects.find((p) => p.id === r.summary.project) ?? null,
				r.summary.stepTitle,
				r.summary.ranWith,
				launchTranscript(r.summary.tabName, r.summary.stepTitle, 'continued'),
				r.summary.step
			);
			this.stream(runId, output);
			progress?.({ phase: 'done' });
			return { kind: 'relaunched', ...started };
		});
	}
	terminalLinkOpen(req: Requests['terminal_link_open']): Promise<T.LinkOutcome> {
		return this.run('terminal_link_open', req, () => {
			if (!this.world.runs.some((x) => x.summary.runId === req.runId)) throw notFound();
			if (req.url.length > 8192) throw invalidField('url', 'tooLong');
			let u: URL;
			try {
				u = new URL(req.url);
			} catch {
				return 'refused';
			}
			if (u.protocol !== 'https:' || u.username || u.password) return 'refused';
			return this.dialog === 'cancel' ? 'cancelled' : 'opened';
		});
	}

	// ---------------------------------------------------------------- 4.8 Files
	fileOpen(req: Requests['file_open']): Promise<null> {
		return this.run('file_open', req, () => {
			this.project(req.projectId);
			if (req.path.startsWith('/') || req.path.includes('..'))
				throw fail('policyViolation', {
					kind: 'policyViolation',
					path: req.path,
					rule: 'outside-project'
				});
			if (!/\.(md|json|txt)$/.test(req.path))
				throw fail('policyViolation', {
					kind: 'policyViolation',
					path: req.path,
					rule: 'extension'
				});
			if (!this.world.settings.editorBundleId) throw notAllowed('no-editor-set');
			return null;
		});
	}
	fileReveal(req: Requests['file_reveal']): Promise<null> {
		return this.run('file_reveal', req, () => {
			this.project(req.projectId);
			if (req.path.startsWith('/') || req.path.includes('..'))
				throw fail('policyViolation', {
					kind: 'policyViolation',
					path: req.path,
					rule: 'outside-project'
				});
			return null;
		});
	}
	terminalOpenHere(req: Requests['terminal_open_here']): Promise<null> {
		return this.run('terminal_open_here', req, () => {
			this.project(req.projectId);
			return null;
		});
	}

	// ---------------------------------------------------------------- 4.9 Decisions and recovery
	pendingChangesList(req: Requests['pending_changes_list']): Promise<T.PendingChange[]> {
		return this.run('pending_changes_list', req, () => this.project(req.projectId).pending);
	}
	private staleOnce = new Set<string>();
	private decide(projectId: string, change: string, seen: string, accept: boolean): T.ProjectView {
		const p = this.project(projectId);
		const c = p.pending.find((x) => x.id === change);
		if (!c) throw notFound();
		if (this.world.scenario === 'outsideChange' && !this.staleOnce.has(change)) {
			this.staleOnce.add(change);
			c.seenSha256 = '7e'.repeat(32);
			c.diff.entries.push({
				keyPath: 'skips',
				change: 'added',
				before: null,
				after: '{ "step": "2.6", "reason": "frozen already" }'
			});
			throw fail('stale', { kind: 'stale', what: 'readStamp', fresh: c });
		}
		if (c.seenSha256 !== seen) throw fail('stale', { kind: 'stale', what: 'readStamp', fresh: c });
		if (accept) this.ask();
		else if (this.world.scenario === 'conflict')
			throw fail('conflict', { kind: 'conflict', path: c.diff.source, written: [] });
		p.pending = p.pending.filter((x) => x.id !== change);
		if (accept && c.file === 'project') {
			p.conditions.payments = 'no';
			p.stages = rebuildStages(p);
		}
		this.world.inbox = this.world.inbox.filter(
			(i) => !(i.kind === 'outsideChange' && i.project === p.id && p.pending.length === 0)
		);
		this.emit('decision:pending', { projectId: p.id, count: p.pending.length });
		this.touch(p);
		return this.projectView(p);
	}
	pendingChangeAccept(req: Requests['pending_change_accept']): Promise<T.ProjectView> {
		return this.run('pending_change_accept', req, () =>
			this.decide(req.projectId, req.change, req.seenSha256, true)
		);
	}
	pendingChangeReject(req: Requests['pending_change_reject']): Promise<T.ProjectView> {
		return this.run('pending_change_reject', req, () =>
			this.decide(req.projectId, req.change, req.seenSha256, false)
		);
	}
	itemKindConfirm(req: Requests['item_kind_confirm']): Promise<T.GateView> {
		return this.run('item_kind_confirm', req, () => {
			const p = this.project(req.projectId);
			const stage = p.current;
			const item = (p.gates[stage] ?? []).find((i) => i.id === req.item);
			if (!item) throw notFound();
			this.ask();
			item.needsReconfirm = false;
			this.touch(p);
			return this.gate(p, stage);
		});
	}
	progressRebuildPreview(req: Requests['progress_rebuild_preview']): Promise<T.RebuildPreview> {
		return this.run('progress_rebuild_preview', req, () => {
			const p = this.project(req.projectId);
			return {
				previewId: this.preview('rebuild', p.id, null),
				stages: p.stages.map((s) => this.stageView(p, s)),
				lost: [
					'Remembered run inputs',
					'Chat-copied timestamps',
					'Optional-part choices for unsigned stages'
				]
			};
		});
	}
	progressRebuildApply(req: Requests['progress_rebuild_apply']): Promise<T.ProjectView> {
		return this.run('progress_rebuild_apply', req, () => {
			const pv = this.take(req.previewId, 'rebuild');
			const p = this.project(pv.project);
			this.touch(p);
			return this.projectView(p);
		});
	}

	// ---------------------------------------------------------------- 4.10 Packets and run log
	private packet(p: MProject, stage: number): T.PacketView {
		const st = p.stages[stage];
		if (!st) throw notFound();
		const nn = String(stage).padStart(2, '0');
		const pk = p.packets[stage];
		const path = `docs/handoff/stage-${nn}.md`;
		if (!pk) {
			if (st.packetExists) {
				// A valid generated packet (Swings 7, a pasted-back packet).
				return {
					path,
					exists: true,
					sha256: '3c'.repeat(32),
					header: {
						line: `=== HANDOFF PACKET · Stage ${stage} → Stage ${stage + 1} ===`,
						fromStage: stage,
						toStage: stage + 1
					},
					fields: [
						{
							key: 'PROJECT',
							label: 'Project',
							value: [{ kind: 'paragraph', spans: visibleSpans(p.name) }],
							missing: false,
							problems: [],
							note: null
						},
						{
							key: 'APPROVED BY',
							label: 'Approved by',
							value: [{ kind: 'paragraph', spans: visibleSpans('Unsigned') }],
							missing: false,
							problems: [],
							note: null
						},
						{
							key: 'OPEN ITEMS',
							label: 'Open items',
							value: [{ kind: 'paragraph', spans: visibleSpans('none') }],
							missing: false,
							problems: [],
							note: null
						}
					],
					problems: [],
					normalization: [],
					signed: null,
					tamper: null,
					raw: [
						`=== HANDOFF PACKET · Stage ${stage} → Stage ${stage + 1} ===`,
						`PROJECT: ${p.name}`,
						'APPROVED BY: <leave blank for me to sign>',
						'OPEN ITEMS: none'
					].map((t, i) => ({ line: i + 1, spans: visibleSpans(t), bad: false })),
					writtenBy:
						'Written by Gate B. Foreman checks it every time the file changes; nothing here edits it.',
					summary: 'Valid. Ready to sign after the gate passes.'
				};
			}
			return {
				path,
				exists: false,
				sha256: null,
				header: null,
				fields: [],
				problems: [],
				normalization: [],
				signed: null,
				tamper: null,
				raw: [],
				writtenBy: null,
				summary: ''
			};
		}
		const problems = pk.fields.flatMap((f) => f.problems);
		const tamper: T.Diff | null = pk.tampered
			? {
					path,
					exists: true,
					hunks: [
						{
							oldStart: 6,
							newStart: 6,
							lines: [
								{ op: 'del', spans: visibleSpans('OPEN ITEMS: none') },
								{
									op: 'add',
									spans: visibleSpans(
										`OPEN ITEMS: none${String.fromCodePoint(0x202e)} (edited after signing)`
									)
								}
							]
						}
					]
				}
			: null;
		return {
			path,
			exists: true,
			sha256: pk.sha,
			header: { line: pk.line, fromStage: stage, toStage: stage + 1 },
			fields: pk.fields,
			problems,
			normalization:
				pk.stage === 2
					? [{ field: 'APPROVED BY', message: '<sign here> is treated as unsigned' }]
					: [],
			signed: pk.signedBy
				? { name: 'Billy Ribeiro', date: '2026-10-12', byForeman: false, confirmed: true }
				: null,
			tamper,
			raw: pk.raw.map((r) => ({ line: r.line, spans: visibleSpans(r.text), bad: r.bad })),
			writtenBy: pk.writtenBy,
			summary:
				problems.length > 0
					? `Fix ${problems.length === 2 ? 'both' : 'each'}, then attest the Stage ${stage} items and run F.2. Sign stays locked until then.`
					: tamper
						? 'This signed packet differs from its commit. Restore it, or sign the change with a Change Request.'
						: 'Valid and signed.'
		};
	}
	packetView(req: Requests['packet_view']): Promise<T.PacketView> {
		return this.run('packet_view', req, () => this.packet(this.project(req.projectId), req.stage));
	}
	packetNormalizeApply(req: Requests['packet_normalize_apply']): Promise<T.PacketView> {
		return this.run('packet_normalize_apply', req, () => {
			const p = this.project(req.projectId);
			const view = this.packet(p, req.stage);
			if (view.sha256 !== req.seenSha256)
				throw fail('stale', { kind: 'stale', what: 'readStamp', fresh: view });
			this.writable(p);
			this.emit('packet:changed', { projectId: p.id, stage: req.stage, rev: ++p.rev });
			return this.packet(p, req.stage);
		});
	}
	packetSignatureConfirm(req: Requests['packet_signature_confirm']): Promise<T.PacketView> {
		return this.run('packet_signature_confirm', req, () => {
			const p = this.project(req.projectId);
			const view = this.packet(p, req.stage);
			if (view.sha256 !== req.seenSha256)
				throw fail('stale', { kind: 'stale', what: 'readStamp', fresh: view });
			if (!view.signed) throw notAllowed('not-signed-form');
			if (req.stage === 2 && Object.values(p.conditions).includes('undecided'))
				throw notAllowed('undecided-early-condition');
			this.ask();
			return view;
		});
	}
	packetRepairPrepare(
		req: Requests['packet_repair_prepare'],
		progress?: T.OnProgress
	): Promise<T.PrepareOutcome> {
		return this.run('packet_repair_prepare', req, () => {
			const p = this.project(req.projectId);
			const view = this.packet(p, req.stage);
			if (view.problems.length === 0) throw notAllowed('no-problems');
			progress?.({ phase: 'version', found: this.world.claudeVersion });
			const planId = this.uuid('f');
			this.plans.set(planId, {
				project: p.id,
				step: null,
				chain: IDS.chain22,
				confirm: [],
				rearm: { kind: 'exited', chain: IDS.chain22 },
				created: performance.now(),
				used: false,
				tab: `fm-${p.slug}-s${String(req.stage).padStart(2, '0')}-repair`,
				title: `Fix only these problems in ${view.path}: … Do not change anything else.`,
				fresh: false,
				rearmText: 'Packet repair · Fable 5.1 · high · Accept edits · permission grants reset'
			});
			progress?.({ phase: 'done' });
			return {
				kind: 'ready',
				planId,
				expiresInMs: 60_000,
				prompt: [
					{
						kind: 'text',
						spans: visibleSpans(
							`Fix only these problems in ${view.path}:\n${view.problems.map((x) => `- ${x.message}`).join('\n')}\nDo not change anything else.`
						)
					}
				],
				argv: [
					this.world.settings.tools.claude.path ?? 'claude',
					'--resume',
					IDS.chain22,
					'--permission-mode',
					'acceptEdits'
				],
				envNames: ['HOME', 'LANG', 'PATH', 'SHELL', 'TERM', 'USER'],
				runSettingsSha256: '5a'.repeat(32),
				confirm: [],
				rearm: { kind: 'exited', chain: IDS.chain22 },
				warnings: [],
				remaps: []
			};
		});
	}
	metricsExport(req: Requests['metrics_export']): Promise<T.ExportResult> {
		return this.run('metrics_export', req, () =>
			this.dialog === 'cancel'
				? { kind: 'cancelled' }
				: { kind: 'saved', displayPath: '~/Desktop/foreman-metrics-2026-10-19.md' }
		);
	}
	metricsView(req: Requests['metrics_view']): Promise<T.MetricsView> {
		return this.run('metrics_view', req, () => {
			if (req.projectId) this.project(req.projectId);
			const empty = this.world.projects.length === 0;
			return {
				perSurface: empty
					? []
					: [
							{
								surface: 'cli',
								medianTimeToStartMs: 6200,
								p95TimeToStartMs: 9100,
								medianActiveMsPerStep: 1_380_000
							},
							{
								surface: 'chat',
								medianTimeToStartMs: 8900,
								p95TimeToStartMs: 9600,
								medianActiveMsPerStep: 960_000
							}
						],
				manualEditsByStage: empty ? [] : [1, 2].map((stage) => ({ stage, edits: 0 })),
				stagesThroughForeman: empty ? 0 : 2,
				baseline: empty ? null : { baselineMedianMs: 94_000, foremanMedianMs: 6200 },
				runsCounted: empty ? 0 : req.projectId ? 37 : 212,
				p95AllMs: empty ? 0 : 9400
			};
		});
	}
	runsClear(req: Requests['runs_clear']): Promise<null> {
		return this.run('runs_clear', req, () => {
			if (this.world.runs.some((r) => r.live)) throw notAllowed('runs-live');
			this.ask();
			this.world.runs = [];
			this.world.settings.runHistory = { runs: 0, megabytes: 0 };
			return null;
		});
	}
	runsSearch(req: Requests['runs_search']): Promise<T.RunsPage> {
		return this.run('runs_search', req, () => {
			if (req.limit < 1 || req.limit > 200) throw invalidField('limit', 'range');
			const q = req.query.trim().toLowerCase();
			const all = [...this.world.runs]
				.map((r) => r.summary)
				.filter((r) => !req.projectId || r.project === req.projectId)
				.sort((a, b) => b.startedAt.localeCompare(a.startedAt));
			const hit = all.filter(
				(r) =>
					!q ||
					[r.stepTitle, r.tabName, r.projectName, r.step ?? '', r.argv.join(' ')].some((s) =>
						s.toLowerCase().includes(q)
					)
			);
			const from = req.cursor ? Number(req.cursor) : 0;
			const page = hit.slice(from, from + req.limit);
			return {
				runs: page,
				next: from + req.limit < hit.length ? String(from + req.limit) : null,
				total: q ? Math.max(hit.length, 0) : this.world.settings.runHistory.runs || all.length
			};
		});
	}

	// ---------------------------------------------------------------- 4.11 Gates
	private gate(p: MProject, stage: number): T.GateView {
		const s = p.stages[stage];
		if (!s) throw notFound();
		let items = p.gates[stage];
		if (!items) {
			items = (
				stage === 7 ? ['skeleton-runs', 'e2e-green', 'adr-check', 'perf-marks'] : ['inspection']
			).map((id, i) => ({
				id,
				text:
					stage === 7
						? ([
								'The walking skeleton runs end to end',
								'E2E smoke is green in CI',
								'No ADR contradicted by the skeleton',
								'Performance marks are recorded'
							][i] ?? id)
						: `Stage ${stage} inspection items`,
				kind: 'attest' as const,
				how: null,
				status: stage === 7 ? ('pass' as const) : ('waiting' as const),
				evidence: null,
				proposed: null,
				note: stage === 7 ? 'Checked in the Gate A session' : null,
				waitsFor: stage === 7 ? null : 'Attest after the stage packet is written',
				artifacts: [],
				failures: 0,
				needsReconfirm: false,
				session: null
			}));
			p.gates[stage] = items;
		}
		const failed = items.filter((i) => i.status === 'fail').length;
		const waiting = items.filter((i) => i.status === 'waiting').length;
		const signed = s.status === 'signed';
		const allPass = failed === 0 && waiting === 0;
		const packetReady = s.packetExists;
		const closeout: T.CloseoutStep[] = [
			{
				key: 'gateA',
				label: 'Gate A inspection',
				state: stage === 5 || allPass ? 'done' : 'now',
				note: null
			},
			{
				key: 'results',
				label: failed
					? `Results: ${failed} failed`
					: waiting
						? `Results: ${waiting} waiting`
						: 'Results: all passed',
				state: failed ? 'failed' : waiting ? 'now' : 'done',
				note: null
			},
			{
				key: 'gateB',
				label: 'Gate B packet',
				state: allPass && packetReady ? 'done' : 'pending',
				note: 'Runs after every item passes. Writes docs/handoff/stage-0N.md.'
			},
			{
				key: 'gateC',
				label: 'Gate C lessons',
				state: allPass && packetReady ? 'done' : 'pending',
				note: `Continues the gate session. Needs an entry headed "Stage ${stage}" in docs/lessons.md.`
			},
			{
				key: 'sign',
				label: signed ? 'Signed' : 'Sign',
				state: signed ? 'done' : allPass && packetReady ? 'now' : 'locked',
				note: null
			}
		];
		const whyNot: string[] = [];
		if (failed || waiting)
			whyNot.push(
				[failed ? `${failed} item failed` : '', waiting ? `${waiting} wait for your note` : '']
					.filter(Boolean)
					.join(' and ')
			);
		if (!packetReady) whyNot.push('Then Gate B and Gate C');
		return {
			stage,
			state: signed ? 'signed' : failed ? 'failed' : allPass ? 'passed' : 'inProgress',
			items,
			closeout,
			signable: !signed && allPass && packetReady,
			whyNot,
			importedFrom:
				stage === 5
					? 'Imported from fm-tradelog-s05-5-gate-a at 14:12. Confirm or change each result.'
					: null
		};
	}
	gateView(req: Requests['gate_view']): Promise<T.GateView> {
		return this.run('gate_view', req, () => this.gate(this.project(req.projectId), req.stage));
	}
	gateItemAttest(req: Requests['gate_item_attest']): Promise<T.GateView> {
		return this.run('gate_item_attest', req, () => {
			const p = this.project(req.projectId);
			const g = this.gate(p, req.stage);
			const item = g.items.find((i) => i.id === req.item);
			const live = (p.gates[req.stage] ?? []).find((i) => i.id === req.item);
			if (!item || !live) throw notFound();
			if (live.needsReconfirm) throw notAllowed('kind-needs-reconfirm');
			const o = req.outcome;
			if (o.kind === 'done') {
				if (o.note.trim().length < 15) throw notAllowed('note-too-short');
				if (
					(p.gates[req.stage] ?? []).some(
						(i) => i.id !== req.item && i.note?.trim() === o.note.trim()
					)
				)
					throw notAllowed('note-repeated');
				live.status = 'pass';
				live.note = o.note;
			} else if (o.kind === 'notApplicable') {
				if (!o.reason.trim()) throw notAllowed('reason-required');
				live.status = 'notApplicable';
				live.note = o.reason;
			} else {
				live.status = 'waiting';
				live.note = null;
			}
			live.proposed = null;
			this.touch(p);
			return this.gate(p, req.stage);
		});
	}
	gateImportResults(req: Requests['gate_import_results']): Promise<T.ImportProposal[]> {
		return this.run('gate_import_results', req, () => {
			const p = this.project(req.projectId);
			const items = this.gate(p, req.stage).items;
			return items.map((i) => ({
				item: i.id,
				proposed: i.status === 'fail' ? 'fail' : (i.proposed ?? 'unmatched'),
				evidence: i.evidence ?? visibleSpans('No matching result in the Gate A message.')
			}));
		});
	}
	gateImportConfirm(req: Requests['gate_import_confirm']): Promise<T.GateView> {
		return this.run('gate_import_confirm', req, () => {
			const p = this.project(req.projectId);
			this.gate(p, req.stage);
			const live = (p.gates[req.stage] ?? []).find((i) => i.id === req.item);
			if (!live) throw notFound();
			live.status = req.result;
			live.evidence = visibleSpans(req.evidence);
			live.proposed = null;
			this.touch(p);
			return this.gate(p, req.stage);
		});
	}
	gateFixStart(
		req: Requests['gate_fix_start'],
		progress?: T.OnProgress
	): Promise<T.PrepareOutcome> {
		return this.run('gate_fix_start', req, () => {
			const p = this.project(req.projectId);
			const live = this.gate(p, req.stage).items.find((i) => i.id === req.item);
			if (!live) throw notFound();
			if (live.status !== 'fail') throw notAllowed('item-not-failed');
			progress?.({ phase: 'version', found: this.world.claudeVersion });
			const planId = this.uuid('f');
			const tab = `fm-${p.slug}-s${String(req.stage).padStart(2, '0')}-fix-${req.item}`;
			this.plans.set(planId, {
				project: p.id,
				step: null,
				chain: null,
				confirm: [],
				rearm: null,
				created: performance.now(),
				used: false,
				tab,
				title: `Fix: ${live.text}`,
				fresh: true,
				rearmText: null
			});
			progress?.({ phase: 'done' });
			return {
				kind: 'ready',
				planId,
				expiresInMs: 60_000,
				prompt: [
					{
						kind: 'text',
						spans: visibleSpans(
							`This inspection item failed: ${live.text}\nEvidence follows. Plan the smallest fix, then wait for my approval.`
						)
					}
				],
				argv: [
					this.world.settings.tools.claude.path ?? 'claude',
					'--session-id',
					'<new uuid>',
					'--model',
					'opus',
					'--effort',
					'high',
					'--permission-mode',
					'plan',
					'--name',
					tab
				],
				envNames: ['HOME', 'LANG', 'PATH', 'SHELL', 'TERM', 'USER'],
				runSettingsSha256: '5a'.repeat(32),
				confirm: [],
				rearm: null,
				warnings:
					live.failures >= 2 ? ['Third failure: Foreman offers the F.4 prompt instead.'] : [],
				remaps: []
			};
		});
	}

	// ---------------------------------------------------------------- 4.12 Signing
	signPreview(req: Requests['sign_preview']): Promise<T.SignPreview> {
		return this.run('sign_preview', req, () => {
			const p = this.project(req.projectId);
			if (p.readOnly) throw fail('readOnly', { kind: 'readOnly', ...p.readOnly });
			const g = this.gate(p, req.stage);
			if (!g.signable) throw notAllowed('not-signable');
			if (!/^\d{4}-\d{2}-\d{2}$/.test(req.date)) throw invalidField('date', 'isoDate');
			if (!req.name.trim()) throw invalidField('name', 'required');
			const nn = String(req.stage).padStart(2, '0');
			const path = `docs/handoff/stage-${nn}.md`;
			const add = (t: string): T.DiffLine => ({ op: 'add', spans: visibleSpans(t) });
			const preview: T.SignPreview = {
				previewId: '',
				packetSha256: '3c'.repeat(32),
				short: 'a3f9c1',
				addedFields: {
					path,
					exists: true,
					hunks: [
						{
							oldStart: 3,
							newStart: 3,
							lines: [
								{ op: 'del', spans: visibleSpans('APPROVED BY: <leave blank for me to sign>') },
								add(`APPROVED BY: ${req.name} on ${req.date}`),
								add('SKIPPED: none in this stage'),
								add('SUBSTITUTIONS: none'),
								add('ATTESTED:'),
								...g.items.filter((i) => i.note).map((i) => add(`  ${i.id}: ${i.note ?? ''}`))
							]
						}
					]
				},
				paths: [path, `.foreman/stages/stage-${nn}.json`],
				confirmConditions: req.stage === 2 ? ['payments'] : null,
				chainBusy: false,
				blockers: [],
				branch: req.branch.kind === 'current' ? p.branch : req.branch.branch,
				commitMessage: `handoff: stage ${nn}`,
				pathNotes: {}
			};
			preview.previewId = this.preview('sign', p.id, { stage: req.stage, branch: preview.branch });
			return preview;
		});
	}
	signApply(req: Requests['sign_apply'], progress?: T.OnSignProgress): Promise<T.SignResult> {
		return this.run('sign_apply', req, () => {
			const pv = this.take(req.previewId, 'sign');
			this.ask();
			const p = this.project(pv.project);
			const { stage } = pv.data as { stage: number };
			const nn = String(stage).padStart(2, '0');
			const paths = [`docs/handoff/stage-${nn}.md`, `.foreman/stages/stage-${nn}.json`];
			progress?.({ step: 'reRead' });
			for (const path of paths) {
				progress?.({ step: 'write', path });
				progress?.({ step: 'hashObject', path });
			}
			progress?.({ step: 'writeTree' });
			progress?.({ step: 'commitTree', signed: false });
			progress?.({ step: 'updateRef' });
			progress?.({ step: 'refreshIndex' });
			for (const path of paths) progress?.({ step: 'readBack', path, ok: true });
			progress?.({ step: 'done', commit: '4e1b2c7' });
			const s = p.stages[stage];
			if (s) {
				s.status = 'signed';
				s.statusWhy = '2026-10-19';
				s.signed = { date: '2026-10-19', commit: '4e1b2c7', branch: p.branch, merged: true };
			}
			const next = p.stages.find((x) => x.stage > stage && x.status !== 'notApplicable');
			if (next) {
				next.status = 'ready';
				next.statusWhy = null;
			}
			this.world.inbox = this.world.inbox.filter((i) => !(i.project === p.id && i.stage === stage));
			this.touch(p);
			return { kind: 'signed', commit: '4e1b2c7', stage };
		});
	}
	signRetryCommit(req: Requests['sign_retry_commit']): Promise<T.SignResult> {
		return this.run('sign_retry_commit', req, () => {
			this.project(req.projectId);
			throw notAllowed('not-commit-pending');
		});
	}
	tamperRestore(req: Requests['tamper_restore']): Promise<T.PacketView> {
		return this.run('tamper_restore', req, () => {
			const p = this.project(req.projectId);
			const view = this.packet(p, req.stage);
			if (view.sha256 !== req.seenSha256)
				throw fail('stale', { kind: 'stale', what: 'readStamp', fresh: view });
			this.writable(p);
			const pk = p.packets[req.stage];
			if (pk) pk.tampered = false;
			const st = p.stages[req.stage];
			if (st) st.packetProblems = 0;
			this.emit('packet:changed', { projectId: p.id, stage: req.stage, rev: ++p.rev });
			return this.packet(p, req.stage);
		});
	}

	// ---------------------------------------------------------------- 4.13 Lessons, manifest, assembler
	lessonsStatus(req: Requests['lessons_status']): Promise<T.LessonsStatus> {
		return this.run('lessons_status', req, () => {
			const p = this.project(req.projectId);
			const signed = p.stages[req.stage]?.status === 'signed';
			return {
				met: signed,
				entries: signed
					? [{ id: `L-00${req.stage}`, title: `Stage ${req.stage} · what we learned` }]
					: [],
				nextId: 'L-012',
				filePresent: true
			};
		});
	}
	private manifest(p: MProject): T.Manifest {
		const files: T.ManifestRow[] = [1, 2, 3, 4]
			.filter((n) => p.stages[n]?.status === 'signed')
			.map((n) => ({
				path: `docs/handoff/stage-0${n}.md`,
				exists: true,
				inProject: this.inProject.has(`${p.id}/${n}`),
				producer: n === 1 ? '1.4' : null
			}));
		return {
			files,
			ready: files.filter((f) => f.inProject).length,
			total: files.length,
			firstProducer: files.length ? null : '1.4'
		};
	}
	private inProject = new Set<string>();
	manifestView(req: Requests['manifest_view']): Promise<T.Manifest> {
		return this.run('manifest_view', req, () => this.manifest(this.project(req.projectId)));
	}
	manifestMark(req: Requests['manifest_mark']): Promise<T.Manifest> {
		return this.run('manifest_mark', req, () => {
			const p = this.project(req.projectId);
			const n = /stage-0(\d)\.md$/.exec(req.path)?.[1];
			if (!n || !this.manifest(p).files.some((f) => f.path === req.path)) throw notFound();
			if (req.inProject) this.inProject.add(`${p.id}/${n}`);
			else this.inProject.delete(`${p.id}/${n}`);
			return this.manifest(p);
		});
	}
	private asmRows = new Map<string, T.AssemblerRow>();
	private assembler(p: MProject): T.Assembler {
		const s4 = p.stages[4];
		if (!s4?.packetExists) throw notAllowed('no-stage-4-packet');
		const def: [string, T.AssemblerRow['state'], string | null, string | null][] = [
			['docs/MASTER-SPEC.md', 'exists', null, null],
			['docs/requirements.md', 'exists', null, null],
			['docs/design-doc.md', 'exists', null, null],
			['docs/migrations/0001_init.sql', 'exists', null, null],
			[
				'docs/tickets.md',
				'empty',
				null,
				'The file exists but has no content. Paste the tickets from 4.6.'
			],
			['docs/threat-model.md', 'missing', null, null],
			['docs/design/tokens.css', 'missing', null, null],
			[
				'CLAUDE.md',
				'exists',
				null,
				'Claude Code loads this file in every session. Writing it asks you again in a macOS dialog.'
			],
			[
				'.github/workflows/ci.yml',
				'refused',
				'github-dir',
				"Foreman doesn't write under .github/ (path policy). Create this file yourself or in a Stage 5 session."
			],
			['docs/adr/README.md', 'exists', null, null],
			['docs/contracts/ipc.md', 'exists', null, null],
			['docs/lessons.md', 'exists', null, null]
		];
		const rows = def.map(
			([path, state, rule, note]) =>
				this.asmRows.get(`${p.id}/${path}`) ?? { path, state, rule, note }
		);
		const late: T.LateConditionView[] = [
			{
				condition: 'has_database',
				value: p.conditions.has_database,
				proposed: p.conditions.has_database === 'undecided' ? 'yes' : null,
				evidence: 'docs/migrations/0001_init.sql',
				decided: null
			},
			{
				condition: 'has_uploads',
				value: p.conditions.has_uploads,
				proposed: null,
				evidence: null,
				decided: null
			},
			{
				condition: 'has_email',
				value: p.conditions.has_email,
				proposed: null,
				evidence: null,
				decided: null
			},
			{
				condition: 'has_integrations',
				value: p.conditions.has_integrations,
				proposed: null,
				evidence: null,
				decided:
					p.conditions.has_integrations !== 'undecided'
						? 'Decided by you on 2026-10-18 ("broker CSV import only, no API").'
						: null
			}
		];
		return {
			rows,
			complete: rows.every((r) => r.state === 'exists' || r.state === 'refused'),
			lateConditions: s4.status === 'signed' ? [] : late
		};
	}
	assemblerView(req: Requests['assembler_view']): Promise<T.Assembler> {
		return this.run('assembler_view', req, () => this.assembler(this.project(req.projectId)));
	}
	assemblerPastePreview(req: Requests['assembler_paste_preview']): Promise<T.PastePreview> {
		return this.run('assembler_paste_preview', req, () => {
			const p = this.project(req.projectId);
			const row = this.assembler(p).rows.find((r) => r.path === req.path);
			if (!row) throw notFound();
			if (row.state === 'refused')
				throw fail('policyViolation', {
					kind: 'policyViolation',
					path: req.path,
					rule: row.rule ?? 'path-policy'
				});
			const text = this.world.clipboard;
			if (!text.trim()) throw fail('clipboard', { kind: 'clipboard', why: 'empty' });
			return {
				previewId: this.preview('asm', p.id, req.path),
				kind: 'output',
				targets: [
					{
						path: req.path,
						exists: row.state !== 'missing',
						hunks: [
							{
								oldStart: 0,
								newStart: 1,
								lines: text
									.split('\n')
									.slice(0, 30)
									.map((l) => ({ op: 'add' as const, spans: visibleSpans(l) }))
							}
						]
					}
				],
				problems: [],
				notes: row.state === 'exists' ? ['Overwrites the current file'] : [],
				packetFields: null
			};
		});
	}
	assemblerPasteApply(req: Requests['assembler_paste_apply']): Promise<T.Assembler> {
		return this.run('assembler_paste_apply', req, () => {
			const pv = this.take(req.previewId, 'asm');
			const p = this.project(pv.project);
			const path = pv.data as string;
			if (path === 'CLAUDE.md') this.ask();
			if (this.world.scenario === 'conflict')
				throw fail('conflict', { kind: 'conflict', path, written: [] });
			this.asmRows.set(`${p.id}/${path}`, { path, state: 'exists', rule: null, note: null });
			this.touch(p);
			return this.assembler(p);
		});
	}

	// ---------------------------------------------------------------- 4.14 Desktop, overlay, run sheet
	desktopOpen(req: Requests['desktop_open'], progress?: T.OnProgress): Promise<T.DesktopOpened> {
		return this.run('desktop_open', req, () => {
			const p = this.project(req.projectId);
			const st = this.step(p, req.step);
			if (st.tpl.surface !== 'desktop') throw notAllowed('not-desktop-step');
			progress?.({ phase: 'done' });
			return { chips: { chips: [], link: null, linkLabel: null } };
		});
	}
	desktopHandoff(req: Requests['desktop_handoff']): Promise<null> {
		return this.run('desktop_handoff', req, () => {
			const r = this.chainRun(req.chain);
			if (!r) throw notFound();
			if (r.live) throw notAllowed('chain-live');
			return null;
		});
	}
	addStep(req: Requests['add_step']): Promise<T.ProjectView> {
		return this.run('add_step', req, () => {
			const p = this.project(req.projectId);
			const s = p.stages[req.stage];
			if (!s) throw notFound();
			if (req.config.start.mode === 'auto' && req.tag === 'core') throw notAllowed('invalid-start');
			for (const o of req.config.out)
				if (o.startsWith('/') || o.includes('..') || o.startsWith('.github/'))
					throw fail('policyViolation', { kind: 'policyViolation', path: o, rule: 'out-path' });
			this.writable(p);
			const idx = s.steps.findIndex((x) => x.tpl.key === req.after);
			const n = s.steps.filter((x) => x.tpl.tag === 'added').length + 1;
			const from = this.step(p, req.copyConfigFrom);
			const key = `x-${req.stage}-${n + (req.stage === 2 ? 1 : 0)}`;
			const st: MStep = {
				...structuredClone(from),
				tpl: { ...from.tpl, key, number: key.replace(/-/g, '‑'), title: req.title, tag: 'added' },
				state: 'notStarted',
				stateText: 'Not started, added',
				why: 'Added steps always confirm before running.',
				run: null,
				next: false,
				inputs: [],
				parts: []
			};
			PROMPTS[key] = req.text;
			s.steps.splice(idx + 1, 0, st);
			this.touch(p);
			return this.projectView(p);
		});
	}
	addItem(req: Requests['add_item']): Promise<T.GateView> {
		return this.run('add_item', req, () => {
			const p = this.project(req.projectId);
			this.gate(p, req.stage);
			this.writable(p);
			(p.gates[req.stage] ?? []).push({
				id: `xi-${req.stage}-${(p.gates[req.stage] ?? []).length + 1}`,
				text: req.text,
				kind: 'attest',
				how: 'Added for this project',
				status: 'waiting',
				evidence: null,
				proposed: null,
				note: null,
				waitsFor: 'Attest',
				artifacts: [],
				failures: 0,
				needsReconfirm: false,
				session: null
			});
			this.touch(p);
			return this.gate(p, req.stage);
		});
	}
	runSheetExport(req: Requests['run_sheet_export']): Promise<T.ExportResult> {
		return this.run('run_sheet_export', req, () => {
			const p = this.project(req.projectId);
			if (!p.stages[req.stage]) throw notFound();
			return this.dialog === 'cancel'
				? { kind: 'cancelled' }
				: {
						kind: 'saved',
						displayPath: `~/Desktop/${p.slug}-stage-${String(req.stage).padStart(2, '0')}-run-sheet.md`
					};
		});
	}
}

function freshStages(p: MProject): MStage[] {
	return rebuildStages({ ...p, stages: [] }, true);
}

/** Re-applies the conditions to a project's steps (the rule engine, mock edition). */
function rebuildStages(p: MProject, fresh = false): MStage[] {
	const tmpl = buildWorld('happy').projects[0];
	if (!tmpl) return p.stages;
	const base: MStage[] = structuredClone(tmpl.stages);
	const out: MStage[] = [];
	for (const s of base) {
		const old = fresh ? undefined : p.stages.find((x) => x.stage === s.stage);
		const ns: MStage = old
			? { ...old, steps: [] }
			: {
					...s,
					steps: [],
					status: 'locked',
					statusWhy: null,
					signed: null,
					packetExists: false,
					packetProblems: 0
				};
		if (fresh) {
			ns.status =
				s.stage === (p.conditions.idea_known === 'no' ? 0 : 1)
					? 'ready'
					: s.stage > 5
						? 'needsAnnotation'
						: 'locked';
			if (s.stage === 0 && p.conditions.idea_known !== 'no') ns.status = 'notApplicable';
		}
		if (s.stage === 10)
			ns.status =
				p.conditions.payments === 'no'
					? 'notApplicable'
					: p.conditions.payments === 'undecided'
						? 'undecided'
						: ns.status === 'undecided' || ns.status === 'notApplicable'
							? 'needsAnnotation'
							: ns.status;
		if (s.stage === 10)
			ns.statusWhy =
				ns.status === 'notApplicable'
					? 'payments = no'
					: ns.status === 'undecided'
						? 'payments = undecided'
						: null;
		const src = old?.steps ?? s.steps;
		for (const st of src) {
			const keep = !fresh && old && ['done', 'running', 'copied', 'skipped'].includes(st.state);
			if (keep) {
				ns.steps.push(st);
				continue;
			}
			const t = st.tpl;
			const c = p.conditions;
			let v: T.TriState = 'yes';
			if (t.key.startsWith('0.'))
				v = c.idea_known === 'yes' ? 'no' : c.idea_known === 'no' ? 'yes' : 'undecided';
			else if (t.when) {
				const vals = t.when.map((id) => c[id]);
				v = vals.includes('yes') ? 'yes' : vals.includes('undecided') ? 'undecided' : 'no';
			}
			const note = t.key.startsWith('0.')
				? `idea_known = ${c.idea_known}`
				: (t.when ?? []).map((id) => `${id} = ${c[id]}`).join(', ');
			const copy = structuredClone(st);
			if (st.state === 'needsAnnotation') {
				ns.steps.push(copy);
				continue;
			}
			if (v === 'no')
				Object.assign(copy, {
					state: 'notApplicable',
					stateText: 'Not applicable',
					why: st.why?.startsWith('Not applicable')
						? st.why
						: 'Not applicable: its condition is false',
					conditionNote: note,
					next: false
				});
			else if (v === 'undecided') {
				const cond = (t.when ?? ['idea_known']).find((id) => c[id] === 'undecided') ?? 'idea_known';
				Object.assign(copy, {
					state: 'undecided',
					stateText: 'Undecided',
					why: `Decide ${cond} to continue`,
					conditionNote: `${cond} = undecided`,
					next: false
				});
			} else if (st.state === 'notApplicable' || st.state === 'undecided')
				Object.assign(copy, {
					state: 'notStarted',
					stateText: 'Not started',
					why: null,
					conditionNote: null
				});
			if (fresh && copy.state === 'notStarted')
				Object.assign(copy, {
					stateText: copy.tpl.kind === 'lessons' ? 'After the inspection items' : 'Not started',
					why: null,
					next: false,
					run: null,
					chain: null,
					copiedAt: null
				});
			ns.steps.push(copy);
		}
		if (fresh) {
			const first = ns.steps.find((x) => x.state === 'notStarted');
			if (first && ns.status === 'ready') first.next = true;
		}
		out.push(ns);
	}
	return out;
}

/** The mock's phases list, re-exported for the miniature strip in tests. */
export { PHASES };

let shared: MockIpcClient | undefined;

/** The app's single mock instance. `?scenario=` and `VITE_MOCK_SCENARIO` pick the world (§7.2 rule 4). */
export function mockClient(): MockIpcClient {
	if (shared) return shared;
	const fromUrl =
		typeof location === 'undefined' ? null : new URLSearchParams(location.search).get('scenario');
	const fromEnv = import.meta.env.VITE_MOCK_SCENARIO;
	const pick = [fromUrl, fromEnv].find(
		(s): s is Scenario => !!s && (SCENARIOS as readonly string[]).includes(s)
	);
	shared = new MockIpcClient(pick ?? 'happy');
	if (
		typeof location !== 'undefined' &&
		new URLSearchParams(location.search).get('dialog') === 'cancel'
	)
		shared.setDialogAnswer('cancel');
	if (typeof window !== 'undefined')
		(window as unknown as { __foremanMock?: MockIpcClient }).__foremanMock = shared;
	return shared;
}

/** Resets the shared instance's world (component tests); event subscriptions stay. */
export function resetMock(scenario: Scenario = 'happy'): MockIpcClient {
	const m = mockClient();
	m.setScenario(scenario);
	m.setDialogAnswer('ok');
	m.streamDelayMs = 0;
	return m;
}

/** Type check: every command in COMMAND_MAP is a method here (tsc proves it via `implements`). */
export const MOCK_COMMANDS: readonly CommandName[] = Object.keys(COMMAND_MAP) as CommandName[];
