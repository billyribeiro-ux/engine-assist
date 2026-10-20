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
		expect(s.container.textContent).toMatch(/problem/i);
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
});
