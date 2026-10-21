// Project-level actions shared by the title bar's project menu and the command palette: pin to a
// ⌘1–⌘9 slot (AC-F017-10), Open terminal here (AC-F014-11) and Remove. Remove's confirmation is the
// native dialog the command raises in Rust (SR-24a); the webview only asks and reports.
import { goto } from '$app/navigation';

import { failureText } from '#lib/actions/words.ts';
import { toIpcFailure } from '#lib/ipc/errors.ts';
import { ipc } from '#lib/ipc/index.ts';
import type { ProjectSummary } from '#lib/ipc/types.ts';
import { to } from '#lib/nav/routes.ts';
import { ui } from '#lib/stores/ui.svelte.ts';

export const PIN_SLOTS = [1, 2, 3, 4, 5, 6, 7, 8, 9] as const;

/** The lowest ⌘-number nobody uses, or null when all nine are taken. */
export function freeSlot(all: readonly ProjectSummary[], except?: string): number | null {
	const used = new Set(
		all.filter((p) => p.id !== except).flatMap((p) => (p.pinned ? [p.pinned] : []))
	);
	return PIN_SLOTS.find((s) => !used.has(s)) ?? null;
}

async function guarded(work: () => Promise<void>): Promise<void> {
	try {
		await work();
	} catch (e: unknown) {
		const f = toIpcFailure(e);
		// Cancelling the native dialog is an answer, not a problem.
		if (f.code !== 'cancelled') ui.say(failureText(f));
	}
}

/** Pins to the lowest free slot, or unpins when already pinned. */
export async function togglePin(projectId: string): Promise<void> {
	await guarded(async () => {
		const c = await ipc();
		const all = await c.projectsList({});
		const me = all.find((p) => p.id === projectId);
		if (!me) return;
		if (me.pinned) {
			await c.projectPinSet({ projectId, slot: null });
			ui.toast(`${me.name} unpinned from ⌘${me.pinned}`, 'pass', 'check');
			return;
		}
		const slot = freeSlot(all, projectId);
		if (slot === null) {
			ui.say('All nine pin slots are used. Unpin a project first.');
			return;
		}
		await c.projectPinSet({ projectId, slot });
		ui.toast(`${me.name} pinned to ⌘${slot}`, 'pass', 'check');
	});
}

export async function openTerminalHere(projectId: string): Promise<void> {
	await guarded(async () => {
		const c = await ipc();
		await c.terminalOpenHere({ projectId });
	});
}

/** Asks Rust to remove the project; Rust shows the native confirmation. */
export async function removeProject(projectId: string): Promise<void> {
	await guarded(async () => {
		const c = await ipc();
		const name = (await c.projectsList({})).find((p) => p.id === projectId)?.name ?? 'The project';
		await c.projectRemove({ projectId });
		ui.toast(`${name} removed. Its folder is untouched.`, 'pass', 'check');
		await goto(to.today());
	});
}
