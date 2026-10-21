import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { PASTE_STAGE1 } from '#lib/ipc/mock/world.ts';
import { fresh, IDS, project, said } from '#lib/test/screen.ts';
import StageWorkspace from '#lib/screens/StageWorkspace.svelte';

import ChatChips from './ChatChips.svelte';
import CloseOutChecklist from './CloseOutChecklist.svelte';
import PasteBack from './PasteBack.svelte';

// M3-23, M3-24, M3-25 (F-012, F-015 P1): chat chips, paste-back and the close-out checklist.
describe('ChatChips', () => {
	it('AC-F012-01: chips are checkable reminders; the session chip is bold', async () => {
		const s = await render(ChatChips, {
			chips: {
				chips: [
					{ id: 'session', label: 'Same chat as 1.1 The architect interview', strong: true },
					{ id: 'model', label: 'Opus 5.5', strong: false },
					{ id: 'research', label: 'Research ON', strong: false }
				],
				link: null,
				linkLabel: null
			}
		});
		const session = s.getByRole('checkbox', { name: /Same chat as 1\.1/ });
		await expect.element(session).toHaveClass(/strong/);
		await expect.element(session).toHaveAttribute('aria-checked', 'false');
		await session.click();
		await expect.element(session).toHaveAttribute('aria-checked', 'true');
		await expect.element(s.getByRole('checkbox', { name: 'Research ON' })).toBeVisible();
		await expect.element(s.getByText(/Reminders only/)).toBeVisible();
	});
});

describe('PasteBack', () => {
	it('AC-F012-10: the wrong stage says which stage, and nothing is offered to write', async () => {
		const m = fresh('stage1');
		m.setClipboard(PASTE_STAGE1.replace('Stage 1 → Stage 2', 'Stage 3 → Stage 4'));
		const s = await render(PasteBack, {
			projectId: IDS.tradelog,
			step: '1.4',
			stage: 1,
			packet: true
		});
		await s.getByRole('button', { name: /Paste back/ }).click();
		await expect
			.element(s.getByText('This is the Stage 3 packet; this step expects Stage 1.'))
			.toBeVisible();
		await expect.element(s.getByRole('button', { name: /^Write/ })).not.toBeInTheDocument();
	});

	it('AC-F012-06: every write is listed before anything is written', async () => {
		const m = fresh('stage1');
		m.setClipboard(PASTE_STAGE1);
		const apply = vi.spyOn(m, 'pasteBackApply');
		const s = await render(PasteBack, {
			projectId: IDS.tradelog,
			step: '1.4',
			stage: 1,
			packet: true
		});
		await s.getByRole('button', { name: /Paste back/ }).click();
		await expect.element(s.getByText('docs/handoff/stage-01.md')).toBeVisible();
		await expect.element(s.getByText('docs/handoff/stage-01-attachments.md')).toBeVisible();
		expect(apply).not.toHaveBeenCalled();
		await s.getByRole('button', { name: /^Write \d files?/ }).click();
		await vi.waitFor(() => expect(apply).toHaveBeenCalled());
		await vi.waitFor(() => expect(said()).toMatch(/Wrote \d files?; step done/));
	});

	it('AC-F012-14: a step that appends offers no write', async () => {
		const m = fresh('stage1');
		m.setClipboard('## L-001 · Stage 1\nKeep packets short.');
		const s = await render(PasteBack, {
			projectId: IDS.tradelog,
			step: '1:F.2',
			stage: 1,
			packet: false
		});
		await s.getByRole('button', { name: /Paste back/ }).click();
		await expect
			.element(s.getByText(/add the entries to docs\/lessons\.md yourself/))
			.toBeVisible();
	});

	it('AC-F016-02: an F.2 step pastes its lessons back as an append to docs/lessons.md', async () => {
		const m = fresh('stage1');
		m.setClipboard('## L-001 · Stage 1 · 2026-10-20 · Packets ran long\nSTATUS: open');
		const preview = vi.spyOn(m, 'pasteBackPreview');
		const s = await render(PasteBack, {
			projectId: IDS.tradelog,
			step: '1:F.2',
			stage: 1,
			packet: false,
			kind: 'lessons'
		});
		await s.getByRole('button', { name: /Paste back/ }).click();
		expect(preview).toHaveBeenCalledWith(expect.objectContaining({ kind: 'lessons' }));
		await expect.element(s.getByText('docs/lessons.md')).toBeVisible();
		await expect.element(s.getByRole('button', { name: /^Write 1 file/ })).toBeVisible();
	});
});

describe('CloseOutChecklist', () => {
	it('AC-F015-01: a manual step lists the sequence; each line is a checkbox', async () => {
		const s = await render(CloseOutChecklist, {
			stage: 1,
			items: ['Packet pasted back and valid', 'Attest the inspection items', 'F.2 lessons', 'Sign']
		});
		await expect.element(s.getByRole('heading', { name: 'Close out Stage 1' })).toBeVisible();
		const first = s.getByRole('checkbox', { name: 'Packet pasted back and valid' });
		await first.click();
		await expect.element(first).toHaveAttribute('aria-checked', 'true');
		await expect.element(s.getByText('Manual · 1 of 4')).toBeVisible();
	});

	it('AC-F015-01: the stage shows it once its packet is valid', async () => {
		const m = fresh('stage1');
		m.setClipboard(PASTE_STAGE1);
		const pv = await m.pasteBackPreview({
			projectId: IDS.tradelog,
			step: '1.4',
			kind: 'auto',
			source: 'clipboard'
		});
		await m.pasteBackApply({ previewId: pv.previewId });
		const p = await project(m, IDS.tradelog);
		const s = await render(StageWorkspace, { project: p, stageNo: 1 });
		await expect.element(s.getByRole('heading', { name: 'Close out Stage 1' })).toBeVisible();
		await expect.element(s.getByText(/F\.2 lessons in the same chat/)).toBeVisible();
	});
});
