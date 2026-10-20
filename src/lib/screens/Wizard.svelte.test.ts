import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh, IDS, project, said } from '#lib/test/screen.ts';

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
