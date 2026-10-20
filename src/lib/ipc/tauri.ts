// The only module that imports @tauri-apps/api (ADR-004, ESLint no-restricted-imports).
import { invoke } from '@tauri-apps/api/core';

import type { AppBootstrap } from './bindings/AppBootstrap.ts';
import type { AppBootstrapReq } from './bindings/AppBootstrapReq.ts';
import type { CommandName, IpcClient } from './contract.ts';
import { toIpcFailure } from './errors.ts';

async function call<T>(cmd: CommandName, req: unknown): Promise<T> {
	try {
		return await invoke<T>(cmd, { req });
	} catch (e: unknown) {
		throw toIpcFailure(e);
	}
}

export class TauriIpcClient implements IpcClient {
	appBootstrap(req: AppBootstrapReq): Promise<AppBootstrap> {
		return call('app_bootstrap', req);
	}
}
