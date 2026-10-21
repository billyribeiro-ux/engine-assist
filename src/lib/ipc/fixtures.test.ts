/// <reference types="node" />
// Fixture files (ipc.md §7.1–7.2): every command has `requests/<cmd>.json` (from requests.ts) and
// `responses/<cmd>.json` (the mock's answer to a realistic call), the files Rust round-trips. Run
// with FIXTURES_WRITE=1 to regenerate them; otherwise this test fails if a file drifts.
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

import { COMMAND_MAP, type CommandName, type Requests, type Responses } from './contract.ts';
import { requests, requestVariants } from './fixtures/requests.ts';
import { MockIpcClient, type Scenario } from './mock.ts';
import { PASTE_STAGE1 } from './mock/world.ts';
import type { SettingsView } from './types.ts';

const dir = join(import.meta.dirname, 'fixtures');
const WRITE = process.env.FIXTURES_WRITE === '1';
const FM = 'f0e1d2c3-b4a5-4697-8879-6a5b4c3d2e1f';

type Setup<C extends CommandName> = (m: MockIpcClient) => Promise<Requests[C]>;
interface Case<C extends CommandName> {
	scenario?: Scenario;
	setup?: Setup<C>;
	/** Commands the mock can only refuse: a typed response built from mock state instead. */
	fallback?: (m: MockIpcClient) => Promise<Responses[C]>;
}
type Cases = { [C in CommandName]?: Case<C> };

const r = requests;
const cases: Cases = {
	doctor_login_path_apply: {
		setup: async (m) => ({ previewId: (await m.doctorCaptureLoginPath({})).previewId })
	},
	setup_preview: {
		setup: async (m) => {
			await m.setupPickFolder({});
			const pick = await m.setupPickFolder({});
			return { ...r.setup_preview, pickId: pick.pickId };
		}
	},
	setup_apply: {
		setup: async (m) => {
			await m.setupPickFolder({});
			const pick = await m.setupPickFolder({});
			const pv = await m.setupPreview({ ...r.setup_preview, pickId: pick.pickId });
			return { previewId: pv.previewId };
		}
	},
	blueprint_repin: { scenario: 'stage1' },
	project_remove: { scenario: 'stage4' },
	step_mark_done: { setup: () => Promise.resolve({ projectId: FM, step: '4.5' }) },
	action_apply: {
		scenario: 'stage1',
		setup: async (m) => ({
			previewId: (
				await m.actionPreview({ projectId: r.project_open.projectId, step: 'council-setup' })
			).previewId
		})
	},
	wait_start: {
		fallback: async (m) => {
			const v = await m.stepView(r.step_view);
			return {
				...v,
				summary: { ...v.summary, state: 'waitingUntil', stateText: 'Waiting until 2026-10-26' }
			};
		}
	},
	paste_back_preview: {
		scenario: 'stage1',
		setup: (m) => {
			m.setClipboard(PASTE_STAGE1);
			return Promise.resolve(r.paste_back_preview);
		}
	},
	paste_back_apply: {
		scenario: 'stage1',
		setup: async (m) => {
			m.setClipboard(PASTE_STAGE1);
			return { previewId: (await m.pasteBackPreview(r.paste_back_preview)).previewId };
		}
	},
	chain_resume_prepare: { scenario: 'stage4' },
	launch_start: {
		scenario: 'brokenChain',
		setup: async (m) => {
			await m.configAck(r.config_ack);
			const out = await m.launchPrepare({ ...r.launch_prepare, mode: 'freshWithContext' });
			if (out.kind !== 'ready') throw new Error('expected a ready plan');
			return { planId: out.planId };
		}
	},
	auth_key_allowlist_add: {
		fallback: async (m) => {
			const s: SettingsView = await m.settingsGet({});
			return {
				...s,
				authKeyAllowlist: [
					{
						sha256: 'f1'.repeat(32),
						file: '~/.claude/settings.json',
						key: 'env.ANTHROPIC_BASE_URL',
						addedAt: '2026-10-19T09:00:00Z'
					}
				]
			};
		}
	},
	project_env_approve: { fallback: () => Promise.resolve(null) },
	pty_paste: {
		setup: (m) => {
			m.setClipboard('pnpm view @sveltejs/kit version');
			return Promise.resolve(r.pty_paste);
		}
	},
	rearm_start: {
		setup: async (m) => {
			await m.configAck(r.config_ack);
			const out = await m.launchPrepare({ ...r.launch_prepare, step: 'x-2-1' });
			if (out.kind !== 'ready') throw new Error('expected a ready plan');
			return { planId: out.planId };
		}
	},
	rearm_continue: { scenario: 'stage4' },
	pending_change_accept: {},
	progress_rebuild_apply: {
		setup: async (m) => ({
			previewId: (await m.progressRebuildPreview(r.progress_rebuild_preview)).previewId
		})
	},
	packet_normalize_apply: {
		setup: async (m) => ({
			...r.packet_normalize_apply,
			seenSha256: (await m.packetView(r.packet_view)).sha256 ?? ''
		})
	},
	packet_signature_confirm: {
		setup: async (m) => ({
			...r.packet_signature_confirm,
			seenSha256: (await m.packetView(r.packet_view)).sha256 ?? ''
		})
	},
	tamper_restore: {
		scenario: 'tamper',
		setup: async (m) => ({
			...r.tamper_restore,
			seenSha256: (await m.packetView(r.packet_view)).sha256 ?? ''
		})
	},
	packet_repair_prepare: {
		scenario: 'invalidPacket',
		setup: () => Promise.resolve({ ...r.packet_repair_prepare, stage: 2 })
	},
	runs_clear: { scenario: 'stage4' },
	gate_fix_start: {
		scenario: 'stage5',
		setup: () => Promise.resolve({ ...r.gate_fix_start, stage: 5, item: 'ci-gates' })
	},
	sign_apply: {
		setup: async (m) => ({ previewId: (await m.signPreview(r.sign_preview)).previewId })
	},
	sign_retry_commit: {
		fallback: () => Promise.resolve({ kind: 'signed', commit: '4e1b2c7', stage: 7 })
	},
	assembler_view: { scenario: 'stage4' },
	assembler_paste_preview: {
		scenario: 'stage4',
		setup: (m) => {
			m.setClipboard('# Tickets\n\n## M0 · Site prep');
			return Promise.resolve(r.assembler_paste_preview);
		}
	},
	assembler_paste_apply: {
		scenario: 'stage4',
		setup: async (m) => {
			m.setClipboard('# Tickets\n\n## M0 · Site prep');
			return { previewId: (await m.assemblerPastePreview(r.assembler_paste_preview)).previewId };
		}
	},
	desktop_open: {
		fallback: () =>
			Promise.resolve({
				chips: {
					chips: [{ id: 'model', label: 'Opus 5.5', strong: false }],
					link: null,
					linkLabel: null
				}
			})
	},
	desktop_handoff: { scenario: 'stage4' }
};

function call(m: MockIpcClient, cmd: CommandName, req: unknown): Promise<unknown> {
	const fn = (m as unknown as Record<string, (r: unknown) => Promise<unknown>>)[COMMAND_MAP[cmd]];
	if (!fn) throw new Error(`no method for ${cmd}`);
	return fn.call(m, req);
}

async function responseFor(cmd: CommandName): Promise<unknown> {
	const c = (cases[cmd] ?? {}) as Case<CommandName>;
	const m = new MockIpcClient(c.scenario ?? 'happy');
	m.streamDelayMs = 0;
	if (c.fallback) return c.fallback(m);
	const req = c.setup ? await c.setup(m) : requests[cmd];
	return call(m, cmd, req);
}

function check(file: string, value: unknown): void {
	const text = `${JSON.stringify(value, null, '\t')}\n`;
	if (WRITE) {
		mkdirSync(join(file, '..'), { recursive: true });
		writeFileSync(file, text);
		return;
	}
	expect(existsSync(file), `${file} is missing: run FIXTURES_WRITE=1 pnpm test:unit`).toBe(true);
	expect(JSON.parse(readFileSync(file, 'utf8')), file).toEqual(value);
}

describe('fixtures (ipc.md §7.1)', () => {
	it('every command has a request fixture that matches requests.ts', () => {
		for (const cmd of Object.keys(COMMAND_MAP) as CommandName[])
			check(join(dir, 'requests', `${cmd}.json`), requests[cmd]);
		for (const [name, req] of Object.entries(requestVariants))
			check(join(dir, 'requests', `${name}.json`), req);
	});

	it('every command has a response fixture the mock produces from a realistic call', async () => {
		for (const cmd of Object.keys(COMMAND_MAP) as CommandName[]) {
			if (process.env.FIXTURES_DEBUG) appendFileSync('/tmp/claude-0/dbg/trace.txt', `${cmd}\n`);
			const res = await responseFor(cmd);
			check(join(dir, 'responses', `${cmd}.json`), JSON.parse(JSON.stringify(res)) as unknown);
		}
	});
});
