import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { terminal } from '#lib/stores/terminal.svelte.ts';
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

	it('AC-F009-13: focus mode toggles the drawer (⌘⇧T runs the same toggle, dispatch test)', async () => {
		fresh('happy');
		const a = 'a0000000-0000-4000-8000-0000000000a1';
		terminal.add({ runId: a, chain: 'c1', tabName: 'fm-tradelog-s02-2-2' }, null);
		const s = await render(TerminalDrawer);
		const b = s.getByRole('button', { name: 'Focus mode (⌘⇧T)' });
		await expect.element(b).toHaveAttribute('aria-pressed', 'false');
		await b.click();
		await expect.element(b).toHaveAttribute('aria-pressed', 'true');
		expect(terminal.focusMode).toBe(true);
		const drawer = s.getByRole('region', { name: 'Terminal drawer' });
		expect(drawer.element().classList.contains('focus-mode')).toBe(true);
		terminal.focusMode = false;
		await expect.element(b).toHaveAttribute('aria-pressed', 'false');
	});
});
