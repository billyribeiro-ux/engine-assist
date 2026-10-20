import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh, IDS, project } from '#lib/test/screen.ts';

import Gate from './Gate.svelte';

// M4-06 / M4-17 (F-011, F-012, F-013): the gate against the mock.
describe('Gate', () => {
	it('primary: Sign packet previews the signature from Rust', async () => {
		const m = fresh('happy');
		const prev = vi.spyOn(m, 'signPreview');
		const p = await project(m, IDS.swings);
		const s = await render(Gate, { project: p, stageNo: 7 });
		await s.getByRole('button', { name: 'Sign packet' }).click();
		await vi.waitFor(() => expect(prev).toHaveBeenCalled());
		await expect.element(s.getByRole('dialog')).toBeVisible();
	});

	it('empty: no lessons yet says where they come from', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.swings);
		const s = await render(Gate, { project: p, stageNo: 7 });
		await expect.element(s.getByText('No lessons for Stage 7 yet')).toBeVisible();
	});

	it('locked: a stage still in progress cannot be signed', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.tradelog);
		const s = await render(Gate, { project: p, stageNo: 2 });
		const sign = s.getByRole('button', { name: 'Sign packet' });
		await expect.element(sign).toBeVisible();
		await expect.element(sign).toHaveAttribute('aria-disabled', 'true');
	});

	it('error: a failed load is announced', async () => {
		const m = fresh('happy');
		failing(m, 'gateView');
		const p = await project(m, IDS.swings);
		const s = await render(Gate, { project: p, stageNo: 7 });
		await expect.element(s.getByRole('alert')).toBeVisible();
	});
});
