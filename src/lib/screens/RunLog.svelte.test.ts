import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh } from '#lib/test/screen.ts';

import RunLog from './RunLog.svelte';

// M3-21 / M4-28 (F-026): the run log against the mock.
describe('RunLog', () => {
	it('primary: Show command reveals the exact argv', async () => {
		fresh('happy');
		const s = await render(RunLog);
		const show = s.getByRole('button', { name: 'Show command' }).first();
		await show.click();
		await expect
			.element(s.getByRole('button', { name: 'Hide command' }))
			.toHaveAttribute('aria-expanded', 'true');
		expect(s.container.querySelector('.argv-row pre')?.textContent).toMatch(/claude/);
	});

	it('metrics: time to start per surface and p95 against the target', async () => {
		fresh('happy');
		const s = await render(RunLog);
		await expect.element(s.getByText('p95, all steps (target 10 s)')).toBeVisible();
	});

	it('metrics (AC-F026-09): active time per step, stages run, edits by stage and the baseline', async () => {
		fresh('happy');
		const s = await render(RunLog);
		await expect.element(s.getByText('Median active time per step, CLI')).toBeVisible();
		await expect.element(s.getByText('23.0 min')).toBeVisible();
		await expect.element(s.getByText('16.0 min')).toBeVisible();
		await expect.element(s.getByText('Stages run through Foreman')).toBeVisible();
		await expect.element(s.getByText('6.2 s vs 94.0 s')).toBeVisible();
		await expect
			.element(s.getByText('Manual prompt edits by stage: Stage 1 · 0, Stage 2 · 0'))
			.toBeVisible();
		await expect.element(s.getByText('Median time to start, CLI (p95 9.1 s)')).toBeVisible();
	});

	it('empty: a search with no match says so', async () => {
		fresh('happy');
		const s = await render(RunLog);
		await s.getByRole('textbox', { name: 'Search runs' }).fill('zzzz-nothing');
		await expect.element(s.getByText('No runs match')).toBeVisible();
	});

	it('not applicable: a run whose transcript is gone offers a fresh chain instead of Resume', async () => {
		fresh('happy');
		const s = await render(RunLog);
		await s.getByRole('radio', { name: 'Interrupted' }).click();
		await expect.element(s.getByText('Transcript removed after 30 days')).toBeVisible();
		await expect
			.element(s.getByRole('button', { name: 'Start a fresh chain with context' }))
			.toBeVisible();
		expect(s.container.querySelectorAll('tbody tr').length).toBe(1);
	});

	it('error: a failed search is announced', async () => {
		const m = fresh('happy');
		failing(m, 'runsSearch');
		const s = await render(RunLog);
		await expect.element(s.getByRole('alert')).toBeVisible();
	});
});
