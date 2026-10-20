// Terminal tabs (F-009): one tab per run, named after the session. PTY bytes arrive through the
// Channel callback; the store keeps each run's bytes so a tab can be re-mounted, and pauses the
// PTY above 1 MB of unwritten output (NFR-08, ipc.md §5).
import type { IpcClient } from '#lib/ipc/contract.ts';
import { splitPtyData } from '#lib/ipc/contract.ts';
import { ipc } from '#lib/ipc/index.ts';
import type { RunStatus, RunStarted } from '#lib/ipc/types.ts';

export interface Tab {
	runId: string;
	chain: string;
	tabName: string;
	status: RunStatus;
	exitCode: number | null;
	projectId: string | null;
	/** Archived buffer of the run this one re-armed (shown collapsed, read-only). */
	archived: { label: string; lines: number } | null;
	divider: string | null;
}

type Sink = (bytes: Uint8Array) => void;
const HIGH_WATER = 1024 * 1024;

class Terminal {
	tabs = $state<Tab[]>([]);
	active = $state<string | null>(null);
	focusMode = $state(false);
	private buffers = new Map<string, Uint8Array[]>();
	private sinks = new Map<string, Sink>();
	private pending = new Map<string, number>();
	private attached = new Set<string>();

	/** The output callback for a run (passed as the Channel's handler). */
	output(runId: string): (buf: ArrayBuffer) => void {
		return (buf) => {
			const bytes = new Uint8Array(buf);
			let list = this.buffers.get(runId);
			if (!list) {
				list = [];
				this.buffers.set(runId, list);
			}
			list.push(bytes);
			this.sinks.get(runId)?.(bytes);
		};
	}

	add(started: RunStarted, projectId: string | null, divider: string | null = null, archived: Tab['archived'] = null): void {
		if (!this.tabs.some((t) => t.runId === started.runId))
			this.tabs = [
				...this.tabs,
				{ runId: started.runId, chain: started.chain, tabName: started.tabName, status: 'running', exitCode: null, projectId, archived, divider }
			];
		this.attached.add(started.runId);
		this.active = started.runId;
	}

	/** Re-attaches to a run that is already live (startup, ipc.md §4.7 pty_attach). */
	async attach(client: IpcClient, runId: string, projectId: string | null, chain: string): Promise<void> {
		if (this.attached.has(runId)) return;
		this.attached.add(runId);
		const info = await client.ptyAttach({ runId }, this.output(runId));
		if (!this.tabs.some((t) => t.runId === runId))
			this.tabs = [
				...this.tabs,
				{ runId, chain, tabName: info.tabName, status: info.status, exitCode: null, projectId, archived: null, divider: null }
			];
		this.active ??= runId;
	}

	setStatus(runId: string, status: RunStatus, exitCode: number | null = null): void {
		this.tabs = this.tabs.map((t) => (t.runId === runId ? { ...t, status, exitCode: exitCode ?? t.exitCode } : t));
	}

	close(runId: string): void {
		this.tabs = this.tabs.filter((t) => t.runId !== runId);
		this.sinks.delete(runId);
		if (this.active === runId) this.active = this.tabs.at(-1)?.runId ?? null;
	}

	/** Connects a mounted xterm; replays what arrived before it mounted. */
	connect(runId: string, sink: Sink): () => void {
		for (const b of this.buffers.get(runId) ?? []) sink(b);
		this.sinks.set(runId, sink);
		return () => {
			if (this.sinks.get(runId) === sink) this.sinks.delete(runId);
		};
	}

	/** Flow control: called by the view with the bytes it has queued in xterm. */
	async track(runId: string, delta: number): Promise<void> {
		const before = this.pending.get(runId) ?? 0;
		const after = Math.max(0, before + delta);
		this.pending.set(runId, after);
		const client = await ipc();
		if (before <= HIGH_WATER && after > HIGH_WATER) await client.ptyFlow({ runId, paused: true }).catch(() => undefined);
		if (before > HIGH_WATER && after <= HIGH_WATER) await client.ptyFlow({ runId, paused: false }).catch(() => undefined);
	}

	/** Writes input to the PTY in ≤ 64 KB pieces, in order (ipc.md §6.2). */
	async write(runId: string, data: string): Promise<void> {
		const client = await ipc();
		for (const part of splitPtyData(data)) await client.ptyWrite({ runId, data: part });
	}

	lines(runId: string): number {
		const text = new TextDecoder().decode(
			(this.buffers.get(runId) ?? []).reduce((a, b) => {
				const c = new Uint8Array(a.length + b.length);
				c.set(a);
				c.set(b, a.length);
				return c;
			}, new Uint8Array())
		);
		return text.split('\n').length;
	}

	reset(): void {
		this.tabs = [];
		this.active = null;
		this.buffers.clear();
		this.sinks.clear();
		this.pending.clear();
		this.attached.clear();
	}
}

export const terminal = new Terminal();

/** A Channel callback for a run whose ID isn't known yet; `bind` routes what arrived so far. */
export function pendingOutput(): { cb: (buf: ArrayBuffer) => void; bind: (runId: string) => void } {
	const early: ArrayBuffer[] = [];
	let target: ((buf: ArrayBuffer) => void) | null = null;
	return {
		cb: (buf) => {
			if (target) target(buf);
			else early.push(buf);
		},
		bind: (runId) => {
			target = terminal.output(runId);
			for (const b of early.splice(0)) target(b);
		}
	};
}
