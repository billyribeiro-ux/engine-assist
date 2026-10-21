import { afterEach, describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';

import { goto } from '$app/navigation';
import { registerShortcuts } from '#lib/keyboard/registry.ts';
import { current } from '#lib/stores/project.svelte.ts';
import { ui } from '#lib/stores/ui.svelte.ts';
import { fresh, IDS, project, said } from '#lib/test/screen.ts';

import Palette from './Palette.svelte';

// M4-33 (AC-F028-04, AC-F028-05, AC-F028-03): ⌘K over projects, stages, steps and every action.
async function open(selectStep: string | null = '2.4') {
	const m = fresh('happy');
	const p = await project(m, IDS.tradelog);
	current.project = p;
	current.stage = 2;
	current.step = p.stages.flatMap((s) => s.steps).find((s) => s.key === (selectStep ?? '')) ?? null;
	ui.paletteOpen = true;
	const s = await render(Palette);
	await expect.element(s.getByRole('combobox')).toHaveFocus();
	return { m, p, s };
}

afterEach(() => {
	ui.paletteOpen = false;
	current.project = null;
	current.step = null;
});

describe('Palette', () => {
	it('primary: opens focused on the search field over every group', async () => {
		const { s } = await open();
		await expect.element(s.getByRole('dialog', { name: 'Commands' })).toBeInTheDocument();
		for (const g of ['Steps', 'Actions', 'Projects', 'Stages'])
			await expect.element(s.getByRole('group', { name: g })).toBeInTheDocument();
		await expect.element(s.getByRole('option', { name: /Swings/ }).first()).toBeInTheDocument();
		await expect.element(s.getByRole('option', { name: /Permits & Zoning/ })).toBeInTheDocument();
		await expect
			.element(s.getByRole('option', { name: /Copy prompt for 2\.4/ }))
			.toBeInTheDocument();
	});

	it('every action: the shortcut actions are listed with their keys', async () => {
		const { s } = await open();
		for (const [title, keys] of [
			['Run the selected step', '⌘↵'],
			['Copy command', '⌘⌥C'],
			['Next stage', '⌘]'],
			['Previous stage', '⌘['],
			['Open gate for this stage', '⌘⇧G'],
			['Toggle inspector', '⌘⌥I'],
			['Paste back into the selected chat step', '⌘⇧V'],
			['Mark a chat step done', '⌘D'],
			['Terminal focus mode', '⌘⇧T'],
			['Return focus from the terminal', '⌘⇧E'],
			['Do next', '⌘⇧↵'],
			['New project', '⌘N'],
			['Toggle terminal drawer', '⌃`'],
			['Settings', '⌘,']
		] as const) {
			const o = s.getByRole('option', { name: new RegExp(title.replace(/[()]/g, '\\$&')) });
			await expect.element(o.first()).toBeInTheDocument();
			expect(o.first().element().textContent, title).toContain(keys);
		}
	});

	it('typing filters across projects, stages and steps; ↓ and ↵ open the chosen row', async () => {
		const { s } = await open();
		await userEvent.keyboard('stack');
		const first = s.getByRole('option').first();
		await expect.element(first).toHaveAttribute('aria-selected', 'true');
		expect(first.element().textContent).toContain('2.4 Stack decision, evidence first');
		await userEvent.keyboard('{Enter}');
		await vi.waitFor(() =>
			expect(goto).toHaveBeenCalledWith(expect.stringContaining('/stages/2/steps/2.4'))
		);
		await expect.element(s.getByRole('dialog')).not.toBeInTheDocument();
		expect(ui.paletteOpen).toBe(false);
	});

	it('↑↓ move the active row and aria-activedescendant follows', async () => {
		const { s } = await open();
		await userEvent.keyboard('stage');
		const box = s.getByRole('combobox');
		const opts = s.getByRole('option');
		const id0 = opts.nth(0).element().id;
		await expect.element(box).toHaveAttribute('aria-activedescendant', id0);
		await userEvent.keyboard('{ArrowDown}');
		await expect.element(box).toHaveAttribute('aria-activedescendant', opts.nth(1).element().id);
		await userEvent.keyboard('{ArrowUp}{ArrowUp}');
		await expect.element(box).toHaveAttribute('aria-activedescendant', id0);
	});

	it('⌘↵ runs the step instead of opening it', async () => {
		const { s } = await open();
		await userEvent.keyboard('2.4 stack');
		const box = s.getByRole('combobox').element();
		box.dispatchEvent(
			new KeyboardEvent('keydown', { key: 'Enter', metaKey: true, bubbles: true, cancelable: true })
		);
		await vi.waitFor(() => expect(ui.paletteOpen).toBe(false));
		// 2.4 waits for 2.3 in this world: the run is refused with the reason, nothing is opened.
		await vi.waitFor(() => expect(said()).toMatch(/Waits for 2\.3|waits for 2\.3/));
		expect(goto).not.toHaveBeenCalledWith(expect.stringContaining('/steps/2.4'));
	});

	it('empty: a query with no match says what to try (AC-F028-04)', async () => {
		const { s } = await open();
		await userEvent.keyboard('stak');
		await expect
			.element(s.getByText(/No steps or actions match “stak”\. Try a step number like 2\.4\./))
			.toBeInTheDocument();
		await expect.element(s.getByRole('option')).not.toBeInTheDocument();
	});

	// Code review N6: with no query the Steps group is the open project's stage, found by the
	// project's ID; two projects with the same name don't mix their steps.
	it('palette::steps_filtered_by_project_id', async () => {
		const m = fresh('happy');
		for (const x of m.world.projects) if (x.id !== IDS.tradelog) x.name = 'TradeLog';
		const p = await project(m, IDS.tradelog);
		current.project = p;
		current.stage = 2;
		current.step = null;
		ui.paletteOpen = true;
		const s = await render(Palette);
		await expect.element(s.getByRole('group', { name: 'Steps' })).toBeInTheDocument();
		const titles = s
			.getByRole('group', { name: 'Steps' })
			.getByRole('option')
			.elements()
			.map((e) => e.textContent.split('TradeLog')[0]?.trim() ?? '');
		expect(titles.length).toBeGreaterThan(0);
		expect(new Set(titles).size).toBe(titles.length);
	});

	it('Esc closes the palette and returns focus to where it was', async () => {
		fresh('happy');
		const trigger = document.createElement('button');
		trigger.textContent = 'Earlier focus';
		document.body.append(trigger);
		trigger.focus();
		ui.paletteOpen = true;
		const s = await render(Palette);
		await expect.element(s.getByRole('combobox')).toHaveFocus();
		await userEvent.keyboard('{Escape}');
		expect(ui.paletteOpen).toBe(false);
		await expect.element(s.getByRole('dialog')).not.toBeInTheDocument();
		expect(document.activeElement).toBe(trigger);
		trigger.remove();
	});

	it('the rest of the window is inert while it is open, and not after', async () => {
		fresh('happy');
		const app = document.createElement('div');
		app.className = 'app';
		document.body.append(app);
		ui.paletteOpen = true;
		const s = await render(Palette);
		await vi.waitFor(() => expect(app.hasAttribute('inert')).toBe(true));
		await userEvent.keyboard('{Escape}');
		await expect.element(s.getByRole('dialog')).not.toBeInTheDocument();
		await vi.waitFor(() => expect(app.hasAttribute('inert')).toBe(false));
		app.remove();
	});

	it('an action runs the same handler as its shortcut (AC-F028-05)', async () => {
		await open();
		const calls: string[] = [];
		const off = registerShortcuts({
			nextStage: () => {
				calls.push('nextStage');
				return true;
			}
		});
		await userEvent.keyboard('next stage');
		await userEvent.keyboard('{Enter}');
		await vi.waitFor(() => expect(calls).toEqual(['nextStage']));
		off();
	});

	it('locked: an action that does not apply says why in one line (AC-F028-03)', async () => {
		await open(null);
		await userEvent.keyboard('mark a chat');
		await userEvent.keyboard('{Enter}');
		await vi.waitFor(() => expect(said()).toContain('Mark done works on a chat step.'));
	});

	it('pin: the palette pins the open project through the same command as the menu', async () => {
		const { m } = await open();
		const pin = vi.spyOn(m, 'projectPinSet');
		await userEvent.keyboard('pin');
		await userEvent.keyboard('{Enter}');
		await vi.waitFor(() => expect(pin).toHaveBeenCalled());
	});
});
