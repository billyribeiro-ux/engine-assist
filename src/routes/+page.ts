// Today (placeholder until M2). load calls only the IpcClient, never fetch (ADR-002).
import type { AppBootstrap } from '#lib/ipc/bindings/AppBootstrap.ts';
import { toIpcFailure, type IpcFailure } from '#lib/ipc/errors.ts';
import { ipc } from '#lib/ipc/index.ts';

import type { PageLoad } from './$types';

export const load: PageLoad = async (): Promise<
	{ boot: AppBootstrap; error: null } | { boot: null; error: IpcFailure }
> => {
	try {
		const client = await ipc();
		return { boot: await client.appBootstrap({}), error: null };
	} catch (e: unknown) {
		return { boot: null, error: toIpcFailure(e) };
	}
};
