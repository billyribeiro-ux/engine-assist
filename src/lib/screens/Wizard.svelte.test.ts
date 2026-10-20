import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { goto } from '$app/navigation';
import { fresh } from '#lib/test/screen.ts';
import { ui } from '#lib/stores/ui.svelte.ts';

import Wizard from './Wizard.svelte';

// M4-03 (F-018 P2): the wizard shares the form and its checks.
describe('Wizard', () => {
	it('primary: Continue moves to Variables once name and folder are set', async () => {
		fresh('happy');
		const s = await render(Wizard);
		await s.getByRole('textbox', { name: /^Name/ }).fill('Ledger');
		await s.getByRole('button', { name: 'Choose…' }).click();
		await s.getByRole('button', { name: 'Choose…' }).click();
		await s.getByRole('button', { name: /Continue/ }).click();
		await expect.element(s.getByRole('textbox', { name: /Rough idea/ })).toBeVisible();
	});

	it('locked: Continue without a name says what to fix', async () => {
		fresh('happy');
		const s = await render(Wizard);
		await s.getByRole('button', { name: /Continue/ }).click();
		await expect.element(s.getByText('Name the project.')).toBeVisible();
	});
});

// M4-26 (AC-F018-08): four steps, defaults from Settings, one line per condition, the reviewed
// path with the file writes, CR-003 errors, and Create landing on the first action.
describe('Wizard steps', () => {
	async function through(s: Awaited<ReturnType<typeof render>>, upTo: 1 | 2 | 3) {
		await s.getByRole('textbox', { name: /^Name/ }).fill('Ledger');
		await s.getByRole('button', { name: 'Choose…' }).click();
		await s.getByRole('button', { name: 'Choose…' }).click();
		await s.getByRole('button', { name: /Continue/ }).click();
		await expect.element(s.getByRole('heading', { name: 'Variables' })).toBeVisible();
		if (upTo === 1) return;
		await s.getByRole('textbox', { name: /Rough idea/ }).fill('A ledger for my receipts.');
		await s.getByRole('textbox', { name: /Who it's for/ }).fill('Me and my accountant.');
		await s.getByRole('button', { name: /Continue/ }).click();
		await expect.element(s.getByRole('heading', { name: 'Conditions' })).toBeVisible();
		if (upTo === 2) return;
		await s.getByRole('button', { name: /Continue/ }).click();
	}

	it('four steps, the current one marked, and the finished ones spoken as done', async () => {
		fresh('happy');
		const s = await render(Wizard);
		const steps = s.getByRole('navigation', { name: 'Wizard steps' });
		const items = [...steps.element().querySelectorAll('li')];
		expect(items.map((li) => li.textContent.replace(/\s+/g, ' ').trim())).toEqual([
			'1 Name and folder',
			'2 Variables',
			'3 Conditions',
			'4 Review your path'
		]);
		expect(items[0]?.getAttribute('aria-current')).toBe('step');
		await through(s, 1);
		const after = [...steps.element().querySelectorAll('li')];
		expect(after[1]?.getAttribute('aria-current')).toBe('step');
		expect(after[0]?.textContent).toContain('done');
	});

	it('variables are prefilled from Settings’ global defaults and say so', async () => {
		fresh('happy');
		const s = await render(Wizard);
		await through(s, 1);
		const stack = s.getByRole('textbox', { name: /Preferred stack/i });
		await expect.element(stack).toHaveValue('SvelteKit + Rust/Axum + Postgres');
		await expect.element(s.getByText('From your defaults in Settings').first()).toBeVisible();
	});

	it('conditions: every one has a line on what it changes and a three-way choice', async () => {
		fresh('happy');
		const s = await render(Wizard);
		await through(s, 2);
		const conds = [...s.container.querySelectorAll('.cond')];
		expect(conds).toHaveLength(8);
		for (const c of conds) {
			expect(c.querySelector('p')?.textContent.trim().length, c.textContent).toBeGreaterThan(10);
			expect(c.querySelectorAll('[role="radio"]')).toHaveLength(3);
		}
	});

	it('review: which stages apply and why, and the writes (create, leave alone, refuse)', async () => {
		fresh('happy');
		const s = await render(Wizard);
		await through(s, 3);
		await expect.element(s.getByRole('heading', { name: /Review Ledger's path/ })).toBeVisible();
		const path = s.getByRole('list', { name: 'Stages and whether they apply' });
		await expect.element(path).toBeVisible();
		expect(path.element().textContent).toMatch(/Applies/);
		expect(path.element().textContent).toMatch(/Not applicable|Decide payments/);
		for (const h of ['Create', 'Leave alone', 'Refuse to overwrite'])
			await expect.element(s.getByRole('heading', { name: h })).toBeVisible();
		await expect.element(s.getByText(/Creates \d+ paths and pins blueprint/)).toBeVisible();
	});

	it('Create lands on the project and asks the stage to focus its first action', async () => {
		const m = fresh('happy');
		const apply = vi.spyOn(m, 'setupApply');
		const s = await render(Wizard);
		await through(s, 3);
		await s.getByRole('button', { name: /Create project/ }).click();
		await vi.waitFor(() => expect(apply).toHaveBeenCalled());
		await vi.waitFor(() =>
			expect(goto).toHaveBeenCalledWith(expect.stringContaining('/projects/'))
		);
		expect(ui.landOnAction).toBe(true);
	});

	it('error: annotation errors (CR-003) stop the review and are each listed', async () => {
		fresh('annotationErrors');
		const s = await render(Wizard);
		await through(s, 3);
		await expect.element(s.getByRole('list', { name: 'Annotation errors' })).toBeVisible();
		await expect.element(s.getByText('Stage 1 step 1.2, line 141')).toBeVisible();
	});

	it('Back returns to the earlier step and keeps what was typed', async () => {
		fresh('happy');
		const s = await render(Wizard);
		await through(s, 1);
		await s.getByRole('button', { name: 'Back' }).click();
		await expect.element(s.getByRole('textbox', { name: /^Name/ })).toHaveValue('Ledger');
	});
});
