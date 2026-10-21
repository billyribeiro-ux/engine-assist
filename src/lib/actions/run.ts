// Run a step through the launch pipeline (F-008, ADR-021): prepare, then start or re-arm with the
// plan ID. The webview never approves anything: Rust raises the native confirmation itself and a
// "Cancel" there comes back as `cancelled` (SR-24a). Returns true, or a one-line reason.
import type { IpcClient } from '#lib/ipc/contract.ts';
import { toIpcFailure } from '#lib/ipc/errors.ts';
import { ipc } from '#lib/ipc/index.ts';
import type { EntryPoint, PrepareMode, PrepareOutcome, Surface } from '#lib/ipc/types.ts';
import { refresh } from '#lib/stores/bus.ts';
import { pendingOutput, terminal } from '#lib/stores/terminal.svelte.ts';
import { ui } from '#lib/stores/ui.svelte.ts';
import { archiveOf } from '#lib/terminal/archive.ts';

import { blockerText, confirmText, failureText } from './words.ts';

export interface RunTarget {
	projectId: string;
	step: string;
	surface: Surface | null;
	entry: EntryPoint;
	mode?: PrepareMode;
}

/** Blockers that Doctor explains: it runs again in the background, so its dot is current (AC-F019-07). */
const DOCTOR_BLOCKERS = new Set([
	'claudeMissing',
	'versionBelowFloor',
	'authFailed',
	'authKeyBlocked'
]);

function recheckDoctor(client: IpcClient): void {
	void client.doctorRun({ scope: 'p1' }).catch(() => undefined);
}

async function start(
	client: IpcClient,
	t: RunTarget,
	prep: PrepareOutcome
): Promise<true | string> {
	if (prep.kind === 'blocked') {
		if (prep.blockers.some((b) => DOCTOR_BLOCKERS.has(b.kind))) recheckDoctor(client);
		const first = prep.blockers[0];
		return first ? blockerText(first) : 'This step is blocked.';
	}
	const out = pendingOutput();
	if (prep.rearm?.kind === 'idleConfirmation') {
		const chain = prep.rearm.chain;
		const old = terminal.tabs.find((x) => x.chain === chain && x.status === 'running');
		const res = await client.rearmStart({ planId: prep.planId }, out.cb);
		if (res.kind === 'endYourself')
			return "Claude didn't exit. End it yourself, then press Continue.";
		out.bind(res.runId);
		// AC-F009-06: the old buffer becomes the new tab's collapsed, read-only archive, and the
		// new run starts in a fresh buffer; the old tab goes away (one tab per chain).
		const archived = old
			? await archiveOf(
					old.tabName,
					terminal.chunks(old.runId),
					terminal.colsOf(old.runId),
					terminal.scrollback
				)
			: null;
		terminal.add(res, t.projectId, null, archived);
		if (old) terminal.close(old.runId);
	} else {
		// AC-F008-18: the preview lists why Rust is about to ask. It is context only; the native
		// dialog is the confirmation and `launch_start` carries nothing but the plan ID.
		if (prep.confirm.length)
			ui.toast(`Foreman will ask you to confirm. ${prep.confirm.map(confirmText).join(' ')}`);
		const res = await client.launchStart({ planId: prep.planId }, out.cb);
		out.bind(res.runId);
		terminal.add(res, t.projectId);
	}
	if (!ui.drawerOpen) ui.toggleDrawer();
	refresh(`project:${t.projectId}`, 'today', 'runs');
	ui.announce('Session started in the terminal drawer');
	return true;
}

export async function runStep(t: RunTarget): Promise<true | string> {
	const client = await ipc();
	try {
		if (t.surface === 'chat') {
			const r = await client.chatCopyAndOpen({
				projectId: t.projectId,
				step: t.step,
				entry: t.entry
			});
			refresh(`project:${t.projectId}`, 'today');
			ui.toast(
				r.missingLink === 'project'
					? 'Prompt copied. Set the claude.ai Project link to open a new conversation in one step.'
					: r.missingLink === 'conversation'
						? 'Prompt copied. Link this chat to open the conversation in one step.'
						: r.opened
							? 'Prompt copied and claude.ai opened. Paste it into the chat.'
							: "Prompt copied. The browser didn't open; paste it into the chat.",
				'pass',
				'copy'
			);
			return true;
		}
		if (t.surface === 'desktop') {
			// F-024: the prompt is copied and `claude --desktop` opens Desktop on the step's folder;
			// Billy picks the model, effort and mode there from the chips (AC-F024-01).
			const r = await client.desktopOpen({ projectId: t.projectId, step: t.step });
			refresh(`project:${t.projectId}`, 'today');
			const pick = r.chips.chips
				.filter((c) => c.id !== 'session')
				.map((c) => c.label)
				.join(', ');
			ui.toast(
				`Prompt copied and Claude Desktop opened. Paste it there${pick ? ` and pick ${pick}` : ''}.`,
				'pass',
				'copy'
			);
			return true;
		}
		const prep = await client.launchPrepare({
			projectId: t.projectId,
			step: t.step,
			entry: t.entry,
			mode: t.mode ?? 'normal'
		});
		return await start(client, t, prep);
	} catch (e: unknown) {
		const f = toIpcFailure(e);
		if (f.code === 'cancelled') return 'Cancelled. Nothing was launched.';
		// A launch that failed to start: Doctor looks again, in the background (AC-F019-07).
		if (f.code === 'toolMissing' || f.code === 'toolFailed') recheckDoctor(client);
		return failureText(f);
	}
}

/** Runs an already prepared outcome (Do next, gate Fix this, packet Repair, Resume). */
export async function runPrepared(t: RunTarget, prep: PrepareOutcome): Promise<true | string> {
	const client = await ipc();
	try {
		return await start(client, t, prep);
	} catch (e: unknown) {
		const f = toIpcFailure(e);
		if (f.code === 'toolMissing' || f.code === 'toolFailed') recheckDoctor(client);
		return f.code === 'cancelled' ? 'Cancelled. Nothing was launched.' : failureText(f);
	}
}

/** ⌘⇧↵ from anywhere (AC-F017-02): the top project's next step through the same preflight. */
export async function doNext(): Promise<true | string> {
	const client = await ipc();
	try {
		const out = await client.doNext({});
		if (out.kind === 'nothing') return out.why;
		if (out.kind === 'chat') {
			if (!out.ready)
				return out.blockers[0] ? blockerText(out.blockers[0]) : 'This step is blocked.';
			return await runStep({
				projectId: out.project,
				step: out.step,
				surface: 'chat',
				entry: 'doNext'
			});
		}
		return await runPrepared(
			{ projectId: out.project, step: out.step, surface: 'cli', entry: 'doNext' },
			out.prepare
		);
	} catch (e: unknown) {
		return failureText(toIpcFailure(e));
	}
}

/** Shows a returned reason; true means the action spoke for itself. */
export function report(r: true | string): void {
	if (r !== true) ui.say(r);
}
