import { afterEach, describe, expect, it, vi } from 'vitest';

import { fresh } from '#lib/test/screen.ts';

import { QUERIES, watchOsDisplay } from './os-display.ts';

// M5-04 (AC-XC-23): Reduce Transparency and Increase Contrast are detected by media query in the
// webview and reported to Rust, which turns window effects off.
class FakeQuery extends EventTarget {
	matches: boolean;
	constructor(matches: boolean) {
		super();
		this.matches = matches;
	}
	flip(on: boolean): void {
		this.matches = on;
		this.dispatchEvent(new Event('change'));
	}
}

function fakeMedia(initial: Partial<Record<string, boolean>> = {}) {
	const queries = new Map<string, FakeQuery>();
	vi.spyOn(window, 'matchMedia').mockImplementation((q: string) => {
		let f = queries.get(q);
		if (!f) {
			f = new FakeQuery(initial[q] ?? false);
			queries.set(q, f);
		}
		return f as unknown as MediaQueryList;
	});
	return queries;
}

afterEach(() => vi.restoreAllMocks());

describe('watchOsDisplay', () => {
	it('primary: reports both settings at start, with nothing else in the patch', async () => {
		const m = fresh('happy');
		fakeMedia({ [QUERIES.reduceTransparency]: true });
		const update = vi.spyOn(m, 'settingsUpdate');
		const stop = watchOsDisplay();
		await vi.waitFor(() => expect(update).toHaveBeenCalledTimes(1));
		const patch = update.mock.calls[0]?.[0].patch;
		expect(patch?.osDisplay).toEqual({ reduceTransparency: true, increaseContrast: false });
		expect(Object.entries(patch ?? {}).filter(([k, v]) => k !== 'osDisplay' && v !== null)).toEqual(
			[]
		);
		stop();
	});

	it('reports again when Increase Contrast or Reduce Transparency changes', async () => {
		const m = fresh('happy');
		const q = fakeMedia();
		const update = vi.spyOn(m, 'settingsUpdate');
		const stop = watchOsDisplay();
		await vi.waitFor(() => expect(update).toHaveBeenCalledTimes(1));
		q.get(QUERIES.increaseContrast)?.flip(true);
		await vi.waitFor(() => expect(update).toHaveBeenCalledTimes(2));
		expect(update.mock.calls[1]?.[0].patch.osDisplay).toEqual({
			reduceTransparency: false,
			increaseContrast: true
		});
		q.get(QUERIES.reduceTransparency)?.flip(true);
		await vi.waitFor(() => expect(update).toHaveBeenCalledTimes(3));
		expect(update.mock.calls[2]?.[0].patch.osDisplay).toEqual({
			reduceTransparency: true,
			increaseContrast: true
		});
		stop();
	});

	it('stopping stops the reports', async () => {
		const m = fresh('happy');
		const q = fakeMedia();
		const update = vi.spyOn(m, 'settingsUpdate');
		const stop = watchOsDisplay();
		await vi.waitFor(() => expect(update).toHaveBeenCalledTimes(1));
		stop();
		q.get(QUERIES.increaseContrast)?.flip(true);
		await new Promise((r) => setTimeout(r, 50));
		expect(update).toHaveBeenCalledTimes(1);
	});

	it('error: a failing update is swallowed (display state is never worth a toast)', async () => {
		const m = fresh('happy');
		fakeMedia();
		vi.spyOn(m, 'settingsUpdate').mockRejectedValue(new Error('disk is full'));
		const stop = watchOsDisplay();
		await new Promise((r) => setTimeout(r, 50));
		stop();
	});
});
