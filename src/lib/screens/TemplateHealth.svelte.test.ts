import { describe, expect, it, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import { failing, fresh } from '#lib/test/screen.ts';
import type { TemplateHealth as Health } from '#lib/ipc/types.ts';

import TemplateHealth from './TemplateHealth.svelte';

// M4-26 (F-031): template health against the mock.
describe('TemplateHealth', () => {
	it('primary: shows counts, stages and each diagnostic with its line', async () => {
		fresh('happy');
		const s = await render(TemplateHealth);
		await expect.element(s.getByRole('table')).toBeVisible();
		await expect.element(s.getByText(/unknown key agent= in fm:step/)).toBeVisible();
	});

	it('not applicable: unannotated stages say they need annotation', async () => {
		fresh('happy');
		const s = await render(TemplateHealth);
		await expect.element(s.getByRole('cell', { name: 'Needs annotation' })).toBeVisible();
		await expect.element(s.getByText('No errors or warnings.')).toBeVisible();
	});

	it('AC-F003-05: each runnable step shows its annotation, lint warnings and outputs', async () => {
		fresh('happy');
		const s = await render(TemplateHealth);
		const stage2 = s.getByRole('button', { name: /Stage 2/ });
		await expect.element(stage2).toHaveAttribute('aria-expanded', 'false');
		await expect.element(s.getByText('Convene the council')).not.toBeVisible();
		await stage2.click();
		await expect.element(stage2).toHaveAttribute('aria-expanded', 'true');
		await expect.element(s.getByText('Convene the council')).toBeVisible();
		await expect
			.element(s.getByText(/fm:step id=2\.2 surface=cli cwd=spec start=opus@xhigh\/plan/))
			.toBeVisible();
		await expect.element(s.getByText('spec/council/r1-research.md')).toBeVisible();
		await expect.element(s.getByText('spec/council/r2-options.md')).toBeVisible();
		// A step's lint warning sits with the step; a step without outputs says so.
		const stage5 = s.getByRole('button', { name: /Stage 5.*with warnings/ });
		await stage5.click();
		const list = s.getByRole('list').filter({ hasText: 'Scaffold the workspace' }).first();
		await expect.element(list.getByText(/RUN WITH says "same session"/)).toBeVisible();
		await expect.element(list.getByText('None declared')).toBeVisible();
		// Keyboard: the disclosure is a button reached by Tab and toggled by Enter.
		stage5.element().focus();
		await userEvent.keyboard('{Enter}');
		await expect.element(stage5).toHaveAttribute('aria-expanded', 'false');
	});

	it('AC-F003-06: a clean template says so instead of showing empty groups', async () => {
		const m = fresh('happy');
		const base = await m.templateHealth({ projectId: null });
		const clean: Health = {
			...base,
			counts: { ...base.counts, errors: 0, warnings: 0 },
			stages: base.stages.map((x) =>
				x.status === 'needsAnnotation' ? x : { ...x, status: 'runnable', text: 'Runnable' }
			),
			diagnostics: [],
			steps: base.steps.map((x) => ({ ...x, warnings: [] }))
		};
		vi.spyOn(m, 'templateHealth').mockResolvedValue(clean);
		const s = await render(TemplateHealth);
		await expect.element(s.getByText('No errors or warnings.')).toBeVisible();
		await expect.element(s.getByRole('heading', { name: /^Stage \d$/ })).not.toBeInTheDocument();
		await expect.element(s.getByRole('heading', { name: /^Stages \d/ })).not.toBeInTheDocument();
	});

	it('loading: says it is reading the blueprint', async () => {
		const m = fresh('happy');
		// A load that never answers.
		vi.spyOn(m, 'templateHealth').mockReturnValue(new Promise(() => undefined));
		const s = await render(TemplateHealth);
		await expect.element(s.getByText(/Reading the blueprint/)).toBeVisible();
		await expect.element(s.getByText(/Reading the blueprint/)).toHaveAttribute('role', 'status');
	});

	it('error: a failed load is announced', async () => {
		const m = fresh('happy');
		failing(m, 'templateHealth');
		const s = await render(TemplateHealth);
		await expect.element(s.getByRole('alert')).toBeVisible();
	});
});
