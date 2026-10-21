import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';

import { goto } from '$app/navigation';
import { IDS, fresh, said } from '#lib/test/screen.ts';

import ProjectMenu from './ProjectMenu.svelte';

// M4-25 (AC-F017-10, AC-F014-11): pin, Open terminal here and Remove from the open project's menu.
async function summary(m: ReturnType<typeof fresh>, id: string) {
	const all = await m.projectsList({});
	const p = all.find((x) => x.id === id);
	if (!p) throw new Error('no such project');
	return p;
}

describe('ProjectMenu', () => {
	it('primary: the keyboard opens the menu, ↑↓ move, Esc closes and returns focus', async () => {
		const m = fresh('happy');
		const s = await render(ProjectMenu, { project: await summary(m, IDS.tradelog) });
		const button = s.getByRole('button', { name: 'Project' });
		(button.element() as HTMLElement).focus();
		await userEvent.keyboard('{ArrowDown}');
		const items = s.getByRole('menuitem');
		await expect.element(items.first()).toHaveFocus();
		await userEvent.keyboard('{ArrowDown}');
		await expect.element(items.nth(1)).toHaveFocus();
		await userEvent.keyboard('{ArrowUp}{ArrowUp}');
		await expect.element(items.nth(2)).toHaveFocus();
		await userEvent.keyboard('{Escape}');
		await expect.element(s.getByRole('menu')).not.toBeInTheDocument();
		await expect.element(button).toHaveFocus();
	});

	it('pin: an unpinned project takes the lowest free ⌘ number, and a pinned one is unpinned', async () => {
		const m = fresh('happy');
		const before = await m.projectsList({});
		const target = before.find((p) => p.pinned === null);
		expect(target).toBeDefined();
		if (!target) return;
		const used = before.flatMap((p) => (p.pinned ? [p.pinned] : []));
		const lowest = [1, 2, 3, 4, 5, 6, 7, 8, 9].find((n) => !used.includes(n));
		const pin = vi.spyOn(m, 'projectPinSet');
		const s = await render(ProjectMenu, { project: target });
		await s.getByRole('button', { name: 'Project' }).click();
		await s.getByRole('menuitem', { name: 'Pin to a ⌘ number' }).click();
		await vi.waitFor(() =>
			expect(pin).toHaveBeenCalledWith({ projectId: target.id, slot: lowest ?? null })
		);
		expect(said()).toContain(`pinned to ⌘${lowest}`);

		await s.unmount();
		const pinned = await summary(m, target.id);
		const s2 = await render(ProjectMenu, { project: pinned });
		await s2.getByRole('button', { name: 'Project' }).click();
		await s2.getByRole('menuitem', { name: `Unpin from ⌘${lowest}` }).click();
		await vi.waitFor(() =>
			expect(pin).toHaveBeenLastCalledWith({ projectId: target.id, slot: null })
		);
	});

	it('Open terminal here asks Rust to open Terminal at the project', async () => {
		const m = fresh('happy');
		const open = vi.spyOn(m, 'terminalOpenHere');
		const s = await render(ProjectMenu, { project: await summary(m, IDS.tradelog) });
		await s.getByRole('button', { name: 'Project' }).click();
		await s.getByRole('menuitem', { name: 'Open terminal here' }).click();
		await vi.waitFor(() => expect(open).toHaveBeenCalledWith({ projectId: IDS.tradelog }));
	});

	it('Remove asks, Rust confirms natively, and the project is gone and Today opens', async () => {
		const m = fresh('happy');
		const remove = vi.spyOn(m, 'projectRemove');
		const s = await render(ProjectMenu, { project: await summary(m, IDS.swings) });
		await s.getByRole('button', { name: 'Project' }).click();
		await s.getByRole('menuitem', { name: 'Remove project…' }).click();
		await vi.waitFor(() => expect(remove).toHaveBeenCalledWith({ projectId: IDS.swings }));
		await vi.waitFor(() => expect(goto).toHaveBeenCalledWith('/'));
		expect((await m.projectsList({})).some((p) => p.id === IDS.swings)).toBe(false);
	});

	it('cancelled: cancelling the native dialog keeps the project and says nothing', async () => {
		const m = fresh('happy');
		m.setDialogAnswer('cancel');
		const s = await render(ProjectMenu, { project: await summary(m, IDS.swings) });
		await s.getByRole('button', { name: 'Project' }).click();
		await s.getByRole('menuitem', { name: 'Remove project…' }).click();
		await vi.waitFor(async () =>
			expect((await m.projectsList({})).some((p) => p.id === IDS.swings)).toBe(true)
		);
		expect(said()).not.toContain('Cancelled');
		expect(said()).not.toContain('removed');
	});

	it('error: a live session blocks the removal and the sentence says so', async () => {
		const m = fresh('happy');
		const s = await render(ProjectMenu, { project: await summary(m, IDS.tradelog) });
		await s.getByRole('button', { name: 'Project' }).click();
		await s.getByRole('menuitem', { name: 'Remove project…' }).click();
		await vi.waitFor(() => expect(said()).toMatch(/session|running/i));
	});
});
