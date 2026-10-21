import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { handlerFor } from '#lib/keyboard/registry.ts';
import { PASTE_STAGE1 } from '#lib/ipc/mock/world.ts';
import { fresh, IDS, project, said } from '#lib/test/screen.ts';

import StageWorkspace from './StageWorkspace.svelte';

// M2-26 (AC-F028-01, AC-F028-03, AC-F007-08): the stage shortcuts invoke their actions over IPC and,
// where the action doesn't apply to the selection, return nothing but a one-line reason. The
// selected step of Stage 1 (scenario stage1) is 1.4, a chat step that writes the Stage 1 packet.
afterEach(() => vi.restoreAllMocks());

const open = async (scenario: 'stage1' | 'happy', stageNo: number) => {
	const m = fresh(scenario);
	const p = await project(m, IDS.tradelog);
	const s = await render(StageWorkspace, { project: p, stageNo });
	// The selected row has loaded its step (the shortcut's row API is registered).
	await vi.waitFor(() => {
		expect(s.container.querySelector('[aria-current="step"] .step-body .actions')).not.toBeNull();
	});
	return { m, s };
};

describe('StageWorkspace shortcuts (AC-F028, AC-F007-08)', () => {
	it('AC-F028-01: ⌘↵ runs the selected chat step over IPC (copy and open)', async () => {
		const { m } = await open('stage1', 1);
		const copy = vi.spyOn(m, 'chatCopyAndOpen');
		const run = handlerFor('run');
		expect(run).toBeDefined();
		expect(await run?.()).toBe(true);
		expect(copy).toHaveBeenCalledTimes(1);
		expect(copy.mock.calls[0]?.[0]).toMatchObject({ projectId: IDS.tradelog, step: '1.4' });
	});

	it('AC-F028-01: ⌘⇧V reads the clipboard in Rust and previews the write; nothing is written yet', async () => {
		const { m, s } = await open('stage1', 1);
		m.setClipboard(PASTE_STAGE1);
		const preview = vi.spyOn(m, 'pasteBackPreview');
		const apply = vi.spyOn(m, 'pasteBackApply');
		expect(await handlerFor('pasteBack')?.()).toBe(true);
		expect(preview).toHaveBeenCalledTimes(1);
		expect(preview.mock.calls[0]?.[0]).toMatchObject({ step: '1.4', source: 'clipboard' });
		expect(apply).not.toHaveBeenCalled();
		await expect.element(s.getByText('docs/handoff/stage-01.md')).toBeVisible();
	});

	it('AC-F007-08: a valid paste-back lands and the step is done', async () => {
		const { m, s } = await open('stage1', 1);
		m.setClipboard(PASTE_STAGE1);
		const apply = vi.spyOn(m, 'pasteBackApply');
		await handlerFor('pasteBack')?.();
		await s.getByRole('button', { name: /^Write \d files?/ }).click();
		await vi.waitFor(() => expect(apply).toHaveBeenCalledTimes(1));
		await vi.waitFor(() => expect(said()).toMatch(/step done/));
	});

	it('AC-F007-08 / AC-F028-01: ⌘D marks the selected chat step done over IPC', async () => {
		const { m } = await open('stage1', 1);
		const done = vi.spyOn(m, 'stepMarkDone');
		expect(await handlerFor('markDone')?.()).toBe(true);
		expect(done).toHaveBeenCalledTimes(1);
		expect(done.mock.calls[0]?.[0]).toMatchObject({ projectId: IDS.tradelog, step: '1.4' });
		expect(said()).toContain('marked done');
	});

	it('AC-F028-03: on a CLI step ⌘D and ⌘⇧V do nothing but give a one-line reason', async () => {
		const { m } = await open('happy', 2);
		// Stage 2's selected step is a CLI step.
		const done = vi.spyOn(m, 'stepMarkDone');
		const preview = vi.spyOn(m, 'pasteBackPreview');
		const reasonDone = await handlerFor('markDone')?.();
		const reasonPaste = await handlerFor('pasteBack')?.();
		expect(reasonDone).toBe('Mark done works on a chat step.');
		expect(reasonPaste).toBe('Paste back works on a chat step that writes a packet or an output.');
		expect(done).not.toHaveBeenCalled();
		expect(preview).not.toHaveBeenCalled();
	});

	it('AC-F028-03: a command copy on a chat step says only CLI steps have one', async () => {
		const { m } = await open('stage1', 1);
		const copy = vi.spyOn(m, 'clipboardCopy');
		expect(await handlerFor('copyCommand')?.()).toBe('Only CLI steps have a command.');
		expect(copy).not.toHaveBeenCalled();
	});
});
