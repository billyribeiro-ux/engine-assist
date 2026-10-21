import { describe, expect, it } from 'vitest';

import { IpcFailure, isIpcError, toIpcFailure } from './errors.ts';

describe('toIpcFailure', () => {
	it('passes typed errors through', () => {
		const e = { code: 'notFound', message: 'x', detail: null } as const;
		const f = toIpcFailure(e);
		expect(f).toBeInstanceOf(IpcFailure);
		expect(f).toMatchObject(e);
		expect(toIpcFailure(f)).toBe(f);
	});

	it('turns anything else into internal without echoing it', () => {
		const e = toIpcFailure('missing required key req with secret=abc');
		expect(e.code).toBe('internal');
		expect(e.message).not.toContain('secret');
		expect(isIpcError({ code: 'bogus', message: 'x' })).toBe(false);
	});
});
