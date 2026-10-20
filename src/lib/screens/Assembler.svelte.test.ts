import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh, IDS, project } from '#lib/test/screen.ts';

import Assembler from './Assembler.svelte';

// M4-26 (F-032): the assembler against the mock.
describe('Assembler', () => {
	it('primary: a signed Stage 4 packet lists the repo files', async () => {
		const m = fresh('stage4');
		const p = await project(m, IDS.tradelog);
		const s = await render(Assembler, { project: p });
		await expect.element(s.getByRole('heading', { name: 'Repo files' })).toBeVisible();
	});

	it('locked: without a Stage 4 packet it says what to run first', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.tradelog);
		const s = await render(Assembler, { project: p });
		await expect.element(s.getByText('No build package yet')).toBeVisible();
		await expect
			.element(
				s
					.getByRole('link', { name: 'Open Stage 4' })
					.or(s.getByRole('button', { name: 'Open Stage 4' }))
			)
			.toBeVisible();
	});

	it('error: a failed load is shown', async () => {
		const m = fresh('stage4');
		failing(m, 'assemblerView');
		const p = await project(m, IDS.tradelog);
		const s = await render(Assembler, { project: p });
		await expect.element(s.getByRole('alert')).toBeVisible();
	});
});
