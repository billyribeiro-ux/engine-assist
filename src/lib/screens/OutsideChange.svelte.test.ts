import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh, IDS, project, said } from '#lib/test/screen.ts';

import OutsideChange from './OutsideChange.svelte';

// M2-24 (AC-F022-02, AC-F022-05): the outside-change review against the mock.
describe('OutsideChange', () => {
	it('primary: Accept asks Rust, which confirms natively', async () => {
		const m = fresh('outsideChange');
		const acc = vi.spyOn(m, 'pendingChangeAccept');
		const p = await project(m, IDS.tradelog);
		const s = await render(OutsideChange, { project: p });
		await s.getByRole('button', { name: /Accept changes/ }).click();
		await vi.waitFor(() => expect(acc).toHaveBeenCalled());
		expect(JSON.stringify(acc.mock.calls[0])).not.toMatch(/approv|confirm/i);
	});

	it('locked: Cancel in the native dialog leaves the change pending', async () => {
		const m = fresh('outsideChange');
		m.setDialogAnswer('cancel');
		const p = await project(m, IDS.tradelog);
		const s = await render(OutsideChange, { project: p });
		await s.getByRole('button', { name: /Accept changes/ }).click();
		await expect.element(s.getByRole('button', { name: /Accept changes/ })).toBeVisible();
		expect((await m.pendingChangesList({ projectId: IDS.tradelog })).length).toBeGreaterThan(0);
	});

	it('not applicable: a skip of a core step is shown as having no effect', async () => {
		const m = fresh('outsideChange');
		const p = await project(m, IDS.tradelog);
		const s = await render(OutsideChange, { project: p });
		await expect.element(s.getByText('Has no effect')).toBeVisible();
	});

	it('empty: nothing pending', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.foreman);
		const s = await render(OutsideChange, { project: p });
		await expect.element(s.getByRole('button', { name: /Accept changes/ })).not.toBeInTheDocument();
		expect(s.container.querySelector('.empty')).not.toBeNull();
	});

	it('error: Reject that fails is reported', async () => {
		const m = fresh('outsideChange');
		failing(m, 'pendingChangeReject');
		const p = await project(m, IDS.tradelog);
		const s = await render(OutsideChange, { project: p });
		await s.getByRole('button', { name: 'Reject and restore' }).click();
		await vi.waitFor(() => expect(said()).not.toBe(''));
		await expect.element(s.getByRole('button', { name: 'Reject and restore' })).toBeVisible();
	});
});
