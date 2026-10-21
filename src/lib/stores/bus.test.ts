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

	// Code review C1: project_open emitted tamper:detected on every open and the refetch it caused
	// opened again; the rule now holds for every event that carries a revision.
	it('bus::rev_rule_on_packet_and_tamper_events', () => {
		resetBus();
		const id = 'p1';
		const tamper = { projectId: id, stage: 1, rev: 7 };
		expect(keysFor('tamper:detected', tamper)).toContain(`project:${id}`);
		expect(keysFor('tamper:detected', tamper)).toEqual([]);
		// Another stage is its own sequence.
		expect(keysFor('tamper:detected', { ...tamper, stage: 2 })).toContain(`packet:${id}:2`);
		expect(keysFor('tamper:detected', { ...tamper, rev: 8 })).toContain('today');
		const packet = { projectId: id, stage: 3, rev: 41 };
		expect(keysFor('packet:changed', packet)).toContain(`packet:${id}:3`);
		expect(keysFor('packet:changed', packet)).toEqual([]);
		expect(keysFor('packet:changed', { ...packet, rev: 40 })).toEqual([]);
		expect(keysFor('packet:changed', { ...packet, stage: 4, rev: 42 })).toContain(`packet:${id}:4`);
	});

	it('doctor:status refreshes status readers, never Doctor itself (doctor_run emits it)', () => {
		expect(keysFor('doctor:status', { level: 'ok' })).toEqual(['doctorStatus']);
	});
});
