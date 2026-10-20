import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh, IDS, project, said } from '#lib/test/screen.ts';

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

	it('AC-F023-05: "the package is complete" is not offered while a row is missing or empty', async () => {
		const m = fresh('stage4');
		const p = await project(m, IDS.tradelog);
		const s = await render(Assembler, { project: p });
		await expect.element(s.getByText(/isn't offered yet/)).toBeVisible();
		await expect.element(s.getByText('The package is complete.')).not.toBeInTheDocument();
		// The waiting rows are named; the refused row doesn't block.
		await expect.element(s.getByText(/The refused row doesn't block it/)).toBeVisible();
		const waiting = s.getByText(/Waiting for:/);
		await expect.element(waiting).toBeVisible();
		const text = waiting.element().textContent.replace(/\s+/g, ' ');
		expect(text).toContain('docs/tickets.md (empty)');
		expect(text).toContain('docs/threat-model.md (missing)');
		// Fill every missing and empty row: complete.
		m.world.clipboard = '# filled';
		for (const path of ['docs/tickets.md', 'docs/threat-model.md', 'docs/design/tokens.css']) {
			const pv = await m.assemblerPastePreview({ projectId: IDS.tradelog, path });
			await m.assemblerPasteApply({ previewId: pv.previewId });
		}
		const done = await render(Assembler, { project: await project(m, IDS.tradelog) });
		await expect.element(done.getByText('The package is complete.').first()).toBeVisible();
	});

	it('AC-F021-06: root CLAUDE.md shows the diff as context and has no approve path of its own', async () => {
		const m = fresh('stage4');
		m.world.clipboard = '# TradeLog\nNever float money.';
		const apply = vi.spyOn(m, 'assemblerPasteApply');
		const p = await project(m, IDS.tradelog);
		const s = await render(Assembler, { project: p });
		await s.getByRole('button', { name: 'Paste into CLAUDE.md' }).click();
		const dialog = s.getByRole('dialog', { name: 'Paste into CLAUDE.md' });
		await expect.element(dialog).toBeVisible();
		await expect.element(dialog.getByText(/Never float money/).first()).toBeVisible();
		await expect
			.element(dialog.getByText(/Claude Code loads this file in every session/))
			.toBeVisible();
		// The webview's button only requests the write; Rust asks (here: Cancel).
		m.setDialogAnswer('cancel');
		await dialog.getByRole('button', { name: 'Write file' }).click();
		await vi.waitFor(() => expect(apply).toHaveBeenCalledTimes(1));
		expect(Object.keys(apply.mock.calls[0]?.[0] ?? {})).toEqual(['previewId']);
		await vi.waitFor(() => expect(said()).toContain('Cancelled. CLAUDE.md is unchanged.'));
	});
});
