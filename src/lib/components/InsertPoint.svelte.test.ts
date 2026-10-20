import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';

import StageWorkspace from '#lib/screens/StageWorkspace.svelte';
import { failing, fresh, IDS, project, said } from '#lib/test/screen.ts';

import InsertPoint from './InsertPoint.svelte';

// M4-31 (AC-F025-01, AC-F025-03): add a prompt or an inspection item for this project only.
async function setup(stageNo = 2, afterKey = '2.4') {
	const m = fresh('happy');
	const p = await project(m, IDS.tradelog);
	const stage = p.stages.find((s) => s.stage === stageNo);
	const after = stage?.steps.find((s) => s.key === afterKey);
	if (!stage || !after) throw new Error('the happy world has this step');
	return { m, p, stage, after };
}

describe('InsertPoint', () => {
	it('primary: a prompt copies the picked step’s run config and is optional by default (AC-F025-01)', async () => {
		const { m, p, stage, after } = await setup();
		const add = vi.spyOn(m, 'addStep');
		const s = await render(InsertPoint, { project: p, stage, after });
		await s.getByRole('button', { name: /Add to Stage 2 after 2\.4/ }).click();
		await s.getByRole('menuitem', { name: /Add prompt/ }).click();
		await s.getByRole('textbox', { name: 'Title' }).fill('Re-rank pain');
		await s.getByRole('textbox', { name: 'Prompt' }).fill('Re-rank the pain map with my notes.');
		await s.getByRole('button', { name: 'Add' }).click();
		await vi.waitFor(() => expect(add).toHaveBeenCalled());
		const sent = add.mock.calls[0]?.[0];
		expect(sent).toMatchObject({
			projectId: IDS.tradelog,
			stage: 2,
			after: '2.4',
			copyConfigFrom: '2.4',
			title: 'Re-rank pain',
			text: 'Re-rank the pain map with my notes.',
			tag: 'optional'
		});
		expect(sent?.config.surface).toBe('cli');
		expect(sent?.config.cwd).not.toBeNull();
		expect(sent?.config.start.mode).not.toBeNull();
		expect(said()).toContain('Prompt added for this project');
	});

	it('the config can be edited and the step made core', async () => {
		const { m, p, stage, after } = await setup();
		const add = vi.spyOn(m, 'addStep');
		const s = await render(InsertPoint, { project: p, stage, after });
		await s.getByRole('button', { name: /Add to Stage 2 after 2\.4/ }).click();
		await s.getByRole('menuitem', { name: /Add prompt/ }).click();
		await s.getByRole('textbox', { name: 'Title' }).fill('Careful');
		await s.getByRole('textbox', { name: 'Prompt' }).fill('Review it.');
		await s.getByLabelText('Model').selectOptions('sonnet');
		await s.getByLabelText('Effort').selectOptions('medium');
		await s.getByLabelText('Mode').selectOptions('acceptEdits');
		await s.getByRole('checkbox', { name: /Make it core/ }).click();
		await s.getByRole('button', { name: 'Add' }).click();
		await vi.waitFor(() => expect(add).toHaveBeenCalled());
		expect(add.mock.calls[0]?.[0]).toMatchObject({
			tag: 'core',
			config: { start: { model: 'sonnet', effort: 'medium', mode: 'acceptEdits' } }
		});
	});

	it('picking another step to copy from reloads the config', async () => {
		const { m, p, stage, after } = await setup();
		const view = vi.spyOn(m, 'stepView');
		const s = await render(InsertPoint, { project: p, stage, after });
		await s.getByRole('button', { name: /Add to Stage 2 after 2\.4/ }).click();
		await s.getByRole('menuitem', { name: /Add prompt/ }).click();
		const pick = s.getByLabelText('Start from the run config of');
		await expect.element(pick).toBeVisible();
		const options = [...pick.element().querySelectorAll('option')];
		const other = options.find((o) => o.value !== '2.4');
		expect(other).toBeDefined();
		if (!other) return;
		await pick.selectOptions(other.value);
		await vi.waitFor(() =>
			expect(view).toHaveBeenCalledWith({ projectId: IDS.tradelog, step: other.value })
		);
	});

	it('an inspection item is a single line, added for this project (AC-F025-02)', async () => {
		const { m, p, stage, after } = await setup(5, '5.1');
		const add = vi.spyOn(m, 'addItem');
		const s = await render(InsertPoint, { project: p, stage, after });
		await s.getByRole('button', { name: /Add to Stage 5/ }).click();
		await s.getByRole('menuitem', { name: /Add inspection item/ }).click();
		await s.getByRole('textbox', { name: 'Inspection item' }).fill('Billing sandbox keys rotated');
		await s.getByRole('button', { name: 'Add' }).click();
		await vi.waitFor(() =>
			expect(add).toHaveBeenCalledWith({
				projectId: IDS.tradelog,
				stage: 5,
				text: 'Billing sandbox keys rotated'
			})
		);
		expect(said()).toContain('Inspection item added for this project');
	});

	it('locked: Add without a title says what is missing and sends nothing', async () => {
		const { m, p, stage, after } = await setup();
		const add = vi.spyOn(m, 'addStep');
		const s = await render(InsertPoint, { project: p, stage, after });
		await s.getByRole('button', { name: /Add to Stage 2 after 2\.4/ }).click();
		await s.getByRole('menuitem', { name: /Add prompt/ }).click();
		(s.getByRole('button', { name: 'Add' }).element() as HTMLElement).click();
		expect(add).not.toHaveBeenCalled();
		expect(said()).toContain('Fill in the title and the text first.');
	});

	it('error: bypassPermissions is refused by Rust and the sentence is shown in the dialog (AC-F025-05)', async () => {
		const { p, stage, after } = await setup();
		const s = await render(InsertPoint, { project: p, stage, after });
		await s.getByRole('button', { name: /Add to Stage 2 after 2\.4/ }).click();
		await s.getByRole('menuitem', { name: /Add prompt/ }).click();
		await s.getByRole('textbox', { name: 'Title' }).fill('Fast');
		await s.getByRole('textbox', { name: 'Prompt' }).fill('Run with bypassPermissions.');
		await s.getByRole('button', { name: 'Add' }).click();
		await expect.element(s.getByRole('alert')).toBeInTheDocument();
		expect(s.getByRole('alert').element().textContent).toContain(
			'bypassPermissions is never allowed'
		);
		await expect.element(s.getByRole('dialog')).toBeVisible();
	});

	it('error: a failed write is announced in the dialog', async () => {
		const { m, p, stage, after } = await setup();
		failing(m, 'addStep');
		const s = await render(InsertPoint, { project: p, stage, after });
		await s.getByRole('button', { name: /Add to Stage 2 after 2\.4/ }).click();
		await s.getByRole('menuitem', { name: /Add prompt/ }).click();
		await s.getByRole('textbox', { name: 'Title' }).fill('x');
		await s.getByRole('textbox', { name: 'Prompt' }).fill('y');
		await s.getByRole('button', { name: 'Add' }).click();
		await expect.element(s.getByRole('alert')).toBeVisible();
	});

	it('the menu is keyboard operable: ↓ moves, Esc closes and returns focus to the +', async () => {
		const { p, stage, after } = await setup();
		const s = await render(InsertPoint, { project: p, stage, after });
		const plus = s.getByRole('button', { name: /Add to Stage 2 after 2\.4/ });
		(plus.element() as HTMLElement).focus();
		await userEvent.keyboard('{Enter}');
		const items = s.getByRole('menuitem');
		await expect.element(items.first()).toHaveFocus();
		await userEvent.keyboard('{ArrowDown}');
		await expect.element(items.nth(1)).toHaveFocus();
		await userEvent.keyboard('{Escape}');
		await expect.element(s.getByRole('menu')).not.toBeInTheDocument();
		await expect.element(plus).toHaveFocus();
	});
});

describe('added step in the workspace (AC-F025-03)', () => {
	it('shows "Added for this project" and says it always confirms before running', async () => {
		const m = fresh('happy');
		await m.addStep({
			projectId: IDS.tradelog,
			stage: 2,
			after: '2.6',
			copyConfigFrom: '2.2',
			title: 'Name check',
			text: 'Check the names.',
			config: {
				surface: 'cli',
				cwd: 'spec',
				start: { model: 'opus', effort: 'high', mode: 'plan' },
				out: []
			},
			tag: 'optional'
		});
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo: 2 });
		await expect.element(s.getByText('Added for this project').first()).toBeInTheDocument();
		await expect.element(s.getByRole('heading', { name: /Name check/ })).toBeInTheDocument();
		// Its launch asks first: the plan carries the overlay-step reason.
		await m.configAck({ projectId: IDS.tradelog, cwd: 'spec', fingerprint: 'ab'.repeat(32) });
		const prep = await m.launchPrepare({
			projectId: IDS.tradelog,
			step: 'x-2-2',
			entry: 'stageView',
			mode: 'normal'
		});
		expect(prep.kind === 'ready' && prep.confirm.some((c) => c.kind === 'overlayStep')).toBe(true);
	});
});
