import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh, said } from '#lib/test/screen.ts';

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
		await expect
			.element(s.getByRole('heading', { name: 'Start your first project' }))
			.toBeVisible();
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
		await expect
			.element(s.getByText('Nothing here runs until the folder is back.').first())
			.toBeVisible();
	});

	it('interrupted: a session from a crash is listed with Resume, through the preflight (AC-F020-10)', async () => {
		const m = fresh('brokenChain');
		const boot = await m.appBootstrap({});
		const run = boot.interrupted[0];
		expect(run).toBeDefined();
		if (!run) return;
		vi.spyOn(m, 'appBootstrap').mockResolvedValue({
			...boot,
			interrupted: [{ ...run, resumable: true, whyNotResumable: null }]
		});
		const prep = vi.spyOn(m, 'chainResumePrepare');
		const s = await render(Today);
		await expect.element(s.getByRole('heading', { name: 'Interrupted sessions' })).toBeVisible();
		await s.getByRole('button', { name: 'Resume' }).click();
		await vi.waitFor(() => expect(prep).toHaveBeenCalledWith({ chain: run.chain }));
	});

	it('interrupted: a session that cannot resume says why and offers nothing to run', async () => {
		fresh('brokenChain');
		const s = await render(Today);
		const list = s.getByRole('list', { name: 'Interrupted sessions' });
		await expect.element(list.getByText(/Transcript removed after 30 days/).first()).toBeVisible();
		await expect.element(s.getByRole('button', { name: 'Resume' })).not.toBeInTheDocument();
	});
});
