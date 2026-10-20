// Picks the IPC implementation once, at startup (ADR-004 §4): the mock only when the build was made
// with VITE_IPC=mock (`pnpm dev:mock`). Vite replaces import.meta.env.VITE_IPC at build time, so the
// release bundle never contains the mock module (ipc::mock_not_in_release).
import type { IpcClient } from './contract.ts';

let client: Promise<IpcClient> | undefined;

export function ipc(): Promise<IpcClient> {
	client ??=
		import.meta.env.VITE_IPC === 'mock'
			? import('./mock.ts').then((m) => new m.MockIpcClient())
			: import('./tauri.ts').then((m) => new m.TauriIpcClient());
	return client;
}

export type { IpcClient } from './contract.ts';
