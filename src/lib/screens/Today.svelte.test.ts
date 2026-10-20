import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh, IDS, project, said } from '#lib/test/screen.ts';

import Today from './Today.svelte';

// M2-20 (AC-F017-01..05): Today against the mock.
describe('Today', () => {
	it('primary: the top row copies the chat prompt and opens the conversation', async () => {
		const m = fresh('happy');
		const s = await render(Today);
		const btn = s.getByRole('button', { name: /Copy and open/ });
		await expect.element(btn).toHaveFocus();
		const before = m.getClipboard();
		await btn.click();
		await vi.waitFor(() => expect(m.getClipboard()).not.toBe(before));
		expect(m.getClipboard()).toContain('Master Spec');
	});

	it('locked: a step that waits says why and launches nothing', async () => {
		const m = fresh('happy');
		const prep = vi.spyOn(m, 'launchPrepare');
		const s = await render(Today);
		const run = s.getByRole('button', { name: 'Run in terminal' });
		await expect.element(run).toHaveAttribute('aria-disabled', 'true');
		await expect.element(run).toHaveAccessibleDescription('Waits for 2.3 to finish');
		(run.element() as HTMLElement).click();
		await vi.waitFor(() => expect(said()).toContain('Waits for 2.3 to finish'));
		expect(said()).toContain('Waits for 2.3 to finish');
		expect(prep).not.toHaveBeenCalled();
	});

	it('empty: no projects offers the first project', async () => {
		fresh('empty');
		const s = await render(Today);
		await expect.element(s.getByRole('heading', { name: 'Start your first project' })).toBeVisible();
		await expect.element(s.getByRole('link', { name: /Start your first project/ })).toBeVisible();
	});

	it('error: a failed load is announced', async () => {
		const m = fresh('happy');
		failing(m, 'todayGet');
		const s = await render(Today);
		await expect.element(s.getByRole('alert')).toBeVisible();
	});

	it('not applicable: a read-only project with its folder missing has nothing to run', async () => {
		fresh('readOnly');
		const s = await render(Today);
		await expect.element(s.getByText('Nothing here runs until the folder is back.').first()).toBeVisible();
	});
});
