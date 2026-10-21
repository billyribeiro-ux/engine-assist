import { beforeEach, describe, expect, it } from 'vitest';

import {
	COLD_START,
	INPUT_MS,
	LONG_TASK_MS,
	markColdStart,
	markSwitchEnd,
	markSwitchStart,
	PROJECT_SWITCH,
	report,
	reset
} from './perf.ts';

// M2-27 (NFR-01, NFR-02, NFR-07): the frontend marks exist and are reported; targets are M5-09.
beforeEach(() => reset());

describe('perf marks', () => {
	it('NFR-01: the first Today render is measured from the page origin, once', () => {
		expect(report().coldStartMs).toBeNull();
		markColdStart();
		markColdStart();
		const r = report();
		expect(r.coldStartMs).toBeGreaterThan(0);
		expect(performance.getEntriesByName(COLD_START)).toHaveLength(1);
	});

	it('NFR-02: a project switch is measured from its start to the next paint', async () => {
		markSwitchStart('p1');
		await new Promise((r) => setTimeout(r, 20));
		markSwitchEnd('p1');
		const r = report();
		expect(r.projectSwitchMs).toHaveLength(1);
		expect(r.projectSwitchMs[0]).toBeGreaterThanOrEqual(19);
		expect(
			(performance.getEntriesByName(PROJECT_SWITCH)[0] as PerformanceMeasure | undefined)?.detail
		).toEqual({
			projectId: 'p1'
		});
	});

	it('a switch that never started is not invented', () => {
		markSwitchEnd('ghost');
		expect(report().projectSwitchMs).toEqual([]);
	});

	it('NFR-07: the budgets are 50 ms per task and 100 ms to visual feedback', () => {
		expect(LONG_TASK_MS).toBe(50);
		expect(INPUT_MS).toBe(100);
		expect(report()).toMatchObject({ longTasks: 0, slowInputs: 0 });
	});
});
