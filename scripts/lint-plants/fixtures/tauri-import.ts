import { invoke } from '@tauri-apps/api/core';

export async function bypass(): Promise<unknown> {
	return invoke('app_bootstrap', { req: {} });
}
