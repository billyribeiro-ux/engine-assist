import { describe, expect, it, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import { failing, fresh, said } from '#lib/test/screen.ts';

import Settings from './Settings.svelte';

// M2-16 / M4-20 (F-023): settings against the mock.
describe('Settings', () => {
	it('primary: choosing a theme saves it', async () => {
		const m = fresh('happy');
		const upd = vi.spyOn(m, 'settingsUpdate');
		const s = await render(Settings);
		await s.getByRole('radio', { name: 'Vellum' }).click();
		await vi.waitFor(() => expect(upd).toHaveBeenCalled());
		expect(JSON.stringify(upd.mock.calls[0])).toContain('vellum');
	});

	it('locked: clearing run history needs the native dialog; Cancel clears nothing', async () => {
		const m = fresh('happy');
		m.setDialogAnswer('cancel');
		const clear = vi.spyOn(m, 'runsClear');
		const s = await render(Settings);
		await s.getByRole('button', { name: 'Clear run history…' }).click();
		await vi.waitFor(() => expect(clear).toHaveBeenCalled());
		await expect.element(s.getByText(/212 runs/)).toBeVisible();
	});

	it('error: a failed load is announced', async () => {
		const m = fresh('happy');
		failing(m, 'settingsGet');
		const s = await render(Settings);
		await expect.element(s.getByRole('alert')).toBeVisible();
	});

	// M4-34, AC-F029-04: each P2 setting persists (it comes back from settings_get) and takes
	// effect where it is used.
	it('AC-F029-04: theme persists and applies to the document', async () => {
		const m = fresh('happy');
		const s = await render(Settings);
		await s.getByRole('radio', { name: 'Night' }).click();
		await vi.waitFor(async () => expect((await m.settingsGet({})).theme).toBe('night'));
		await vi.waitFor(() =>
			expect(document.documentElement.getAttribute('data-theme')).toBe('night')
		);
		await s.getByRole('radio', { name: 'System' }).click();
		await vi.waitFor(() => expect(document.documentElement.hasAttribute('data-theme')).toBe(false));
	});

	it('AC-F029-04: terminal font size and scrollback persist; out of range is refused inline', async () => {
		const m = fresh('happy');
		const s = await render(Settings);
		const font = s.getByLabelText('Font size');
		await font.fill('14');
		await userEvent.keyboard('{Tab}');
		await vi.waitFor(async () => expect((await m.settingsGet({})).terminal.fontSize).toBe(14));
		const sb = s.getByLabelText('Scrollback per tab');
		await sb.fill('25000');
		await userEvent.keyboard('{Tab}');
		await vi.waitFor(async () => expect((await m.settingsGet({})).terminal.scrollback).toBe(25000));
		await font.fill('40');
		await expect.element(s.getByText('Use a size from 9 to 32.')).toBeVisible();
		await expect.element(font).toHaveAttribute('aria-invalid', 'true');
		expect((await m.settingsGet({})).terminal.fontSize).toBe(14);
	});

	it('AC-F029-04: global variable defaults persist', async () => {
		const m = fresh('happy');
		const s = await render(Settings);
		const before = await m.settingsGet({});
		const name = Object.keys(before.globalVariables)[0] ?? '';
		expect(name).not.toBe('');
		const input = s.getByLabelText(`[${name}]`);
		await input.fill('A new default');
		await userEvent.keyboard('{Tab}');
		await vi.waitFor(async () =>
			expect((await m.settingsGet({})).globalVariables[name]).toBe('A new default')
		);
	});

	it('AC-F029-04, AC-F029-05: the pass-through list changes only through the native dialog', async () => {
		const m = fresh('happy');
		const upd = vi.spyOn(m, 'settingsUpdate');
		const s = await render(Settings);
		const add = s.getByLabelText('Add a variable');
		await add.fill('MY_TOOL_HOME');
		m.setDialogAnswer('cancel');
		await s.getByRole('button', { name: 'Add', exact: true }).click();
		await vi.waitFor(() => expect(upd).toHaveBeenCalledTimes(1));
		expect(Object.keys(upd.mock.calls[0]?.[0] ?? {})).toEqual(['expectRev', 'patch']);
		expect((await m.settingsGet({})).envPassthrough).not.toContain('MY_TOOL_HOME');
		m.setDialogAnswer('ok');
		await s.getByRole('button', { name: 'Add', exact: true }).click();
		await vi.waitFor(async () =>
			expect((await m.settingsGet({})).envPassthrough).toContain('MY_TOOL_HOME')
		);
		await expect.element(s.getByRole('button', { name: 'Remove MY_TOOL_HOME' })).toBeVisible();
	});

	it('AC-F029-04: the blueprint repo is chosen in the Rust dialog; the login PATH can be captured and cleared', async () => {
		const m = fresh('happy');
		const pick = vi.spyOn(m, 'blueprintPickRepo');
		const capture = vi.spyOn(m, 'doctorCaptureLoginPath');
		const s = await render(Settings);
		await s.getByRole('button', { name: 'Choose…' }).first().click();
		await vi.waitFor(() => expect(pick).toHaveBeenCalledWith({ mode: 'choose' }));
		const again = s
			.getByRole('button', { name: 'Capture again' })
			.or(s.getByRole('button', { name: 'Capture my login shell PATH' }));
		await again.click();
		await vi.waitFor(() => expect(capture).toHaveBeenCalled());
		if ((await m.settingsGet({})).loginPath) {
			await s.getByRole('button', { name: 'Clear', exact: true }).click();
			await vi.waitFor(async () => expect((await m.settingsGet({})).loginPath).toBeNull());
		}
		expect(said()).not.toMatch(/failed/i);
	});
});
