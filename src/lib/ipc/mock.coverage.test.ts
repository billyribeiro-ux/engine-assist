// ipc/mock.coverage.test.ts (docs/contracts/ipc.md §7.2 rules 1, 6 and 7).
import { describe, expect, it } from 'vitest';

import { COMMAND_MAP, splitPtyData, type CommandName } from './contract.ts';
import { isIpcError } from './errors.ts';
import { requests, responses } from './fixtures/index.ts';
import { MockIpcClient } from './mock.ts';

function call(m: MockIpcClient, cmd: CommandName, req: unknown): Promise<unknown> {
	const fn = (m as unknown as Record<string, (r: unknown) => Promise<unknown>>)[COMMAND_MAP[cmd]];
	if (!fn) throw new Error(cmd);
	return fn.call(m, req);
}

describe('MockIpcClient', () => {
	it('answers every command with a typed result or a typed IpcError (never "not implemented")', async () => {
		for (const cmd of Object.keys(COMMAND_MAP) as CommandName[]) {
			const m = new MockIpcClient();
			m.streamDelayMs = 0;
			const out = await call(m, cmd, requests[cmd]).then(
				(v) => ({ ok: true, v }),
				(e: unknown) => ({ ok: false, v: e })
			);
			if (!out.ok) expect(isIpcError(out.v), `${cmd}: ${String(out.v)}`).toBe(true);
		}
	});

	it('has a response fixture for every command', () => {
		for (const cmd of Object.keys(COMMAND_MAP)) expect(responses, cmd).toHaveProperty(cmd);
	});

	it('rejects unknown fields with a typed Invalid, like Rust (one case per command)', async () => {
		for (const cmd of Object.keys(COMMAND_MAP) as CommandName[]) {
			const err: unknown = await call(new MockIpcClient(), cmd, {
				...requests[cmd],
				extra: 1
			}).catch((e: unknown) => e);
			expect(err, cmd).toMatchObject({
				code: 'invalid',
				detail: { kind: 'invalid', field: 'req', rule: 'unknownField' }
			});
		}
	});

	it('returns copies, so callers cannot mutate state', async () => {
		const mock = new MockIpcClient();
		const a = await mock.appBootstrap({});
		a.projects.length = 0;
		const b = await mock.appBootstrap({});
		expect(b.projects.length).toBeGreaterThan(0);
	});

	it('native dialogs are scripted: Cancel returns Cancelled and changes nothing', async () => {
		const mock = new MockIpcClient();
		mock.setDialogAnswer('cancel');
		const before = await mock.pendingChangesList({ projectId: requests.project_open.projectId });
		const err: unknown = await mock
			.pendingChangeAccept(requests.pending_change_accept)
			.catch((e: unknown) => e);
		expect(err).toMatchObject({ code: 'cancelled' });
		expect(await mock.pendingChangesList({ projectId: requests.project_open.projectId })).toEqual(
			before
		);
	});

	it('preview and plan IDs are one-time (IPC-R7)', async () => {
		const mock = new MockIpcClient();
		const pv = await mock.progressRebuildPreview({ projectId: requests.project_open.projectId });
		await mock.progressRebuildApply({ previewId: pv.previewId });
		await expect(mock.progressRebuildApply({ previewId: pv.previewId })).rejects.toMatchObject({
			code: 'expired'
		});
	});

	it('pty_write to a run that is not live is NotAllowed', async () => {
		const mock = new MockIpcClient('stage4');
		await expect(
			mock.ptyWrite({ runId: requests.pty_write.runId, data: 'x' })
		).rejects.toMatchObject({ code: 'notAllowed', detail: { rule: 'pty-not-live' } });
	});

	it('delivers PTY output as ArrayBuffer chunks and exits on /exit (like fake-claude)', async () => {
		const mock = new MockIpcClient();
		mock.streamDelayMs = 0;
		const chunks: ArrayBuffer[] = [];
		const statuses: string[] = [];
		await mock.listen('run:status', (p) => statuses.push(p.status));
		await mock.ptyAttach({ runId: requests.pty_attach.runId }, (b) => chunks.push(b));
		await new Promise((r) => setTimeout(r, 20));
		expect(chunks.length).toBeGreaterThan(3);
		expect(chunks[0]).toBeInstanceOf(ArrayBuffer);
		await mock.ptyWrite({ runId: requests.pty_write.runId, data: '/exit\r' });
		await new Promise((r) => setTimeout(r, 0));
		expect(statuses).toContain('exited');
	});

	it('terminal::large_paste_chunked: pty_write data is split at code points into ≤ 64 KB', () => {
		const big = 'é'.repeat(70_000);
		const parts = splitPtyData(big);
		expect(parts.join('')).toBe(big);
		for (const p of parts)
			expect(new TextEncoder().encode(p).length).toBeLessThanOrEqual(64 * 1024);
		expect(parts.length).toBe(3);
	});
});
