import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';

import type { ProjectView, StageView } from '#lib/ipc/types.ts';
import { fresh, IDS, project } from '#lib/test/screen.ts';

import TitleBlock from './TitleBlock.svelte';

// M5-03 (AC-F030-07): sheet number, revision (the pinned blueprint), DRI, status and date signed.
async function setup(stageNo: number): Promise<{ p: ProjectView; st: StageView }> {
	const m = fresh('happy');
	const p = await project(m, IDS.tradelog);
	const st = p.stages.find((s) => s.stage === stageNo);
	if (!st) throw new Error('the happy world has this stage');
	return { p, st };
}
const label = (s: Awaited<ReturnType<typeof render>>) =>
	s.getByRole('group').element().getAttribute('aria-label') ?? '';

describe('TitleBlock', () => {
	it('primary: sheet, project, revision, DRI and status, all in the accessible name', async () => {
		const { p, st } = await setup(2);
		const s = await render(TitleBlock, { project: p, stage: st });
		const name = label(s);
		expect(name).toContain('sheet 2 of 19');
		expect(name).toContain(`project ${p.summary.name}`);
		expect(name).toContain(`revision Blueprint ${p.blueprint.version}, ${p.blueprint.commit}`);
		expect(name).toContain(`DRI ${p.dri ?? ''}`);
		expect(name).toMatch(/status In progress, \d+ of \d+/);
		await expect.element(s.getByText('Blueprint 2.2, a3f9c1')).toBeInTheDocument();
	});

	it('signed: the status is the date signed, with the stamp glyph', async () => {
		const { p, st } = await setup(1);
		expect(st.signed).not.toBeNull();
		const s = await render(TitleBlock, { project: p, stage: st });
		expect(label(s)).toContain(`status Signed ${st.signed?.date ?? ''}`);
		expect(s.container.querySelector('.stamp')).not.toBeNull();
	});

	it('signed on another branch and not merged says so', async () => {
		const { p, st } = await setup(1);
		if (!st.signed) throw new Error('stage 1 is signed in the happy world');
		const signed = { ...st.signed, merged: false };
		const s = await render(TitleBlock, {
			project: p,
			stage: { ...st, signed: { ...signed, branch: 'stage/01-meet' } }
		});
		expect(label(s)).toContain('Signed on stage/01-meet, not merged');
	});

	it('locked, not applicable and needs annotation are named, not left blank', async () => {
		const { p, st } = await setup(3);
		for (const [status, word] of [
			['locked', 'Locked'],
			['notApplicable', 'Not applicable'],
			['needsAnnotation', 'Needs annotation']
		] as const) {
			const s = await render(TitleBlock, {
				project: p,
				stage: { ...st, status, signed: null, steps: [] }
			});
			expect(label(s)).toContain(`status ${word}`);
			await s.unmount();
		}
	});

	it('inspecting counts the steps done, and an unset DRI is said so', async () => {
		const { p, st } = await setup(2);
		const s = await render(TitleBlock, {
			project: { ...p, dri: null },
			stage: { ...st, status: 'inspecting' }
		});
		expect(label(s)).toMatch(/status Inspecting, \d+ of \d+/);
		expect(label(s)).toContain('DRI not set');
	});

	it('long values truncate with a tooltip that holds the full text', async () => {
		const { p, st } = await setup(2);
		const long = 'A very long driver name that will not fit in a drafting box on a narrow window';
		const s = await render(TitleBlock, { project: { ...p, dri: long }, stage: st });
		expect(s.container.querySelector(`[title="${long}"]`)).not.toBeNull();
	});
});
