import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh } from '#lib/test/screen.ts';

import FirstRun from './FirstRun.svelte';

// M2-16 acceptance criteria against the mock (comp): AC-F019-01, AC-F019-02, AC-F001-18.
const titles = (c: Element) => [...c.querySelectorAll('.fr-t')].map((e) => e.textContent.trim());

describe('FirstRun (AC-F019, AC-F001-18)', () => {
	it('AC-F019-01: blueprint repo, template check, your name, Doctor, then Start your first project', async () => {
		fresh('firstRun');
		const s = await render(FirstRun);
		expect(titles(s.container)).toEqual([
			expect.stringMatching(/blueprint repo/i),
			'Template health',
			'Your name for signatures',
			'Doctor',
			'Start your first project'
		]);
		// 1: auto-detected from the fixed candidates; 2: checked.
		await expect.element(s.getByText(/Found ~\/claude-blueprint/)).toBeVisible();
		// 3: the name comes prefilled from git config user.name; Change edits it.
		await expect.element(s.getByText(/Billy Ribeiro, from git config user\.name/)).toBeVisible();
		await s.getByRole('button', { name: 'Change' }).click();
		const name = s.getByLabelText('Name');
		await expect.element(name).toHaveValue('Billy Ribeiro');
		await s.getByRole('button', { name: 'Save name' }).click();
		// 4: Doctor's checks, then the probe-folder trust (skippable).
		await expect.element(s.getByRole('list', { name: 'Doctor checks' })).toBeVisible();
		await expect.element(s.getByText('Available after Doctor')).toBeVisible();
		await s.getByRole('button', { name: 'Skip for now' }).click();
		// 5: the first project is now the current step.
		await vi.waitFor(() => {
			const now = s.container.querySelector('[aria-current="step"]');
			expect(now?.textContent).toContain('Start your first project');
		});
		await expect.element(s.getByText('Available after Doctor')).not.toBeInTheDocument();
	});

	it('AC-F019-02: the template check shows the blueprint version and the parse diagnostics', async () => {
		fresh('firstRun');
		const s = await render(FirstRun);
		await expect.element(s.getByText(/Blueprint 2\.2 at /)).toBeVisible();
		await expect.element(s.getByText(/Stages .* runnable\./)).toBeVisible();
		await expect.element(s.getByText(/1 errors?, 2 warnings?/)).toBeVisible();
		const diags = s.container.querySelector('details.diags');
		expect(diags?.querySelector('summary')?.textContent).toMatch(/Show the \d+ diagnostics/);
		expect(diags?.querySelectorAll('li').length).toBeGreaterThan(0);
	});

	it('AC-F001-18: a missing or unreadable blueprint repo asks to "Choose your blueprint repo"', async () => {
		const m = fresh('firstRun');
		failing(m, 'blueprintPickRepo');
		const s = await render(FirstRun);
		await expect.element(s.getByText('Choose your blueprint repo')).toBeVisible();
		await expect.element(s.getByRole('button', { name: 'Choose…' })).toBeVisible();
		await expect.element(s.getByRole('alert')).toBeVisible();
	});
});
