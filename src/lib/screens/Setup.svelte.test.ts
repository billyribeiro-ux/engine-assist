import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh, IDS, project, said } from '#lib/test/screen.ts';

import { userEvent } from 'vitest/browser';

import Setup from './Setup.svelte';

// M2-14 (F-018): the single setup form against the mock.
describe('Setup', () => {
	it('primary: a complete form previews the files and creates the project', async () => {
		const m = fresh('happy');
		const apply = vi.spyOn(m, 'setupApply');
		const s = await render(Setup);
		await s.getByRole('textbox', { name: /^Name/ }).fill('Ledger');
		await s.getByRole('button', { name: 'Choose…' }).click();
		// The first folder is inside another Foreman project: Rust refuses it and says where to go.
		await expect.element(s.getByText(/inside another Foreman project/).first()).toBeVisible();
		await s.getByRole('button', { name: 'Choose…' }).click();
		await s.getByRole('textbox', { name: /Rough idea/ }).fill('A ledger for my receipts.');
		await s.getByRole('textbox', { name: /Who it's for/ }).fill('Me and my accountant.');
		await vi.waitFor(() => expect(s.container.textContent.match(/Fix \d+ fields? first[^.]*\./)?.[0]).toBeUndefined());
		await s.getByRole('button', { name: 'Create project' }).click();
		await vi.waitFor(() => expect(apply).toHaveBeenCalled());
	});

	it('locked: Create names the fields to fix first', async () => {
		fresh('happy');
		const s = await render(Setup);
		await expect.element(s.getByText("Fix 4 fields first: Name, Folder, Rough idea, Who it's for.")).toBeVisible();
	});

	it('empty: no folder yet, so no file preview', async () => {
		fresh('happy');
		const s = await render(Setup);
		await expect.element(s.getByText(/Choose the folder to see the exact files/)).toBeVisible();
	});

	it('error: a placeholder is not an answer', async () => {
		fresh('happy');
		const s = await render(Setup);
		const who = s.getByRole('textbox', { name: /Who it's for/ });
		await who.fill("<who it's for>");
		await userEvent.tab();
		await expect.element(s.getByText(/Describe who it's for in your words/)).toBeVisible();
	});
});
