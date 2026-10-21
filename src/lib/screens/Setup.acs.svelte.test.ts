import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { fresh } from '#lib/test/screen.ts';

import Setup from './Setup.svelte';

// M2-18 (AC-F018-01) and the form's side of AC-F018-02, against the mock.
describe('Setup form (AC-F018)', () => {
	it('AC-F018-01: asks for name, folder, variables, the eight early conditions, the Project link and shows the blueprint pin', async () => {
		fresh('happy');
		const s = await render(Setup);
		await expect.element(s.getByRole('textbox', { name: /^Name/ })).toBeVisible();
		await expect.element(s.getByLabelText('Folder')).toBeVisible();
		await expect.element(s.getByLabelText('claude.ai Project link')).toBeVisible();
		// Variables: each labelled with its token.
		await expect.element(s.getByRole('textbox', { name: /Rough idea/ })).toBeVisible();
		await expect.element(s.getByRole('textbox', { name: /Who it's for/ })).toBeVisible();
		// The eight early conditions, each tri-state.
		await vi.waitFor(() => expect(s.container.querySelectorAll('.cond').length).toBe(8));
		for (const c of s.container.querySelectorAll('.cond')) {
			expect(c.querySelectorAll('[role="radio"], input[type="radio"]').length).toBe(3);
		}
		const text = s.container.textContent;
		expect(text).not.toMatch(/has_database|has_uploads|has_email/);
		// The blueprint pin appears once a folder is chosen.
		await s.getByRole('button', { name: 'Choose…' }).click();
		await s.getByRole('button', { name: 'Choose…' }).click();
		await expect.element(s.getByText(/2\.2 at [0-9a-f]{6,}, clean working tree/)).toBeVisible();
	});

	it('AC-F018-02: the review lists what is created, left alone and refused', async () => {
		fresh('happy');
		const s = await render(Setup);
		await s.getByRole('button', { name: 'Choose…' }).click();
		await s.getByRole('button', { name: 'Choose…' }).click();
		await vi.waitFor(() => {
			const groups = [...s.container.querySelectorAll('.group-label')].map((e) => e.textContent);
			expect(groups).toContain('Create');
			expect(groups).toContain('Leave alone');
			expect(groups).toContain('Refuse to overwrite');
		});
	});
});
