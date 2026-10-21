// Code review W5: the terminal store keeps about the scrollback of each run (not every byte
// forever, replayed on each remount), and a view that goes away with bytes still queued in its
// xterm resumes a PTY it had paused.
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import TerminalView from '#lib/components/TerminalView.svelte';
import { terminal } from '#lib/stores/terminal.svelte.ts';
import { fresh } from '#lib/test/screen.ts';

const RUN = 'a0000000-0000-4000-8000-0000000000c1';
const enc = new TextEncoder();

function lines(chunks: readonly Uint8Array[]): number {
	let n = 0;
	for (const c of chunks) for (const b of c) if (b === 10) n += 1;
	return n;
}

describe('terminal store (W5)', () => {
	it('terminal::buffers_capped_at_scrollback', () => {
		fresh('happy');
		terminal.scrollback = 100;
		const out = terminal.output(RUN);
		for (let i = 0; i < 1000; i++) out(enc.encode(`line ${String(i)}\r\n`).buffer);
		const kept = terminal.chunks(RUN);
		expect(lines(kept)).toBeGreaterThanOrEqual(100);
		expect(lines(kept)).toBeLessThanOrEqual(101);
		// The newest output is what stays.
		expect(new TextDecoder().decode(kept.at(-1))).toBe('line 999\r\n');
		// Output without line breaks is capped by bytes too.
		const long = terminal.output('b0000000-0000-4000-8000-0000000000c2');
		for (let i = 0; i < 200; i++) long(new Uint8Array(1024).fill(65).buffer);
		const bytes = terminal
			.chunks('b0000000-0000-4000-8000-0000000000c2')
			.reduce((s, c) => s + c.length, 0);
		expect(bytes).toBeLessThanOrEqual(100 * 512 + 1024);
		terminal.scrollback = 10_000;
	});

	it('terminal::unmount_resumes_paused_pty', async () => {
		const mock = fresh('happy');
		const runId = mock.world.runs.find((r) => r.live)?.summary.runId ?? RUN;
		terminal.add({ runId, chain: 'c1', tabName: 'fm-x-s02-2-2' }, null);
		const flow = vi.spyOn(mock, 'ptyFlow');
		const view = await render(TerminalView, { runId, live: true, label: 'Terminal output' });
		await vi.waitFor(() => expect(document.querySelector('.xterm-screen')).not.toBeNull());
		// 2 MB queued in xterm that never called back: the PTY is paused.
		await terminal.track(runId, 2 * 1024 * 1024);
		await vi.waitFor(() => expect(flow).toHaveBeenCalledWith({ runId, paused: true }));
		await view.unmount();
		await vi.waitFor(() => expect(flow).toHaveBeenLastCalledWith({ runId, paused: false }));
	});
});
