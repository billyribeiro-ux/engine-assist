import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';

import type { ProjectView, StageView } from '#lib/ipc/types.ts';
import { ui } from '#lib/stores/ui.svelte.ts';
import { fresh, IDS, project } from '#lib/test/screen.ts';

import Inspector from './Inspector.svelte';

// M5-03 (AC-F030-08): the inspector shows the selected step's, gate's or packet's context; ⌘⌥I
// collapses it.
async function setup(stageNo: number): Promise<{ p: ProjectView; st: StageView }> {
	const m = fresh('happy');
	const p = await project(m, IDS.tradelog);
	const st = p.stages.find((s) => s.stage === stageNo);
	if (!st) throw new Error('the happy world has this stage');
	return { p, st };
}

afterEach(() => {
	ui.inspectorOpen = true;
	ui.inspectorOverlay = false;
});

describe('Inspector', () => {
	it('primary: a labelled complementary region with Packet, Gate and Lessons tabs; Gate first', async () => {
		const { p, st } = await setup(2);
		const s = await render(Inspector, { project: p, stage: st });
		await expect.element(s.getByRole('complementary', { name: 'Inspector' })).toBeInTheDocument();
		const tabs = s.getByRole('tab').elements();
		expect(tabs.map((t) => t.textContent)).toEqual(['Packet', 'Gate', 'Lessons']);
		await expect
			.element(s.getByRole('tab', { name: 'Gate' }))
			.toHaveAttribute('aria-selected', 'true');
		await expect
			.element(s.getByRole('heading', { name: 'Stage 2 inspection' }))
			.toBeInTheDocument();
		await expect
			.element(s.getByText(/The feature matrix covers every serious competitor/))
			.toBeInTheDocument();
	});

	it('the packet tab tells you how the packet gets written when there is none', async () => {
		const { p, st } = await setup(2);
		const s = await render(Inspector, { project: p, stage: st, initial: 'packet' });
		await expect.element(s.getByText('No Stage 2 packet yet')).toBeInTheDocument();
		await expect.element(s.getByText(/writes docs\/handoff\/stage-02\.md/)).toBeInTheDocument();
	});

	it('a chat step shows the knowledge for its chat', async () => {
		const { p, st } = await setup(1);
		const step = st.steps.find((x) => x.surface === 'chat') ?? null;
		const s = await render(Inspector, { project: p, stage: st, step, initial: 'packet' });
		await expect
			.element(s.getByRole('heading', { name: 'Knowledge for this chat' }))
			.toBeInTheDocument();
	});

	it('empty: the lessons tab invites the next action', async () => {
		const { p, st } = await setup(2);
		const s = await render(Inspector, { project: p, stage: st, initial: 'lessons' });
		await expect.element(s.getByText('No lessons for Stage 2 yet')).toBeInTheDocument();
	});

	it('keyboard: ← → move between tabs and the arrow key selects it', async () => {
		const { p, st } = await setup(2);
		const s = await render(Inspector, { project: p, stage: st });
		(s.getByRole('tab', { name: 'Gate' }).element() as HTMLElement).focus();
		await userEvent.keyboard('{ArrowRight}');
		await expect.element(s.getByRole('tab', { name: 'Lessons' })).toHaveFocus();
		await expect
			.element(s.getByRole('tab', { name: 'Lessons' }))
			.toHaveAttribute('aria-selected', 'true');
		await userEvent.keyboard('{ArrowRight}');
		await expect.element(s.getByRole('tab', { name: 'Packet' })).toHaveFocus();
		await userEvent.keyboard('{ArrowLeft}');
		await expect.element(s.getByRole('tab', { name: 'Lessons' })).toHaveFocus();
		// One tab stop: only the selected tab is in the tab order.
		const stops = s
			.getByRole('tab')
			.elements()
			.filter((t) => t.getAttribute('tabindex') === '0');
		expect(stops).toHaveLength(1);
	});

	it('⌘⌥I collapses it and brings it back', async () => {
		const { p, st } = await setup(2);
		const s = await render(Inspector, { project: p, stage: st });
		const aside = s.container.querySelector('aside.inspector');
		expect(aside?.classList.contains('closed')).toBe(false);
		ui.toggleInspector();
		await expect.poll(() => aside?.classList.contains('closed')).toBe(window.innerWidth >= 1360);
		ui.toggleInspector();
		await expect.poll(() => aside?.classList.contains('closed')).toBe(false);
	});

	it('narrow windows bring it back as an overlay', async () => {
		const { p, st } = await setup(2);
		ui.inspectorOverlay = true;
		const s = await render(Inspector, { project: p, stage: st });
		await expect
			.poll(() =>
				s.getByRole('complementary', { name: 'Inspector' }).element().classList.contains('forced')
			)
			.toBe(true);
	});
});
