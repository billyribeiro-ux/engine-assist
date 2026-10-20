import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh } from '#lib/test/screen.ts';

import Doctor from './Doctor.svelte';

// M2-15 (F-019): Doctor against the mock.
describe('Doctor', () => {
	it('primary: Check again runs the checks again', async () => {
		const m = fresh('happy');
		const run = vi.spyOn(m, 'doctorRun');
		const s = await render(Doctor);
		await expect.element(s.getByRole('heading', { name: 'Claude Code' })).toBeVisible();
		await s.getByRole('button', { name: 'Check again' }).click();
		await vi.waitFor(() => expect(run.mock.calls.length).toBeGreaterThanOrEqual(2));
	});

	it('locked: an old Claude Code names the steps it blocks', async () => {
		fresh('happy');
		const s = await render(Doctor);
		await expect.element(s.getByText('Stage 5 Gate A (test-runner)')).toBeVisible();
		await expect.element(s.getByText(/needs 2\.1\.293 or later/)).toBeVisible();
	});

	it('empty: an uncaptured login PATH offers the capture', async () => {
		fresh('happy');
		const s = await render(Doctor);
		await expect
			.element(s.getByRole('button', { name: 'Capture my login shell PATH' }))
			.toBeVisible();
	});

	it('primary: Run probes now runs the contract probes', async () => {
		const m = fresh('happy');
		const probe = vi.spyOn(m, 'probeRun');
		const s = await render(Doctor);
		await s.getByRole('button', { name: 'Run probes now' }).click();
		await vi.waitFor(() => expect(probe).toHaveBeenCalledTimes(1));
	});

	it('primary: Open probe folder opens Claude Code there in a terminal tab', async () => {
		const m = fresh('happy');
		const open = vi.spyOn(m, 'probeFolderOpen');
		const s = await render(Doctor);
		await s.getByRole('button', { name: 'Open probe folder' }).click();
		await vi.waitFor(() => expect(open).toHaveBeenCalledTimes(1));
	});

	it('error: a failed run is announced', async () => {
		const m = fresh('happy');
		failing(m, 'doctorRun');
		const s = await render(Doctor);
		await expect.element(s.getByRole('alert')).toBeVisible();
	});
});
