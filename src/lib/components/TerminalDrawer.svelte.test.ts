import { describe, expect, it, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import { terminal } from '#lib/stores/terminal.svelte.ts';
import { ui } from '#lib/stores/ui.svelte.ts';
import { fresh } from '#lib/test/screen.ts';

import TerminalDrawer from './TerminalDrawer.svelte';

// M4-10, AC-F009-14: with the sensor, terminal tabs show `idle` and `blocked` (waiting for
// permission) as Rust's `run:status` reports them.
describe('TerminalDrawer', () => {
	it('states: tabs show idle and blocked from the sensor', async () => {
		fresh('happy');
		const a = 'a0000000-0000-4000-8000-0000000000a1';
		const b = 'a0000000-0000-4000-8000-0000000000b2';
		terminal.add({ runId: a, chain: 'c1', tabName: 'fm-tradelog-s02-2-2' }, null);
		terminal.add({ runId: b, chain: 'c2', tabName: 'fm-tradelog-s05-5-1' }, null);
		terminal.setStatus(a, 'idle');
		terminal.setStatus(b, 'blocked');
		const s = await render(TerminalDrawer);
		await expect.element(s.getByRole('tab', { name: /fm-tradelog-s02-2-2.*idle/ })).toBeVisible();
		await expect
			.element(s.getByRole('tab', { name: /fm-tradelog-s05-5-1.*waiting for permission/ }))
			.toBeVisible();
	});

	// AC-F030-05: ⌃` toggles the drawer (the dispatcher calls `ui.toggleDrawer`; keyboard.test
	// covers the key) and the drawer remembers its height, by keyboard or by dragging.
	it('AC-F030-05: the drawer toggles and remembers its height', async () => {
		fresh('happy');
		localStorage.removeItem('fm.drawerHeight');
		localStorage.removeItem('fm.drawerOpen');
		ui.drawerOpen = true;
		ui.setDrawerHeight(216);
		terminal.add(
			{ runId: 'a0000000-0000-4000-8000-0000000000c3', chain: 'c3', tabName: 'fm-x' },
			null
		);
		const s = await render(TerminalDrawer);
		const grip = s.getByRole('separator', { name: 'Resize terminal drawer' });
		await expect.element(grip).toHaveAttribute('aria-valuenow', '216');
		(grip.element() as HTMLElement).focus();
		await userEvent.keyboard('{ArrowUp}{ArrowUp}');
		await expect.element(grip).toHaveAttribute('aria-valuenow', '248');
		expect(localStorage.getItem('fm.drawerHeight')).toBe('248');
		// A new window (a fresh store reading the saved value) comes back at the same height.
		vi.resetModules();
		const reloaded = await import('#lib/stores/ui.svelte.ts');
		expect(reloaded.ui.drawerHeight).toBe(248);
		const again = s;
		// Hidden: the bar says so and offers Show; toggling brings the drawer back.
		ui.toggleDrawer();
		await expect.element(again.getByText(/1 session, hidden/)).toBeVisible();
		ui.toggleDrawer();
		await expect.element(again.getByRole('tab', { name: /fm-x/ })).toBeVisible();
		expect(localStorage.getItem('fm.drawerOpen')).toBe('1');
	});
});
