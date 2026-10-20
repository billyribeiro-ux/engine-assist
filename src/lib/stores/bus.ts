// Invalidation bus (ipc.md §6): Rust events carry identifiers and revisions only; each turns into
// refresh keys that resources listen for, and the resource refetches through a command. An event
// whose `rev` is not newer than the one already seen for that project is ignored (§6.2).
import { ipc } from '#lib/ipc/index.ts';
import { EVENT_NAMES, type EventName, type EventPayloads } from '#lib/ipc/types.ts';

export type RefreshKey =
	| 'projects'
	| 'today'
	| 'settings'
	| 'doctor'
	| 'doctorStatus'
	| 'runs'
	| `project:${string}`
	| `packet:${string}:${number}`
	| `pending:${string}`;

type Listener = (key: RefreshKey) => void;
const listeners = new Set<Listener>();
const revs = new Map<string, number>();
let started: Promise<void> | null = null;
let ticker: ReturnType<typeof setInterval> | null = null;

/** Items that appear with time (a chat prompt copied 10 minutes ago, a wait step's day arriving)
 * are not events, so Today and the sidebar refetch once a minute (AC-F017-08). */
export const TODAY_TICK_MS = 60_000;

export function onRefresh(fn: Listener): () => void {
	listeners.add(fn);
	return () => listeners.delete(fn);
}

/** Asks every resource that depends on one of `keys` to refetch. */
export function refresh(...keys: RefreshKey[]): void {
	for (const k of keys) for (const fn of listeners) fn(k);
}

function fresh(id: string, rev: number): boolean {
	const seen = revs.get(id);
	if (seen !== undefined && rev <= seen) return false;
	revs.set(id, rev);
	return true;
}

export function keysFor<E extends EventName>(event: E, p: EventPayloads[E]): RefreshKey[] {
	switch (event) {
		case 'project:changed': {
			const e = p as EventPayloads['project:changed'];
			return fresh(e.projectId, e.rev) ? [`project:${e.projectId}`, 'today', 'projects'] : [];
		}
		case 'projects:changed':
			return ['projects', 'today'];
		case 'packet:changed':
		case 'tamper:detected': {
			const e = p as EventPayloads['packet:changed'];
			return [`packet:${e.projectId}:${e.stage}`, `project:${e.projectId}`, 'today'];
		}
		case 'decision:pending': {
			const e = p as EventPayloads['decision:pending'];
			return [`pending:${e.projectId}`, `project:${e.projectId}`, 'today'];
		}
		case 'run:status':
			return ['runs', 'today'];
		case 'config:changed': {
			const e = p as EventPayloads['config:changed'];
			return [`project:${e.projectId}`];
		}
		case 'doctor:status':
			// Only status readers refetch: Doctor itself fetches through doctor_run, which emits
			// this event, so refreshing 'doctor' here would run the checks in a loop.
			return ['doctorStatus'];
		case 'settings:changed':
			return ['settings'];
	}
	return [];
}

/** Subscribes once to every event and fans them out as refresh keys. */
export function startBus(): Promise<void> {
	started ??= (async () => {
		ticker ??= setInterval(() => refresh('today'), TODAY_TICK_MS);
		const client = await ipc();
		for (const name of EVENT_NAMES) {
			await client.listen(name, (payload) => {
				refresh(...keysFor(name, payload as never));
			});
		}
	})();
	return started;
}

/** Test helper: forget subscriptions and revisions. */
export function resetBus(): void {
	if (ticker !== null) clearInterval(ticker);
	ticker = null;
	started = null;
	revs.clear();
}
