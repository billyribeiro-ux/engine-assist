import { describe, expect, it, vi } from 'vitest';

import { fresh, IDS, said } from '#lib/test/screen.ts';

import { runPrepared, runStep } from './run.ts';

// M2-25 (AC-F008-18, comp): the preview lists the reasons Rust will ask about, as context, and
// `launch_start` carries nothing but the plan ID. The confirmation itself is Rust's native dialog.
const addedStep = async () => {
	const m = fresh('happy');
	await m.addStep({
		projectId: IDS.tradelog,
		stage: 2,
		after: '2.6',
		copyConfigFrom: '2.2',
		title: 'Name check',
		text: 'Check the names.',
		config: {
			surface: 'cli',
			cwd: 'spec',
			start: { model: 'opus', effort: 'high', mode: 'plan' },
			out: []
		},
		tag: 'optional'
	});
	// The spec/ configuration notice is its own blocker; acknowledge it so the plan can start.
	const view = await m.projectOpen({ projectId: IDS.tradelog });
	if (view.configChanged)
		await m.configAck({ projectId: IDS.tradelog, fingerprint: view.configChanged.fingerprint });
	return m;
};

describe('run: launch preview context', () => {
	it('an added step names why Rust will ask; the start request is only { planId }', async () => {
		const m = await addedStep();
		const start = vi.spyOn(m, 'launchStart');
		const out = await runStep({
			projectId: IDS.tradelog,
			step: 'x-2-2',
			surface: 'cli',
			entry: 'stageView'
		});
		expect(out).toBe(true);
		expect(said()).toContain('Foreman will ask you to confirm.');
		expect(said()).toContain('This step was added for this project.');
		expect(start).toHaveBeenCalledTimes(1);
		const first = start.mock.calls[0]?.[0];
		expect(Object.keys(first ?? {})).toEqual(['planId']);
	});

	it('a plan Rust will not ask about adds no context line', async () => {
		const m = await addedStep();
		const prep = await m.launchPrepare({
			projectId: IDS.tradelog,
			step: 'x-2-2',
			entry: 'stageView',
			mode: 'normal'
		});
		expect(prep.kind).toBe('ready');
		if (prep.kind !== 'ready') return;
		const out = await runPrepared(
			{ projectId: IDS.tradelog, step: 'x-2-2', surface: 'cli', entry: 'stageView' },
			{ ...prep, confirm: [] }
		);
		expect(out).toBe(true);
		expect(said()).not.toContain('Foreman will ask you to confirm.');
	});
});
