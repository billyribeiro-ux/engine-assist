import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import { failing, fresh, IDS, project, said } from '#lib/test/screen.ts';

import Packet from './Packet.svelte';

// M3-28 / M4-17 (F-010): the packet view against the mock.
describe('Packet', () => {
	it('primary: an invalid packet lists its problems with fixes', async () => {
		const m = fresh('invalidPacket');
		const p = await project(m, IDS.tradelog);
		const s = await render(Packet, { project: p, stageNo: 2 });
		await expect.element(s.getByRole('heading', { name: /Stage 2 packet/ })).toBeVisible();
		await expect.element(s.getByText('2 problems')).toBeVisible();
		await expect.element(s.getByText(/DATA WE HANDLE is required/)).toBeVisible();
		const repair = vi.spyOn(m, 'packetRepairPrepare');
		await s.getByRole('button', { name: 'Repair' }).first().click();
		await vi.waitFor(() => expect(repair).toHaveBeenCalled());
	});

	it('empty: no packet yet says which step writes it', async () => {
		const m = fresh('happy');
		const p = await project(m, IDS.foreman);
		const s = await render(Packet, { project: p, stageNo: 4 });
		await expect.element(s.getByText('No Stage 4 packet yet')).toBeVisible();
	});

	it('error: a failed load is announced', async () => {
		const m = fresh('happy');
		failing(m, 'packetView');
		const p = await project(m, IDS.foreman);
		const s = await render(Packet, { project: p, stageNo: 4 });
		await expect.element(s.getByRole('alert')).toBeVisible();
	});

	it('normalize: the changes are listed first and written only on the button (AC-F013-08)', async () => {
		const m = fresh('invalidPacket');
		const p = await project(m, IDS.tradelog);
		const apply = vi.spyOn(m, 'packetNormalizeApply');
		const s = await render(Packet, { project: p, stageNo: 2 });
		await expect
			.element(s.getByRole('heading', { name: 'Formatting Foreman can fix' }))
			.toBeVisible();
		await expect.element(s.getByText('unsigned').first()).toBeVisible();
		expect(apply).not.toHaveBeenCalled();
		await s.getByRole('button', { name: 'Normalize the file' }).click();
		await vi.waitFor(() => expect(apply).toHaveBeenCalledTimes(1));
		await vi.waitFor(() => expect(said()).toContain('Packet normalized'));
		await expect
			.element(s.getByRole('heading', { name: 'Formatting Foreman can fix' }))
			.not.toBeInTheDocument();
	});
});
