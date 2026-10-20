import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh } from '#lib/test/screen.ts';

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

	it('error: a failed load is announced', async () => {
		const m = fresh('happy');
		failing(m, 'templateHealth');
		const s = await render(TemplateHealth);
		await expect.element(s.getByRole('alert')).toBeVisible();
	});
});
