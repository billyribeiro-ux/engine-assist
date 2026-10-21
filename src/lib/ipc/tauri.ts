// The only module that imports @tauri-apps/api (ADR-004, ESLint no-restricted-imports).
import { Channel, invoke } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';

import { COMMAND_MAP, type CommandName, type IpcClient } from './contract.ts';
import { toIpcFailure } from './errors.ts';
import type { EventName, EventPayloads, OnProgress, PtyOutput, Unlisten } from './types.ts';

const PTY: ReadonlySet<CommandName> = new Set(['launch_start', 'pty_attach', 'probe_folder_open']);
const PROGRESS: ReadonlySet<CommandName> = new Set([
	'launch_prepare',
	'chain_resume_prepare',
	'packet_repair_prepare',
	'gate_fix_start',
	'desktop_open',
	'probe_run'
]);
const REARM: ReadonlySet<CommandName> = new Set(['rearm_start', 'rearm_continue']);
const SIGN: ReadonlySet<CommandName> = new Set(['sign_apply', 'sign_retry_commit']);

function channel<T>(cb: ((m: T) => void) | undefined): Channel<T> {
	const ch = new Channel<T>();
	if (cb) ch.onmessage = cb;
	return ch;
}

async function call(cmd: CommandName, args: Record<string, unknown>): Promise<unknown> {
	try {
		return await invoke(cmd, args);
	} catch (e: unknown) {
		throw toIpcFailure(e);
	}
}

/** Builds the argument object: `req` plus the command's Channel(s) by role (IPC-R1, §5). */
function args(cmd: CommandName, req: unknown, a?: unknown, b?: unknown): Record<string, unknown> {
	if (PTY.has(cmd)) return { req, output: channel(a as PtyOutput | undefined) };
	if (PROGRESS.has(cmd)) return { req, progress: channel(a as OnProgress | undefined) };
	if (REARM.has(cmd))
		return {
			req,
			output: channel(a as PtyOutput | undefined),
			progress: channel(b as OnProgress | undefined)
		};
	if (SIGN.has(cmd)) return { req, progress: channel(a as ((m: unknown) => void) | undefined) };
	return { req };
}

function build(): IpcClient {
	const client: Record<string, unknown> = {
		async listen<E extends EventName>(
			event: E,
			handler: (payload: EventPayloads[E]) => void
		): Promise<Unlisten> {
			return tauriListen<EventPayloads[E]>(event, (e) => {
				handler(e.payload);
			});
		}
	};
	for (const [cmd, method] of Object.entries(COMMAND_MAP) as [CommandName, string][]) {
		client[method] = (req: unknown, a?: unknown, b?: unknown) => call(cmd, args(cmd, req, a, b));
	}
	return client as unknown as IpcClient;
}

/** The real client: every contract method invokes its Rust command. */
export function createTauriClient(): IpcClient {
	return build();
}
