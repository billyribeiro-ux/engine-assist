import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { ui } from '#lib/stores/ui.svelte.ts';
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

	// M5-07: an added step's key (`x-2-1`) is wider than the callout circle; the circle shows its
	// number in the stage ("+1") and the title row still names the whole key for a screen reader.
	it('added step: the callout shows its display number and fits its circle', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo: 2 });
		const row = s.getByRole('listitem').filter({ hasText: 'Added for this project' });
		const callout = row.element().querySelector<HTMLElement>('.callout');
		expect(callout?.textContent).toBe('+1');
		const text = document.createRange();
		text.selectNodeContents(callout as HTMLElement);
		expect(text.getBoundingClientRect().width).toBeLessThan(callout?.clientWidth ?? 0);
		expect(row.element().querySelector('.vh')?.textContent).toContain('x‑2‑1');
		// A template step keeps its number.
		const plain = s.getByRole('listitem').filter({ hasText: 'Stack decision, evidence first' });
		expect(plain.element().querySelector('.callout')?.textContent).toBe('2.4');
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

	// AC-F018-08: after Create the stage opens with its first action focused, once.
	it('landing: the selected step’s primary action takes focus once after Create', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.tradelog);
		ui.landOnAction = true;
		const s = await render(StageWorkspace, { project: p, stageNo: 2 });
		await vi.waitFor(() => expect(ui.landOnAction).toBe(false));
		const focused = document.activeElement;
		expect(focused?.classList.contains('primary')).toBe(true);
		expect(focused?.closest('.actions')).not.toBeNull();
		await s.unmount();
		const again = await render(StageWorkspace, { project: p, stageNo: 2 });
		await expect.element(again.getByRole('button', { name: 'Copy prompt' })).toBeVisible();
		expect(document.activeElement?.closest('.actions')).toBeNull();
	});

	it('AC-F024-01: a desktop step opens in Desktop with the prompt copied and the chips named', async () => {
		const m = fresh('happy');
		const open = vi.spyOn(m, 'desktopOpen');
		const v = await m.addStep({
			projectId: IDS.tradelog,
			stage: 2,
			after: '2.4',
			copyConfigFrom: '2.4',
			title: 'Review the stack in Desktop',
			text: 'List what could break first.',
			config: {
				surface: 'desktop',
				cwd: 'spec',
				start: { model: 'opus', effort: 'high', mode: 'plan' },
				out: []
			},
			tag: 'optional'
		});
		const key =
			v.stages[2]?.steps.find((x) => x.title === 'Review the stack in Desktop')?.key ?? '';
		const s = await render(StageWorkspace, { project: v, stageNo: 2, stepKey: key });
		await s.getByRole('button', { name: /Open in Desktop/ }).click();
		await vi.waitFor(() =>
			expect(open).toHaveBeenCalledWith({ projectId: IDS.tradelog, step: key })
		);
		await vi.waitFor(() => expect(m.getClipboard()).toContain('List what could break first.'));
		await vi.waitFor(() =>
			expect(ui.toasts.map((t) => t.text).join('\n')).toContain(
				'Prompt copied and Claude Desktop opened. Paste it there and pick Opus 5.5, high effort, Plan.'
			)
		);
	});

	it('AC-F027-02: Export run sheet goes through the Rust save dialog; Cancel writes nothing', async () => {
		const m = fresh('happy');
		const exp = vi.spyOn(m, 'runSheetExport');
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo: 2 });
		const b = s.getByRole('button', { name: 'Export run sheet…' });
		await b.click();
		await vi.waitFor(() => expect(exp).toHaveBeenCalledWith({ projectId: IDS.tradelog, stage: 2 }));
		await vi.waitFor(() =>
			expect(ui.toasts.map((t) => t.text).join('\n')).toContain(
				'Run sheet exported to ~/Desktop/tradelog-stage-02-run-sheet.md'
			)
		);
		ui.toasts = [];
		m.setDialogAnswer('cancel');
		await b.click();
		await vi.waitFor(() => expect(exp).toHaveBeenCalledTimes(2));
		expect(ui.toasts).toEqual([]);
	});
});
