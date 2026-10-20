// MockIpcClient (ADR-004, docs/contracts/ipc.md §7.2): browser preview (`pnpm dev:mock`), Vitest
// and Playwright. Never bundled in release (ipc::mock_not_in_release greps for the marker below).
import type { AppBootstrap } from './bindings/AppBootstrap.ts';
import type { AppBootstrapReq } from './bindings/AppBootstrapReq.ts';
import type { CommandName, IpcClient } from './contract.ts';
import { IPC_ERROR_TEXT, IpcFailure } from './errors.ts';
import { responses } from './fixtures/index.ts';

export const MOCK_IPC_MARKER = '__FOREMAN_MOCK_IPC__';

/** Request keys each command accepts; anything else is `Invalid` like Rust (IPC-R1, §7.2 rule 7). */
const REQUEST_KEYS: Record<CommandName, readonly string[]> = {
	app_bootstrap: []
};

function invalid(rule: string): IpcFailure {
	return new IpcFailure({
		code: 'invalid',
		message: IPC_ERROR_TEXT.invalid,
		detail: { kind: 'invalid', field: 'req', rule }
	});
}

function check(cmd: CommandName, req: unknown): IpcFailure | null {
	if (typeof req !== 'object' || req === null || Array.isArray(req)) return invalid('shape');
	const allowed = REQUEST_KEYS[cmd];
	return Object.keys(req).every((k) => allowed.includes(k)) ? null : invalid('unknownField');
}

function clone<T>(value: T): T {
	return structuredClone(value);
}

export class MockIpcClient implements IpcClient {
	readonly marker = MOCK_IPC_MARKER;

	appBootstrap(req: AppBootstrapReq): Promise<AppBootstrap> {
		const err = check('app_bootstrap', req);
		return err ? Promise.reject(err) : Promise.resolve(clone(responses.app_bootstrap));
	}
}
