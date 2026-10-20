import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';

import { goto } from '$app/navigation';
import type { InboxItem } from '#lib/ipc/types.ts';
import { handlerFor } from '#lib/keyboard/registry.ts';
import { TODAY_TICK_MS } from '#lib/stores/bus.ts';

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

// M4-25 (AC-F017-07): the inbox in priority order, J/K and Enter.
describe('Today inbox', () => {
	// A Today whose inbox holds one item per given kind, in the order given.
	const stub = (m: ReturnType<typeof fresh>, order: InboxItem['kind'][]) => {
		const real = m.todayGet.bind(m);
		return vi.spyOn(m, 'todayGet').mockImplementation(async (req) => {
			const view = await real(req);
			const base = view.inbox[0];
			if (!base) throw new Error('the happy world has an inbox item to copy');
			return {
				...view,
				inbox: order.map((kind, n) => ({
					...base,
					kind,
					title: `item ${n} ${kind}`,
					detail: 'detail'
				}))
			};
		});
	};

	it('orders the rows by AC-F017-07 whatever order they arrive in', async () => {
		const m = fresh('happy');
		stub(m, [
			'stepReady',
			'tamper',
			'waitElapsed',
			'sessionYourTurn',
			'gateFailed',
			'packetLanded'
		]);
		const s = await render(Today);
		const list = s.getByRole('list', { name: 'Waiting on you, in priority order' });
		await expect.element(list).toBeVisible();
		const text = list.element().textContent;
		const at = (k: string) => text.indexOf(k);
		expect(at('sessionYourTurn')).toBeLessThan(at('packetLanded'));
		expect(at('packetLanded')).toBeLessThan(at('gateFailed'));
		expect(at('gateFailed')).toBeLessThan(at('tamper'));
		expect(at('tamper')).toBeLessThan(at('waitElapsed'));
		expect(at('waitElapsed')).toBeLessThan(at('stepReady'));
	});

	it('J and K move between rows and Return runs the focused row’s action', async () => {
		const m = fresh('happy');
		stub(m, ['gateSignable', 'packetInvalid']);
		const s = await render(Today);
		const list = s.getByRole('list', { name: 'Waiting on you, in priority order' });
		const buttons = () => [...list.element().querySelectorAll<HTMLElement>('[data-today-row]')];
		await vi.waitFor(() => expect(buttons()).toHaveLength(2));
		expect(buttons().map((b) => b.textContent.trim())).toEqual(['Open packet', 'Review and sign']);
		buttons()[0]?.focus();
		const next = handlerFor('nextStep');
		expect(next).toBeDefined();
		await next?.();
		expect(document.activeElement).toBe(buttons()[1]);
		await handlerFor('prevStep')?.();
		expect(document.activeElement).toBe(buttons()[0]);
		await userEvent.keyboard('{Enter}');
		await vi.waitFor(() => expect(goto).toHaveBeenCalledWith(expect.stringContaining('/packet')));
	});

	it('polls once a minute, because wait and awaiting-paste items appear with time', async () => {
		const m = fresh('happy');
		const spy = vi.spyOn(m, 'todayGet');
		vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] });
		try {
			await render(Today);
			await vi.waitFor(() => expect(spy).toHaveBeenCalled());
			const before = spy.mock.calls.length;
			await vi.advanceTimersByTimeAsync(TODAY_TICK_MS);
			await vi.waitFor(() => expect(spy.mock.calls.length).toBeGreaterThan(before));
		} finally {
			vi.useRealTimers();
		}
	});
});
