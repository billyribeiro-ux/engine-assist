// Component-test helpers (M2-27): every screen test starts from a fresh mock world, an empty
// refresh bus and no terminal tabs, and can make one command fail.
import { vi } from 'vitest';

import { IpcFailure } from '#lib/ipc/errors.ts';
import { type MockIpcClient, resetMock } from '#lib/ipc/mock.ts';
import type { Scenario } from '#lib/ipc/mock/world.ts';
import type { ProjectView } from '#lib/ipc/types.ts';
import { resetBus } from '#lib/stores/bus.ts';
import { terminal } from '#lib/stores/terminal.svelte.ts';
import { ui } from '#lib/stores/ui.svelte.ts';

export { IDS } from '#lib/ipc/mock/world.ts';

export function fresh(scenario: Scenario = 'happy'): MockIpcClient {
	vi.restoreAllMocks();
	resetBus();
	terminal.reset();
	ui.toasts = [];
	ui.hint = '';
	return resetMock(scenario);
}

/** Makes one mock command reject with an `io` failure. */
export function failing(m: MockIpcClient, command: keyof MockIpcClient): void {
	vi.spyOn(m, command as never).mockRejectedValue(
		new IpcFailure({ code: 'io', message: 'disk is full', detail: null }) as never
	);
}

export async function project(m: MockIpcClient, id: string): Promise<ProjectView> {
	return m.projectOpen({ projectId: id });
}

/** Text of the toasts and hint so far. */
export function said(): string {
	return [...ui.toasts.map((t) => t.text), ui.hint].join('\n');
}
