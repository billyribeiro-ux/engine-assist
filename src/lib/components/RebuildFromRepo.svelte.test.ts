import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { fresh, IDS, said } from '#lib/test/screen.ts';

import RebuildFromRepo from './RebuildFromRepo.svelte';

// M3-16 (AC-F020-11, AC-F020-12): Rebuild from repo is reviewed first and written only on Rebuild.
describe('RebuildFromRepo', () => {
	it('primary: review lists the stages and what is lost; Rebuild applies it once', async () => {
		const m = fresh('happy');
		const apply = vi.spyOn(m, 'progressRebuildApply');
		const s = await render(RebuildFromRepo, { projectId: IDS.tradelog });
		await s.getByRole('button', { name: 'Review the rebuild' }).click();
		await expect.element(s.getByRole('heading', { name: 'What the rebuild finds' })).toHaveFocus();
		await expect.element(s.getByRole('list', { name: 'Stages after the rebuild' })).toBeVisible();
		expect(apply).not.toHaveBeenCalled();
		await s.getByRole('button', { name: 'Rebuild', exact: true }).click();
		await vi.waitFor(() => expect(apply).toHaveBeenCalledTimes(1));
		await vi.waitFor(() => expect(said()).toContain('Rebuilt from the repo'));
	});

	it('cancel: nothing is written', async () => {
		const m = fresh('happy');
		const apply = vi.spyOn(m, 'progressRebuildApply');
		const s = await render(RebuildFromRepo, { projectId: IDS.tradelog });
		await s.getByRole('button', { name: 'Review the rebuild' }).click();
		await s.getByRole('button', { name: 'Cancel' }).click();
		await expect.element(s.getByRole('button', { name: 'Review the rebuild' })).toBeVisible();
		expect(apply).not.toHaveBeenCalled();
	});
});
