import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';

import type { SheetStatus } from '#lib/ipc/types.ts';
import { ui } from '#lib/stores/ui.svelte.ts';

import SheetStrip from './SheetStrip.svelte';

// M5-02 (AC-F030-06): 19 sheets grouped by phase, a status mark by shape and color, the stage name
// on hover, click opens it.
const TITLES = [
	'Land Survey',
	'Meet the Architect',
	"Architect's Design Studio",
	'Permits & Zoning',
	'Construction Drawings',
	'Site Prep & Excavation',
	'Foundation',
	'Framing',
	'Roofing',
	'Plumbing',
	'Electrical',
	'HVAC',
	'Insulation & Drywall',
	'Siding & Exterior',
	'Inspections',
	'Paint & Trim',
	'Final Walkthrough',
	'Certificate of Occupancy',
	'Homeowner Maintenance'
];
const PID = '7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11';

function stages(by: Partial<Record<number, SheetStatus>> = {}, why: Record<number, string> = {}) {
	return TITLES.map((title, stage) => ({
		stage,
		title,
		status: by[stage] ?? 'locked',
		statusWhy: why[stage] ?? null
	}));
}
const ALL: Partial<Record<number, SheetStatus>> = {
	0: 'notApplicable',
	1: 'signed',
	2: 'inProgress',
	3: 'ready',
	4: 'inspecting',
	5: 'waiting',
	6: 'needsAnnotation',
	10: 'undecided'
};

afterEach(() => {
	ui.stampStage = null;
});

describe('SheetStrip', () => {
	// M5-07: the 9 px dimension line clips sideways only; the descenders of "Systems", "Occupancy"
	// and "Ownership" hang below it.
	it('phase labels: descenders are not clipped by the 9 px dimension line', async () => {
		const s = await render(SheetStrip, {
			projectId: PID,
			projectName: 'TradeLog',
			stages: stages(ALL),
			open: 2
		});
		const dims = [...s.container.querySelectorAll<HTMLElement>('.phase-dim')];
		expect(dims).toHaveLength(6);
		for (const d of dims) {
			const st = getComputedStyle(d);
			expect(st.overflowY, d.textContent ?? '').toBe('visible');
			const label = d.querySelector('span');
			// A label that is hidden at this width (the last one, below 920 px) has no box.
			const h = label?.getBoundingClientRect().height ?? 0;
			if (h > 0) expect(h).toBeGreaterThan(d.clientHeight);
		}
	});

	it('primary: 19 sheets in six phase groups, each group named with its stage range', async () => {
		const s = await render(SheetStrip, {
			projectId: PID,
			projectName: 'TradeLog',
			stages: stages(ALL),
			open: 2
		});
		await expect
			.element(s.getByRole('navigation', { name: 'TradeLog drawing set: 19 stages' }))
			.toBeInTheDocument();
		expect(s.getByRole('link').elements()).toHaveLength(19);
		for (const g of [
			'Pre-construction, stages 0 to 4',
			'Structure, stages 5 to 8',
			'Systems, stages 9 to 11',
			'Finishes, stages 12 to 15',
			'Occupancy, stages 16 to 17',
			'Ownership, stages 18 to 18'
		])
			await expect.element(s.getByRole('list', { name: g })).toBeInTheDocument();
	});

	it('every status has words in its name, and a mark whose shape differs from the others', async () => {
		const s = await render(SheetStrip, {
			projectId: PID,
			projectName: 'TradeLog',
			stages: stages(ALL, { 10: 'payments = undecided' }),
			open: 2
		});
		const named = (n: number) => s.getByRole('link', { name: new RegExp(`^Stage ${n},`) });
		const expectName = (n: number, word: string): void =>
			expect(named(n).element().getAttribute('aria-label')).toContain(word);
		expectName(0, 'not applicable');
		expectName(1, 'signed');
		expectName(2, 'in progress');
		expectName(3, 'ready');
		expectName(4, 'inspecting');
		expectName(5, 'packet waiting for your signature');
		expectName(6, 'needs annotation');
		expectName(7, 'locked');
		expectName(10, 'a condition is undecided (payments = undecided)');
		// Shape: the marks that exist are all different for different statuses.
		const shape = (n: number) =>
			[...named(n).element().querySelectorAll('.sheet-mark *')]
				.map((c) => `${c.tagName}:${c.getAttribute('d') ?? c.getAttribute('r') ?? ''}`)
				.join('|');
		const shapes = [1, 2, 3, 4, 10].map(shape);
		expect(shapes.every((x) => x.length > 0)).toBe(true);
		expect(new Set(shapes).size).toBe(shapes.length);
		// Locked, not applicable and needs-annotation are told apart by border and hatch classes.
		expect(named(7).element().classList.contains('is-locked')).toBe(true);
		expect(named(0).element().classList.contains('is-na')).toBe(true);
		expect(named(6).element().classList.contains('is-annot')).toBe(true);
	});

	it('hover shows the stage name and click opens the stage', async () => {
		const s = await render(SheetStrip, {
			projectId: PID,
			projectName: 'TradeLog',
			stages: stages(ALL),
			open: 2
		});
		const link = s.getByRole('link', { name: /^Stage 4,/ });
		expect(link.element().getAttribute('title')).toBe('4 Construction Drawings: inspecting');
		expect(link.element().getAttribute('href')).toContain(`/projects/${PID}/stages/4`);
	});

	it('the open sheet is marked current for assistive technology', async () => {
		const s = await render(SheetStrip, {
			projectId: PID,
			projectName: 'TradeLog',
			stages: stages(ALL),
			open: 2
		});
		await expect
			.element(s.getByRole('link', { name: /^Stage 2,/ }))
			.toHaveAttribute('aria-current', 'page');
		expect(s.container.querySelectorAll('[aria-current="page"]')).toHaveLength(1);
	});

	it('keyboard: one tab stop; ← → Home End move between sheets', async () => {
		const s = await render(SheetStrip, {
			projectId: PID,
			projectName: 'TradeLog',
			stages: stages(ALL),
			open: 2
		});
		const stops = s
			.getByRole('link')
			.elements()
			.filter((e) => e.getAttribute('tabindex') === '0');
		expect(stops).toHaveLength(1);
		(stops[0] as HTMLElement).focus();
		await userEvent.keyboard('{ArrowRight}');
		await expect.element(s.getByRole('link', { name: /^Stage 3,/ })).toHaveFocus();
		await userEvent.keyboard('{ArrowLeft}{ArrowLeft}');
		await expect.element(s.getByRole('link', { name: /^Stage 1,/ })).toHaveFocus();
		await userEvent.keyboard('{End}');
		await expect.element(s.getByRole('link', { name: /^Stage 18,/ })).toHaveFocus();
		await userEvent.keyboard('{Home}');
		await expect.element(s.getByRole('link', { name: /^Stage 0,/ })).toHaveFocus();
		await userEvent.keyboard('{ArrowLeft}');
		await expect.element(s.getByRole('link', { name: /^Stage 0,/ })).toHaveFocus();
	});

	it('wizard preview: the sheets are pictures, not links', async () => {
		const s = await render(SheetStrip, {
			projectId: PID,
			projectName: 'TradeLog',
			stages: stages(ALL),
			open: null,
			preview: true
		});
		expect(s.getByRole('link').elements()).toHaveLength(0);
		expect(s.getByRole('img').elements()).toHaveLength(19);
	});

	it('the stamp lands on the sheet that was just signed, and only on that one', async () => {
		ui.stampStage = { projectId: PID, stage: 2 };
		const s = await render(SheetStrip, {
			projectId: PID,
			projectName: 'TradeLog',
			stages: stages({ ...ALL, 2: 'signed', 3: 'ready' }),
			open: 3
		});
		const landing = (n: number) =>
			s
				.getByRole('link', { name: new RegExp(`^Stage ${n},`) })
				.element()
				.querySelector('.stamp.is-landing');
		expect(landing(2)).not.toBeNull();
		expect(landing(1)).toBeNull();
		// The next stage turns ready in the same render.
		expect(
			s
				.getByRole('link', { name: /^Stage 3,/ })
				.element()
				.getAttribute('aria-label')
		).toContain('ready');
	});
});
