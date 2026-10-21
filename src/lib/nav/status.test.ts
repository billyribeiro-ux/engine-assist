// AC-XC-25 (M5-05): every status is shape plus color plus words, never color alone. status.ts is the
// one table the UI reads; this pins its shape. The rendered check (no bare color mark on any screen,
// both themes) is tests/flows/status.spec.ts.
import { describe, expect, it } from 'vitest';

import { ICONS } from '#lib/components/icons.ts';

import { DOCTOR, PROJECT, RUN, SHEET, STEP, type Look } from './status.ts';

const FAMILIES: Record<string, Record<string, Look>> = { SHEET, STEP, RUN, DOCTOR, PROJECT };

/** Pairs that share a shape and differ by color and words (components.md "Sheet strip" table). */
const SAME_SHAPE_DIFFERENT_COLOR = new Set(['SHEET:inspecting/waiting']);
/** Pairs that are the same look in two moments of one run (starting, then running). */
const SAME_LOOK = new Set(['RUN:starting/running']);

describe('status looks', () => {
	for (const [family, looks] of Object.entries(FAMILIES)) {
		it(`${family}: every status has a drawn shape, a tone and words`, () => {
			for (const [status, look] of Object.entries(looks)) {
				expect(ICONS[look.icon], `${family}.${status} icon`).toBeDefined();
				expect(look.tone, `${family}.${status} tone`).toBeTruthy();
				expect(look.word.trim().length, `${family}.${status} words`).toBeGreaterThan(2);
			}
		});

		it(`${family}: statuses that differ in color differ in shape`, () => {
			const entries = Object.entries(looks);
			for (const [i, [a, la]] of entries.entries())
				for (const [b, lb] of entries.slice(i + 1)) {
					if (la.icon !== lb.icon) continue;
					const key = `${family}:${a}/${b}`;
					if (la.tone === lb.tone) {
						expect(SAME_LOOK.has(key), `${key} share shape and color`).toBe(true);
					} else {
						expect(SAME_SHAPE_DIFFERENT_COLOR.has(key), `${key} differ by color alone`).toBe(true);
					}
				}
		});
	}

	it('no family leans on a color-only mark: nothing maps to a bare dot except in-progress states', () => {
		for (const [family, looks] of Object.entries(FAMILIES))
			for (const [status, look] of Object.entries(looks))
				if (look.icon === 'dot')
					expect(['inProgress', 'running', 'starting'], `${family}.${status}`).toContain(status);
	});

	it('--attention is only for things waiting on you (components.md principles)', () => {
		// A tone of attention names a state that needs Billy: waiting, copied, undecided, blocked,
		// warnings, needs a newer Foreman.
		const allowed = new Set([
			'waiting',
			'copied',
			'undecided',
			'blocked',
			'warning',
			'needsNewerForeman'
		]);
		for (const [family, looks] of Object.entries(FAMILIES))
			for (const [status, look] of Object.entries(looks))
				if (look.tone === 'attention')
					expect(allowed.has(status), `${family}.${status} is attention`).toBe(true);
	});

	it('the sidebar project levels are three different shapes', () => {
		expect(new Set(Object.values(PROJECT).map((l) => l.icon)).size).toBe(3);
	});
});
