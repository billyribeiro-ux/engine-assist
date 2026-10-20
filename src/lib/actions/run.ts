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

import { blockerText, failureText } from './words.ts';

export interface RunTarget {
	projectId: string;
	step: string;
	surface: Surface | null;
	entry: EntryPoint;
	mode?: PrepareMode;
}

async function start(
	client: IpcClient,
	t: RunTarget,
	prep: PrepareOutcome
): Promise<true | string> {
	if (prep.kind === 'blocked') {
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
		if (old) terminal.setStatus(old.runId, 'exited', 0);
		terminal.add(
			res,
			t.projectId,
			null,
			old ? { label: old.tabName, lines: terminal.lines(old.runId) } : null
		);
	} else {
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
