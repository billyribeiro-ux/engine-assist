// Terminal tabs (F-009): one tab per run, named after the session. PTY bytes arrive through the
// Channel callback; the store keeps each run's bytes so a tab can be re-mounted, and pauses the
// PTY above 1 MB of unwritten output (NFR-08, ipc.md §5).
import type { IpcClient } from '#lib/ipc/contract.ts';
import { splitPtyData } from '#lib/ipc/contract.ts';
import { ipc } from '#lib/ipc/index.ts';
import type { RunStatus, RunStarted } from '#lib/ipc/types.ts';
import type { Archive } from '#lib/terminal/archive.ts';
import { SCROLLBACK_DEFAULT } from '#lib/terminal/options.ts';

export interface Tab {
	runId: string;
	chain: string;
	tabName: string;
	status: RunStatus;
	exitCode: number | null;
	projectId: string | null;
	/** Archived buffer of the run this one re-armed (shown collapsed, read-only; AC-F009-06). */
	archived: Archive | null;
	divider: string | null;
}

type Sink = (bytes: Uint8Array) => void;
const HIGH_WATER = 1024 * 1024;

class Terminal {
	tabs = $state<Tab[]>([]);
	active = $state<string | null>(null);
	focusMode = $state(false);
	/** The scrollback cap from Settings (the drawer keeps it current; AC-F009-06). */
	scrollback = SCROLLBACK_DEFAULT;
	/** Bumped to move focus into the active tab's terminal (⌘⇧T, first-run probe). */
	focusTick = $state(0);
	private buffers = new Map<string, Uint8Array[]>();
	private sinks = new Map<string, Sink>();
	private pending = new Map<string, number>();
	private attached = new Set<string>();
	private cols = new Map<string, number>();

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

	add(
		started: RunStarted,
		projectId: string | null,
		divider: string | null = null,
		archived: Tab['archived'] = null
	): void {
		if (!this.tabs.some((t) => t.runId === started.runId))
			this.tabs = [
				...this.tabs,
				{
					runId: started.runId,
					chain: started.chain,
					tabName: started.tabName,
					status: 'running',
					exitCode: null,
					projectId,
					archived,
					divider
				}
			];
		this.attached.add(started.runId);
		this.active = started.runId;
	}

	/** Re-attaches to a run that is already live (startup, ipc.md §4.7 pty_attach). */
	async attach(
		client: IpcClient,
		runId: string,
		projectId: string | null,
		chain: string
	): Promise<void> {
		if (this.attached.has(runId)) return;
		this.attached.add(runId);
		const info = await client.ptyAttach({ runId }, this.output(runId));
		if (!this.tabs.some((t) => t.runId === runId))
			this.tabs = [
				...this.tabs,
				{
					runId,
					chain,
					tabName: info.tabName,
					status: info.status,
					exitCode: null,
					projectId,
					archived: null,
					divider: null
				}
			];
		this.active ??= runId;
	}

	requestFocus(): void {
		this.focusTick += 1;
	}

	/**
	 * `run:status` (AC-F009-02, AC-F009-03): the tab takes the new status at once; when the run
	 * ended, its exit code comes from the run record (`runs_search`, newest first), so the tab says
	 * `exited 1`, not a guess.
	 */
	async statusChanged(client: IpcClient, runId: string, status: RunStatus): Promise<void> {
		this.setStatus(runId, status);
		if (status !== 'exited' && status !== 'errored') return;
		const page = await client
			.runsSearch({ query: '', projectId: null, cursor: null, limit: 50 })
			.catch(() => null);
		const run = page?.runs.find((r) => r.runId === runId);
		if (run) this.setStatus(runId, run.status, run.exitCode);
	}

	setStatus(runId: string, status: RunStatus, exitCode: number | null = null): void {
		this.tabs = this.tabs.map((t) =>
			t.runId === runId ? { ...t, status, exitCode: exitCode ?? t.exitCode } : t
		);
	}

	close(runId: string): void {
		this.tabs = this.tabs.filter((t) => t.runId !== runId);
		this.sinks.delete(runId);
		this.buffers.delete(runId);
		this.pending.delete(runId);
		this.cols.delete(runId);
		if (this.active === runId) this.active = this.tabs.at(-1)?.runId ?? null;
	}

	/** The raw output a run has sent so far (for its archive on re-arm). */
	chunks(runId: string): readonly Uint8Array[] {
		return this.buffers.get(runId) ?? [];
	}

	/** The width a run's xterm last had (the archive replays at that width). */
	setCols(runId: string, cols: number): void {
		this.cols.set(runId, cols);
	}
	colsOf(runId: string): number {
		return this.cols.get(runId) ?? 120;
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
		if (before <= HIGH_WATER && after > HIGH_WATER)
			await client.ptyFlow({ runId, paused: true }).catch(() => undefined);
		if (before > HIGH_WATER && after <= HIGH_WATER)
			await client.ptyFlow({ runId, paused: false }).catch(() => undefined);
	}

	/** Writes input to the PTY in ≤ 64 KB pieces, in order (ipc.md §6.2). */
	async write(runId: string, data: string): Promise<void> {
		const client = await ipc();
		for (const part of splitPtyData(data)) await client.ptyWrite({ runId, data: part });
	}

	reset(): void {
		this.tabs = [];
		this.active = null;
		this.buffers.clear();
		this.sinks.clear();
		this.pending.clear();
		this.attached.clear();
		this.cols.clear();
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
