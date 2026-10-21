import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { doNext } from '#lib/actions/run.ts';
import type { NextStepRow, SheetStatus } from '#lib/ipc/types.ts';
import { to } from '#lib/nav/routes.ts';
import { fresh } from '#lib/test/screen.ts';

import Today from './Today.svelte';

// M2-20 acceptance criteria against the mock (comp): AC-F017-01 to AC-F017-05.
const topRow = (container: Element): Promise<Element> =>
	vi.waitFor(() => {
		const row = container.querySelector('li.is-top');
		if (!row) throw new Error('no top row');
		return row;
	});

describe('Today (AC-F017)', () => {
	it('AC-F017-01: each row shows the next step with its spec line and primary action, and the top row has focus', async () => {
		fresh('happy');
		const s = await render(Today);
		const top = await topRow(s.container);
		expect(top.querySelector('.proj b')?.textContent.length).toBeGreaterThan(0);
		expect(top.querySelector('.stepname')?.textContent).toMatch(/\d\.\d/);
		// The spec line: surface, model and effort of the step.
		expect(top.textContent).toMatch(/Opus|Sonnet|Haiku/);
		const primary = top.querySelector<HTMLElement>('button.primary');
		expect(primary?.textContent).toMatch(/Copy and open|Run in terminal/);
		await vi.waitFor(() => expect(document.activeElement).toBe(primary));
	});

	it('AC-F017-02: ⌘⇧↵ runs the next step through the same preflight; blockers are shown and nothing launches', async () => {
		const m = fresh('happy');
		const start = vi.spyOn(m, 'launchStart');
		vi.spyOn(m, 'doNext').mockResolvedValue({
			kind: 'cli',
			project: 'p',
			step: '2.2',
			prepare: {
				kind: 'blocked',
				blockers: [{ kind: 'claudeMissing', path: '/opt/missing/claude', why: 'notExecutable' }],
				warnings: []
			}
		});
		const out = await doNext();
		expect(typeof out).toBe('string');
		expect(out).toMatch(/claude/i);
		expect(start).not.toHaveBeenCalled();
	});

	it('AC-F017-03: with no projects Today says "Start your first project" and the button opens the setup form', async () => {
		fresh('empty');
		const s = await render(Today);
		await expect
			.element(s.getByRole('heading', { name: 'Start your first project' }))
			.toBeVisible();
		const link = s.getByRole('link', { name: /Start your first project/ });
		await expect.element(link).toHaveAttribute('href', to.setup());
	});

	describe('AC-F017-04: a project with no runnable step says why', () => {
		const withRow = async (patch: (row: NextStepRow) => NextStepRow) => {
			const m = fresh('happy');
			const view = await m.todayGet({});
			vi.spyOn(m, 'todayGet').mockResolvedValue({
				...view,
				rows: view.rows.map((r, n) => (n === 0 ? patch(r) : r))
			});
			return render(Today);
		};
		const stripWith = (row: NextStepRow, status: SheetStatus) =>
			row.strip.map((c, i) => (i === (row.project.currentStage ?? 0) ? status : c));

		it('"Decide payments to continue" offers the stage, not a run', async () => {
			const s = await withRow((r) => ({
				...r,
				next: null,
				whyNone: 'Decide payments to continue'
			}));
			const top = await topRow(s.container);
			await vi.waitFor(() =>
				expect(top.querySelector('.stepname')?.textContent).toContain('Decide payments to continue')
			);
			expect(top.textContent).not.toMatch(/Sign the Stage/);
			expect(top.querySelector('button')?.textContent).toMatch(/Open stage/);
		});

		it('"Waiting until <date>"', async () => {
			const s = await withRow((r) => ({ ...r, next: null, whyNone: 'Waiting until 2026-11-02' }));
			const top = await topRow(s.container);
			await vi.waitFor(() =>
				expect(top.querySelector('.stepname')?.textContent).toContain('Waiting until 2026-11-02')
			);
		});

		it('a stage whose packet is waiting says to sign it', async () => {
			const s = await withRow((r) => ({
				...r,
				next: null,
				whyNone: 'Sign Stage 2 first: its steps are done and its packet isn’t signed.',
				strip: stripWith(r, 'waiting')
			}));
			const top = await topRow(s.container);
			await vi.waitFor(() => expect(top.textContent).toMatch(/Sign the Stage \d packet/));
			expect(top.querySelector('button')?.textContent).toMatch(/Review and sign/);
		});

		it('every step done', async () => {
			const s = await withRow((r) => ({
				...r,
				next: null,
				whyNone: 'Every step in this stage is done. Close out the stage next.'
			}));
			const top = await topRow(s.container);
			await vi.waitFor(() =>
				expect(top.querySelector('.stepname')?.textContent).toContain(
					'Every step in this stage is done'
				)
			);
		});
	});

	it('AC-F017-05: a project whose folder is missing shows "Project folder not found" and offers nothing runnable', async () => {
		fresh('readOnly');
		const s = await render(Today);
		const rows = () => [...s.container.querySelectorAll('ol.next li')];
		await vi.waitFor(() => expect(rows().length).toBeGreaterThan(1));
		const missing = rows().find((li) => li.textContent.includes('Old Notes'));
		expect(missing?.textContent).toContain('Project folder not found');
		expect(missing?.querySelector('button')).toBeNull();
		expect(missing?.textContent).toContain('Nothing to run');
	});
});
