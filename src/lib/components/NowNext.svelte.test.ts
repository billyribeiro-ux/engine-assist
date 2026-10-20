import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';

import type { ProjectView, StepView } from '#lib/ipc/types.ts';
import { fresh, IDS, project } from '#lib/test/screen.ts';

import NowNext from './NowNext.svelte';

// M4-11 (F-009 P2): the Now/Next strip and the round tracker against the mock. The happy world
// has 2.3 running in the council chain.
function withSpec(p: ProjectView): ProjectView {
	const q = structuredClone(p);
	for (const st of q.stages.flatMap((s) => s.steps))
		if (st.state === 'running' && st.specLine) {
			st.specLine.mode = { value: 'Plan', origin: { kind: 'template' }, from: null };
			st.specLine.model = { value: 'Opus 5.5', origin: { kind: 'template' }, from: null };
			st.specLine.effort = { value: 'xhigh', origin: { kind: 'template' }, from: null };
		}
	return q;
}

async function viewWith(
	m: ReturnType<typeof fresh>,
	patch: Partial<StepView>
): Promise<ProjectView> {
	const p = withSpec(await project(m, IDS.tradelog));
	const real = m.stepView.bind(m);
	vi.spyOn(m, 'stepView').mockImplementation(async (req) => ({ ...(await real(req)), ...patch }));
	return p;
}

/** The strip's text, whitespace collapsed (non-breaking spaces included). */
async function stripText(s: Awaited<ReturnType<typeof render>>): Promise<string> {
	const strip = s.getByRole('status', { name: 'Now and next' });
	await expect.element(strip).toBeVisible();
	return strip.element().textContent.replace(/\s+/g, ' ').trim();
}

describe('NowNext', () => {
	it('AC-F009-11: a running step with a switch shows Now and the in-step cue', async () => {
		const m = fresh('happy');
		const p = await viewWith(m, {
			switches: [{ text: 'Next: after you approve the plan, choose Accept edits' }],
			live: null
		});
		const s = await render(NowNext, { project: p });
		await vi.waitFor(async () =>
			expect(await stripText(s)).toMatch(
				/^Now Plan · Opus 5\.5 · xhigh Next after you approve the plan, choose Accept edits/
			)
		);
	});

	it('AC-F009-11: with the sensor the live mode shows; a match is not flagged', async () => {
		const m = fresh('happy');
		const p = await viewWith(m, {
			switches: [{ text: 'Next: after you approve the plan, choose Accept edits' }],
			live: { mode: 'Accept edits', expected: null }
		});
		const s = await render(NowNext, { project: p });
		await vi.waitFor(async () =>
			expect(await stripText(s)).toMatch(/^Now Accept edits · Opus 5\.5 · xhigh Next/)
		);
		await expect.element(s.getByText(/expected/)).not.toBeInTheDocument();
	});

	it('AC-F009-11: a live mode that differs from the step is in the attention state', async () => {
		const m = fresh('happy');
		const p = await viewWith(m, { live: { mode: 'Auto', expected: 'Plan' } });
		const s = await render(NowNext, { project: p });
		await expect.element(s.getByText('Plan expected')).toBeVisible();
		const now = s.getByText(/Auto · Opus 5\.5 · xhigh/);
		await expect.element(now).toBeVisible();
		expect(now.element().closest('.now')?.classList.contains('attention')).toBe(true);
	});

	it('without a running step only Next shows', async () => {
		const m = fresh('happy');
		const p = structuredClone(await project(m, IDS.tradelog));
		for (const st of p.stages.flatMap((x) => x.steps))
			if (st.state === 'running') st.state = 'done';
		const s = await render(NowNext, { project: p });
		await vi.waitFor(async () => expect(await stripText(s)).toMatch(/^Next 2\.4 /));
		expect(await stripText(s)).not.toContain('Now');
	});

	it('AC-F009-12: the round tracker counts written outputs and opens each', async () => {
		const m = fresh('happy');
		const open = vi.spyOn(m, 'fileOpen');
		const p = await viewWith(m, {
			outs: [
				{ path: 'spec/council/r1-market.md', exists: true },
				{ path: 'spec/council/r1-users.md', exists: true },
				{ path: 'spec/council/r2-features.md', exists: false }
			]
		});
		const s = await render(NowNext, { project: p });
		await expect.element(s.getByText('2 of 3 written')).toBeVisible();
		const b = s.getByRole('button', { name: 'Open output spec/council/r1-users.md' });
		await expect.element(b).toBeVisible();
		await expect
			.element(s.getByRole('button', { name: 'Open output spec/council/r2-features.md' }))
			.not.toBeInTheDocument();
		await b.click();
		await vi.waitFor(() =>
			expect(open).toHaveBeenCalledWith({
				projectId: IDS.tradelog,
				path: 'spec/council/r1-users.md'
			})
		);
	});
});
