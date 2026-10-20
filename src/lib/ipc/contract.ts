// The typed IPC contract (ADR-004, docs/contracts/ipc.md). One method per command in
// `bindings/commands.json`; `TauriIpcClient` and `MockIpcClient` both implement it (tsc enforces).
// Methods resolve with the response type or reject with an `IpcError` (never a bare string).
import type { AppBootstrap } from './bindings/AppBootstrap.ts';
import type { AppBootstrapReq } from './bindings/AppBootstrapReq.ts';

export interface IpcClient {
	appBootstrap(req: AppBootstrapReq): Promise<AppBootstrap>;
}

/** Rust command name → IpcClient method name. Its keys must equal `bindings/commands.json`. */
export const COMMAND_MAP = {
	app_bootstrap: 'appBootstrap'
} as const satisfies Record<string, keyof IpcClient>;

export type CommandName = keyof typeof COMMAND_MAP;
