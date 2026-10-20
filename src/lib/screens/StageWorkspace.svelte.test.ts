import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh, IDS, project, said } from '#lib/test/screen.ts';

import StageWorkspace from './StageWorkspace.svelte';

// M2-21 / M2-22 (F-004, F-005, F-006, F-030): the stage workspace against the mock.
describe('StageWorkspace', () => {
	it('primary: Copy prompt copies the resolved prompt of the selected step', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo: 2 });
		await s.getByRole('button', { name: 'Copy prompt' }).click();
		await vi.waitFor(() =>
			expect(m.getClipboard()).toContain('Recommend the best stack for TradeLog')
		);
	});

	it('locked: a step that waits for a running step says why next to Run', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo: 2 });
		await expect
			.element(s.getByText(/Waits for 2\.3: Claude is still working in this session/))
			.toBeVisible();
	});

	it('not applicable: the step says why, with the condition', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo: 2 });
		await expect.element(s.getByText(/TradeLog isn't sold.*commercial = no/)).toBeVisible();
	});

	it('config changed: Acknowledge records it', async () => {
		const m = fresh('happy');
		const ack = vi.spyOn(m, 'configAck');
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo: 2 });
		await s.getByRole('button', { name: 'Acknowledge' }).click();
		await vi.waitFor(() => expect(ack).toHaveBeenCalled());
	});

	it('error: a refused copy is reported, not thrown', async () => {
		const m = fresh('happy');
		failing(m, 'clipboardCopy');
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo: 2 });
		await s.getByRole('button', { name: 'Copy prompt' }).click();
		await vi.waitFor(() => expect(said()).not.toBe(''));
	});
});
