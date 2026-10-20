// A fetched IPC value that refetches when its inputs change or a matching refresh key arrives.
// Call during component init (it uses $effect for its lifetime).
import type { IpcClient } from '#lib/ipc/contract.ts';
import { toIpcFailure, type IpcFailure } from '#lib/ipc/errors.ts';
import { ipc } from '#lib/ipc/index.ts';

import { onRefresh, startBus, type RefreshKey } from './bus.svelte.ts';

export interface Resource<T> {
	readonly value: T | undefined;
	readonly error: IpcFailure | null;
	readonly loading: boolean;
	reload(): Promise<void>;
	/** Replaces the value with a command's fresh response (e.g. a StepView after run_input_set). */
	set(value: T): void;
}

export function resource<A, T>(
	args: () => A,
	fetch: (client: IpcClient, args: A) => Promise<T>,
	keys: (args: A) => RefreshKey[] = () => []
): Resource<T> {
	const s = $state<{ value: T | undefined; error: IpcFailure | null; loading: boolean }>({
		value: undefined,
		error: null,
		loading: true
	});
	let seq = 0;
	let current: A;

	async function load(a: A): Promise<void> {
		const mine = ++seq;
		s.loading = true;
		try {
			const client = await ipc();
			const v = await fetch(client, a);
			if (mine === seq) {
				s.value = v;
				s.error = null;
			}
		} catch (e: unknown) {
			if (mine === seq) s.error = toIpcFailure(e);
		} finally {
			if (mine === seq) s.loading = false;
		}
	}

	void startBus();
	$effect(() => {
		const a = args();
		current = a;
		void load(a);
		const ks = new Set(keys(a));
		return onRefresh((k) => {
			if (ks.has(k)) void load(current);
		});
	});

	return {
		get value() {
			return s.value;
		},
		get error() {
			return s.error;
		},
		get loading() {
			return s.loading;
		},
		reload: () => load(current),
		set: (v: T) => {
			seq += 1;
			s.value = v;
			s.error = null;
			s.loading = false;
		}
	};
}
