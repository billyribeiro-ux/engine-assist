// M4-10 (AC-F010-13, AC-F010-14, AC-F011-09): the mock's P2 sensor stand-in follows Rust's re-arm:
// sensor idle skips the idle dialog, outstanding tasks ask the busy question (Wait refuses), and
// without a sensor answer Billy's native idle confirmation stays.
import { describe, expect, it } from 'vitest';

import { requests } from './fixtures/requests.ts';
import { MockIpcClient } from './mock.ts';
import { IDS } from './mock/world.ts';

async function plan(m: MockIpcClient): Promise<string> {
	await m.configAck(requests.config_ack).catch(() => undefined);
	const out = await m.launchPrepare(requests.launch_prepare);
	if (out.kind !== 'ready') throw new Error(JSON.stringify(out));
	expect(out.rearm?.kind).toBe('idleConfirmation');
	return out.planId;
}

describe('mock sensor and re-arm', () => {
	it('idle: Run re-arms without the idle dialog', async () => {
		const m = new MockIpcClient();
		m.streamDelayMs = 0;
		m.setRunIdle(IDS.run23);
		const id = await plan(m);
		m.setDialogAnswer('cancel');
		const r = await m.rearmStart({ planId: id });
		expect(r.kind).toBe('relaunched');
	});

	it('busy: background tasks ask first, and Wait refuses', async () => {
		const m = new MockIpcClient();
		m.streamDelayMs = 0;
		m.setRunSensor(IDS.run23, { status: 'idle', tasks: 2 });
		const id = await plan(m);
		m.setDialogAnswer('cancel');
		await expect(m.rearmStart({ planId: id })).rejects.toMatchObject({
			code: 'notAllowed'
		});
		m.setDialogAnswer('ok');
		const r = await m.rearmStart({ planId: await plan(m) });
		expect(r.kind).toBe('relaunched');
	});

	it('no sensor answer: the native idle confirmation stays', async () => {
		const m = new MockIpcClient();
		m.streamDelayMs = 0;
		// The session ended its turn but no sensor event says so (P1, or hooks that never fire).
		const run = m.world.runs.find((x) => x.summary.runId === IDS.run23);
		if (run) run.summary.status = 'idle';
		const id = await plan(m);
		m.setDialogAnswer('cancel');
		await expect(m.rearmStart({ planId: id })).rejects.toMatchObject({
			code: 'cancelled'
		});
	});
});
