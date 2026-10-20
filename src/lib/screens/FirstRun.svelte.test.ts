import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh } from '#lib/test/screen.ts';

import FirstRun from './FirstRun.svelte';

// M2-15 (F-016): first run against the mock.
describe('FirstRun', () => {
	it('primary: trusting the probe folder opens it in a terminal tab', async () => {
		const m = fresh('firstRun');
		const open = vi.spyOn(m, 'probeFolderOpen');
		const s = await render(FirstRun);
		await s.getByRole('button', { name: 'Open in terminal and trust' }).click();
		await vi.waitFor(() => expect(open).toHaveBeenCalled());
	});

	it('locked: the first project waits for Doctor', async () => {
		fresh('firstRun');
		const s = await render(FirstRun);
		await expect.element(s.getByText('Available after Doctor')).toBeVisible();
	});

	it('empty: no projects yet', async () => {
		fresh('firstRun');
		const s = await render(FirstRun);
		await expect.element(s.getByText(/No projects yet/)).toBeVisible();
	});

	it('error: a failed blueprint detection is announced, not thrown', async () => {
		const m = fresh('firstRun');
		failing(m, 'blueprintPickRepo');
		const s = await render(FirstRun);
		await expect.element(s.getByRole('alert')).toBeVisible();
	});
});
