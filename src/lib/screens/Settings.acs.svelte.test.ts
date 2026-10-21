import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { patch } from '#lib/actions/settings.ts';
import { fresh, IDS, project } from '#lib/test/screen.ts';

import Packet from './Packet.svelte';
import Settings from './Settings.svelte';

// M2-14: tool paths, DRI name and editor are shown and changed through Rust dialogs, with refusals
// inline (the Settings screen); AC-F029-07's UI part (Open in editor disabled with the reason).
describe('Settings screen (M2-14)', () => {
	it('shows the claude and git paths, the DRI name and the editor', async () => {
		fresh('happy');
		const s = await render(Settings);
		await vi.waitFor(() => expect(s.container.querySelector('#s-claude')).not.toBeNull());
		const claude = s.container.querySelector<HTMLInputElement>('#s-claude');
		const git = s.container.querySelector<HTMLInputElement>('#s-git');
		expect(claude?.value).toMatch(/claude$/);
		expect(git?.value).toMatch(/git$/);
		await expect.element(s.getByLabelText('Editor for Open in editor')).toBeVisible();
		expect(s.container.textContent).toContain('Name for signatures');
	});

	it('a tool path is picked in the Rust dialog: the field is read-only and Choose… asks Rust', async () => {
		const m = fresh('happy');
		const pick = vi.spyOn(m, 'settingsPickTool');
		const s = await render(Settings);
		await vi.waitFor(() => expect(s.container.querySelector('#s-claude')).not.toBeNull());
		expect(s.container.querySelector<HTMLInputElement>('#s-claude')?.readOnly).toBe(true);
		expect(s.container.querySelector<HTMLInputElement>('#s-git')?.readOnly).toBe(true);
		const row = s.container.querySelector('#s-claude')?.closest('.srow');
		row?.querySelector<HTMLButtonElement>('button')?.click();
		await vi.waitFor(() => expect(pick).toHaveBeenCalledWith({ tool: 'claude' }));
	});

	it('an invalid editor bundle ID is refused inline and not saved', async () => {
		const m = fresh('happy');
		const update = vi.spyOn(m, 'settingsUpdate');
		const s = await render(Settings);
		const editor = s.getByLabelText('Editor for Open in editor');
		await editor.fill('not a bundle id');
		await expect.element(s.container.querySelector('#s-editor-err') as HTMLElement).toBeVisible();
		expect(update).not.toHaveBeenCalled();
	});

	it('AC-F029-07: with no editor set, Open in editor is disabled and says to set one', async () => {
		const m = fresh('invalidPacket');
		const settings = await m.settingsGet({});
		await m.settingsUpdate({
			expectRev: settings.rev,
			patch: patch({ editor: { kind: 'clear' } })
		});
		const p = await project(m, IDS.tradelog);
		const s = await render(Packet, { project: p, stageNo: 2 });
		const btn = s.getByRole('button', { name: 'Open in editor' }).first();
		await expect.element(btn).toHaveAttribute('aria-disabled', 'true');
		await expect
			.element(s.getByText(/Open in editor needs an editor: set one in Settings/))
			.toBeVisible();
		const open = vi.spyOn(m, 'fileOpen');
		(btn.element() as HTMLElement).click();
		expect(open).not.toHaveBeenCalled();
	});
});
