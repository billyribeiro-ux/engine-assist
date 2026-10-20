// Invalidation bus (ipc.md §6): events become refresh keys; stale revisions are ignored.
import { describe, expect, it } from 'vitest';

import { keysFor, resetBus } from './bus.ts';

describe('bus', () => {
	it('ignores a project revision that is not newer', () => {
		resetBus();
		const id = 'p1';
		expect(keysFor('project:changed', { projectId: id, rev: 3 })).toContain(`project:${id}`);
		expect(keysFor('project:changed', { projectId: id, rev: 3 })).toEqual([]);
		expect(keysFor('project:changed', { projectId: id, rev: 2 })).toEqual([]);
		expect(keysFor('project:changed', { projectId: id, rev: 4 })).toContain('today');
	});

	it('doctor:status refreshes status readers, never Doctor itself (doctor_run emits it)', () => {
		expect(keysFor('doctor:status', { level: 'ok' })).toEqual(['doctorStatus']);
	});
});
