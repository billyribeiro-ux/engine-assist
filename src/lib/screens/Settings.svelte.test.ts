import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh } from '#lib/test/screen.ts';

import Settings from './Settings.svelte';

// M2-16 / M4-20 (F-023): settings against the mock.
describe('Settings', () => {
	it('primary: choosing a theme saves it', async () => {
		const m = fresh('happy');
		const upd = vi.spyOn(m, 'settingsUpdate');
		const s = await render(Settings);
		await s.getByRole('radio', { name: 'Vellum' }).click();
		await vi.waitFor(() => expect(upd).toHaveBeenCalled());
		expect(JSON.stringify(upd.mock.calls[0])).toContain('vellum');
	});

	it('locked: clearing run history needs the native dialog; Cancel clears nothing', async () => {
		const m = fresh('happy');
		m.setDialogAnswer('cancel');
		const clear = vi.spyOn(m, 'runsClear');
		const s = await render(Settings);
		await s.getByRole('button', { name: 'Clear run history…' }).click();
		await vi.waitFor(() => expect(clear).toHaveBeenCalled());
		await expect.element(s.getByText(/212 runs/)).toBeVisible();
	});

	it('error: a failed load is announced', async () => {
		const m = fresh('happy');
		failing(m, 'settingsGet');
		const s = await render(Settings);
		await expect.element(s.getByRole('alert')).toBeVisible();
	});
});
