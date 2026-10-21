import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { IpcFailure } from '#lib/ipc/errors.ts';
import { fresh, IDS, project } from '#lib/test/screen.ts';

import StageWorkspace from './StageWorkspace.svelte';

// M2-21 / M2-22 acceptance criteria against the mock (comp): not-applicable, core, undecided and
// needs-annotation rows (AC-F004-03, -04, -07, AC-F030-02), the spec line (AC-F005-03, -04), what
// the workspace shows for the selected step (AC-F030-01), run inputs and the prompt
// (AC-F006-12, -13, -14, AC-F008-11) and the exact command (AC-F008-17).
const ADDED = 'Re-rank pain with interview notes';
const ADDED_KEY = 'x-2-1';
const row = (container: Element, number: string): HTMLElement => {
	const li = [...container.querySelectorAll<HTMLElement>('li.step')].find((l) =>
		number === ADDED
			? l.querySelector('.step-title')?.textContent.includes(ADDED)
			: l.querySelector('.callout')?.textContent.trim() === number
	);
	if (!li) throw new Error(`no row ${number}`);
	return li;
};
const ensureOpen = async (r: HTMLElement): Promise<void> => {
	const b = r.querySelector<HTMLButtonElement>('.step-title button');
	if (b && b.getAttribute('aria-expanded') !== 'true') b.click();
	await vi.waitFor(() => expect(r.querySelector('.step-body .actions')).not.toBeNull());
};
const names = (el: Element) => [...el.querySelectorAll('button')].map((b) => b.textContent.trim());

describe('StageWorkspace rows (AC-F004, AC-F030)', () => {
	it('AC-F004-03: a not-applicable row is hatched, names its condition and has no Skip', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo: 2 });
		const r = row(s.container, '2.1');
		expect(r.classList.contains('is-na')).toBe(true);
		expect(r.textContent).toMatch(/Not applicable: TradeLog isn't sold/);
		expect(r.textContent).toMatch(/commercial = no/);
		expect(r.querySelector('button')).toBeNull();
		expect(names(r).some((n) => n.includes('Skip'))).toBe(false);
	});

	it('AC-F004-04: a step behind an undecided condition says what to decide and cannot run', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo: 3 });
		const r = row(s.container, '3.1');
		expect(r.textContent).toContain('Decide payments to continue');
		await ensureOpen(r);
		expect(r.querySelector('.actions .primary')?.getAttribute('aria-disabled')).toBe('true');
	});

	it('AC-F004-07 / AC-F030-02: a core row carries the lock and never offers Skip; a skip over IPC is rejected', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo: 2 });
		const core = row(s.container, '2.5');
		const lock = core.querySelector('.lock');
		expect(lock?.getAttribute('title')).toBe(
			'Core: protects production quality. Change it in the blueprint with a Change Request.'
		);
		await ensureOpen(core);
		expect(core.textContent).toContain('Core: protects production quality.');
		expect(names(core).some((n) => n.includes('Skip'))).toBe(false);
		// Over IPC: rejected, with a reason, and the step is unchanged.
		await expect(
			m.stepSkip({ projectId: IDS.tradelog, step: '2.5', reason: 'not needed' })
		).rejects.toBeInstanceOf(IpcFailure);
		const after = await m.projectOpen({ projectId: IDS.tradelog });
		expect(after.stages.find((x) => x.stage === 2)?.steps.find((x) => x.key === '2.5')?.state).toBe(
			'notStarted'
		);
	});

	it('AC-F030-02: a needs-annotation row says so and offers nothing to run', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo: 6 });
		const rows = [...s.container.querySelectorAll<HTMLElement>('li.step')];
		expect(rows.length).toBeGreaterThan(0);
		for (const r of rows) {
			expect(r.textContent).toMatch(/needs annotation/i);
			expect(r.querySelector('.primary')).toBeNull();
			expect(names(r).some((n) => n.includes('Skip'))).toBe(false);
		}
	});
});

describe('StageWorkspace spec line (AC-F005)', () => {
	it('AC-F005-03: a substituted model reads "Fable 5.1 · high (Max) → Opus 5.5 · xhigh (substituted)"', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.foreman);
		const s = await render(StageWorkspace, { project: p, stageNo: 2 });
		const tips = [...row(s.container, '2.2').querySelectorAll('[role="tooltip"]')].map((t) =>
			t.textContent.trim()
		);
		expect(tips).toContain('Fable 5.1 · high (Max) → Opus 5.5 · xhigh (substituted)');
		// The edited values are shown, not the template's.
		const edited = [...row(s.container, '2.2').querySelectorAll('.edited')].map((b) =>
			b.textContent.trim()
		);
		expect(edited).toContain('Opus 5.5');
		expect(edited).toContain('xhigh');
	});

	it('AC-F005-04: hovering or focusing a value says where it came from', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo: 2 });
		// Template: the title says so.
		const tpl = row(s.container, '2.5').querySelector('dd.model');
		expect(tpl?.getAttribute('title')).toBe('Model: From the template');
		// Override: dotted, with the tooltip naming the override.
		await m.runOverrideSet({
			projectId: IDS.tradelog,
			step: '2.5',
			over: { model: 'sonnet', effort: null, mode: null }
		});
		const p2 = await m.projectOpen({ projectId: IDS.tradelog });
		const s2 = await render(StageWorkspace, { project: p2, stageNo: 2 });
		const tips = [...row(s2.container, '2.5').querySelectorAll('[role="tooltip"]')].map((t) =>
			t.textContent.trim()
		);
		expect(tips.some((t) => t.includes('your override for this run'))).toBe(true);
		// Substitution: reachable by keyboard (a button), described by its tooltip.
		const pf = await project(m, IDS.foreman);
		const sf = await render(StageWorkspace, { project: pf, stageNo: 2 });
		const b = row(sf.container, '2.2').querySelector<HTMLButtonElement>('.edited');
		expect(b?.getAttribute('aria-describedby')).toBeTruthy();
		expect(document.getElementById(b?.getAttribute('aria-describedby') ?? '')?.textContent).toMatch(
			/substituted/
		);
	});
});

describe('StageWorkspace selected step (AC-F030-01, AC-F006, AC-F008)', () => {
	async function open(number: string, stageNo = 2) {
		const m = fresh('happy');
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo, stepKey: number });
		const r = row(s.container, number === ADDED_KEY ? ADDED : number);
		await ensureOpen(r);
		await vi.waitFor(() => expect(r.querySelector('.step-body .actions')).not.toBeNull());
		return { m, s, r };
	}

	it('AC-F030-01 / AC-F006-12: the step shows its spec line, its input above the prompt, and the primary action for its surface', async () => {
		const { r } = await open(ADDED_KEY);
		expect(r.querySelector('dl.spec-line')).not.toBeNull();
		expect(r.querySelector('.primary')?.textContent).toMatch(/Run in terminal/);
		const input = r.querySelector('textarea');
		const prompt = r.querySelector('pre.prompt');
		expect(input).not.toBeNull();
		expect(prompt).not.toBeNull();
		// Above the prompt in document order; labelled; remembered values say so and are prefilled.
		expect(
			(input as Node).compareDocumentPosition(prompt as Node) & Node.DOCUMENT_POSITION_FOLLOWING
		).toBeTruthy();
		expect(document.querySelector(`label[for="${input?.id}"]`)?.textContent).toMatch(/run input/);
		if (!(input instanceof HTMLTextAreaElement)) throw new Error('no input');
		expect(input.value.length).toBeGreaterThan(0);
	});

	it('AC-F030-01: a chat step’s primary action is Copy and open', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.foreman);
		const s = await render(StageWorkspace, { project: p, stageNo: 4 });
		const r = row(s.container, '4.5');
		await ensureOpen(r);
		expect(r.querySelector('.primary')?.textContent).toMatch(/Copy and open/);
		// The same step also offers the plain copy.
		expect(names(r)).toContain('Copy prompt');
	});

	it('AC-F006-13 / AC-F008-11: an empty required input is an attention pill that focuses its field, and Run stays disabled until it is filled', async () => {
		const { m, s } = await open(ADDED_KEY);
		await m.runInputSet({ projectId: IDS.tradelog, step: ADDED_KEY, name: 'notes', value: '' });
		const p2 = await m.projectOpen({ projectId: IDS.tradelog });
		await s.unmount();
		const s2 = await render(StageWorkspace, { project: p2, stageNo: 2, stepKey: ADDED_KEY });
		const r2 = row(s2.container, ADDED);
		await vi.waitFor(() => expect(r2.querySelector('.step-body textarea')).not.toBeNull());
		const input = r2.querySelector<HTMLTextAreaElement>('textarea');
		expect(input?.value).toBe('');
		expect(input?.getAttribute('aria-invalid')).toBe('true');
		expect(r2.textContent).toMatch(/Fill .* to run this step/);
		const pill = r2.querySelector<HTMLButtonElement>('.tok-missing');
		expect(pill).not.toBeNull();
		pill?.click();
		expect(document.activeElement).toBe(input);
		expect(r2.querySelector('.actions .primary')?.getAttribute('aria-disabled')).toBe('true');
		// Filled again: Run is no longer blocked.
		await m.runInputSet({ projectId: IDS.tradelog, step: ADDED_KEY, name: 'notes', value: 'ok' });
		const v = await m.stepView({ projectId: IDS.tradelog, step: ADDED_KEY });
		expect(v.blockers).toEqual([]);
	});

	it('AC-F006-14: an input over 64 KB is accepted with a warning that chat paste limits may apply', async () => {
		const m = fresh('happy');
		await project(m, IDS.tradelog);
		const big = 'x'.repeat(64 * 1024 + 1);
		const v = await m.runInputSet({
			projectId: IDS.tradelog,
			step: ADDED_KEY,
			name: 'notes',
			value: big
		});
		const inp = v.inputs.find((i) => i.name === 'notes');
		expect(inp?.value.length).toBe(big.length);
		expect(inp?.warning).toMatch(/Chat paste limits may apply/);
		const p = await m.projectOpen({ projectId: IDS.tradelog });
		const s = await render(StageWorkspace, { project: p, stageNo: 2, stepKey: ADDED_KEY });
		const r = row(s.container, ADDED);
		await vi.waitFor(() => expect(r.textContent).toMatch(/Chat paste limits may apply/));
	});

	it('AC-F008-17: the resolved prompt, the exact command and the environment names are one click away', async () => {
		const { r } = await open(ADDED_KEY);
		expect(r.querySelector('pre.prompt')).not.toBeNull();
		const details = r.querySelector('details.argv');
		expect(details).not.toBeNull();
		expect((details as HTMLDetailsElement).open).toBe(false);
		details?.querySelector('summary')?.click();
		await vi.waitFor(() => expect((details as HTMLDetailsElement).open).toBe(true));
		const text = details?.textContent ?? '';
		expect(text).toMatch(/--name/);
		expect(text).toMatch(/Environment names \(values stay in Rust\):/);
		// Names only: no value is rendered for any variable.
		expect(text).not.toMatch(/=\S+.*Environment names/);
	});
	it('AC-F030-03: a read-only project states why in a banner and no Run action is enabled', async () => {
		const m = fresh('readOnly');
		const p = await project(m, IDS.tradelog);
		expect(p.readOnly?.reason).toBe('lockMismatch');
		const s = await render(StageWorkspace, { project: p, stageNo: 2 });
		const banner = s.container.querySelector('.banner[role="alert"]');
		expect(banner?.textContent).toMatch(/This project is open read-only\./);
		expect(banner?.textContent).toContain("doesn't match the hash in .foreman/blueprint.lock.json");
		const r = row(s.container, '2.5');
		await ensureOpen(r);
		const primary = r.querySelector('.actions .primary');
		expect(primary?.getAttribute('aria-disabled')).toBe('true');
		expect(r.textContent).toMatch(/Open read-only:/);
		// The hidden path is closed as well: the command the button would send does not launch.
		const out = await m.launchPrepare({
			projectId: IDS.tradelog,
			step: '2.5',
			entry: 'stageView',
			mode: 'normal'
		});
		expect(out.kind).toBe('blocked');
		if (out.kind === 'blocked') expect(out.blockers.map((b) => b.kind)).toContain('readOnly');
	});
});
