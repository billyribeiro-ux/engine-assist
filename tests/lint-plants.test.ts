/// <reference types="node" />
// M0-03: each planted security-lint violation is reported by ESLint.
import { describe, expect, it } from 'vitest';

import { runEslintPlants } from '../scripts/lint-plants/run.mjs';

describe('lint plants', () => {
	it('ESLint catches every planted violation', { timeout: 120_000 }, async () => {
		const results = await runEslintPlants();
		expect(results).toHaveLength(5);
		for (const r of results) {
			expect(r.got, r.target).toEqual(expect.arrayContaining(r.expected));
			expect(r.ok, r.target).toBe(true);
		}
	});
});
