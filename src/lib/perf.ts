// Performance marks (NFR-01, NFR-02, NFR-07; ADR-015). These only measure: the User Timing API
// entries below are what the Playwright trace and the run log's `perf:` lines are compared with,
// and the targets are enforced in M5-09 on the release build.
//
//   fm:cold-start       page origin → first Today render (NFR-01; Rust logs process start → bootstrap)
//   fm:project-switch   a project route starts loading → its screen has painted (NFR-02)
//   fm:long-task        any main-thread task over 50 ms (NFR-07)
//   fm:input            an input whose next paint took over 100 ms (NFR-07)
//
// Nothing here reads content, and nothing leaves the window: marks are in-memory entries.

export const COLD_START = 'fm:cold-start';
export const PROJECT_SWITCH = 'fm:project-switch';
export const LONG_TASK = 'fm:long-task';
export const SLOW_INPUT = 'fm:input';
/** NFR-07: no main-thread task longer than this. */
export const LONG_TASK_MS = 50;
/** NFR-07: input to visual feedback. */
export const INPUT_MS = 100;

export interface PerfReport {
	/** Page origin to first Today render, or null before it. */
	coldStartMs: number | null;
	/** Each completed project switch, in order. */
	projectSwitchMs: number[];
	/** Main-thread tasks over 50 ms seen so far. */
	longTasks: number;
	worstTaskMs: number;
	/** Inputs whose paint took over 100 ms. */
	slowInputs: number;
	worstInputMs: number;
}

/** `durationThreshold` (Event Timing) isn't in the DOM lib's `PerformanceObserverInit` yet. */
type ObserverInit = PerformanceObserverInit & { durationThreshold?: number };

const switches = new Map<string, number>();
let coldDone = false;
const stats = { longTasks: 0, worstTask: 0, slowInputs: 0, worstInput: 0 };
let observing = false;

const now = (): number => {
	try {
		return performance.now();
	} catch {
		return 0;
	}
};

function measure(name: string, start: number, end: number, detail?: unknown): void {
	try {
		performance.measure(name, { start, end, detail });
	} catch {
		// A browser without User Timing level 3: the report below still works from `stats`.
	}
}

/** First Today render: call once the rows are on screen. Later calls do nothing. */
export function markColdStart(): void {
	if (coldDone) return;
	coldDone = true;
	measure(COLD_START, 0, now());
}

/** A project route began loading (a click, a shortcut or a link). */
export function markSwitchStart(projectId: string): void {
	switches.set(projectId, now());
}

/** The project's screen is rendered; the measure ends at its next paint. */
export function markSwitchEnd(projectId: string): void {
	const start = switches.get(projectId);
	if (start === undefined) return;
	switches.delete(projectId);
	const done = () => measure(PROJECT_SWITCH, start, now(), { projectId });
	if (typeof requestAnimationFrame === 'function') requestAnimationFrame(() => done());
	else done();
}

/** Starts the long-task and input observers (NFR-07). Safe to call twice. */
export function observe(): void {
	if (observing || typeof PerformanceObserver === 'undefined') return;
	observing = true;
	const watch = (type: string, init: ObserverInit, on: (e: PerformanceEntry) => void) => {
		try {
			if (!PerformanceObserver.supportedEntryTypes.includes(type)) return;
			new PerformanceObserver((list) => {
				for (const e of list.getEntries()) on(e);
			}).observe({ type, buffered: true, ...init });
		} catch {
			// Not supported in this webview: that signal is simply absent from the report.
		}
	};
	watch('longtask', {}, (e) => {
		if (e.duration <= LONG_TASK_MS) return;
		stats.longTasks += 1;
		stats.worstTask = Math.max(stats.worstTask, e.duration);
		measure(LONG_TASK, e.startTime, e.startTime + e.duration);
	});
	watch('event', { durationThreshold: 16 }, (e) => {
		if (e.duration <= INPUT_MS) return;
		stats.slowInputs += 1;
		stats.worstInput = Math.max(stats.worstInput, e.duration);
		measure(SLOW_INPUT, e.startTime, e.startTime + e.duration, { type: e.name });
	});
}

/** What has been measured so far. */
export function report(): PerfReport {
	const last = (name: string) => performance.getEntriesByName(name).map((e) => e.duration);
	return {
		coldStartMs: last(COLD_START).at(-1) ?? null,
		projectSwitchMs: last(PROJECT_SWITCH),
		longTasks: stats.longTasks,
		worstTaskMs: stats.worstTask,
		slowInputs: stats.slowInputs,
		worstInputMs: stats.worstInput
	};
}

/** Forgets everything (tests). */
export function reset(): void {
	switches.clear();
	coldDone = false;
	stats.longTasks = 0;
	stats.worstTask = 0;
	stats.slowInputs = 0;
	stats.worstInput = 0;
	for (const n of [COLD_START, PROJECT_SWITCH, LONG_TASK, SLOW_INPUT])
		try {
			performance.clearMeasures(n);
		} catch {
			// nothing to clear
		}
}
