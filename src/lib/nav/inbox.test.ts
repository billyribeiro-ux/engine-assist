// M4-25 (AC-F017-07, AC-F017-10): the inbox order and the sidebar's per-project level.
import { describe, expect, it } from 'vitest';

import type { InboxItem } from '#lib/ipc/types.ts';

import { INBOX_RANK, projectLevel, sortInbox } from './inbox.ts';

const A = '0c6d2f5b-8407-4e1b-8b3d-5f94c3e7a104';
const B = '7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11';

const item = (kind: InboxItem['kind'], project = A, title: string = kind): InboxItem => ({
	kind,
	project,
	step: null,
	stage: null,
	at: '2026-10-20T12:00:00Z',
	projectName: 'TradeLog',
	title,
	detail: '',
	chain: null,
	run: null
});

const SPEC_ORDER: InboxItem['kind'][] = [
	'sessionBlocked',
	'sessionYourTurn',
	'packetLanded',
	'packetInvalid',
	'gateFailed',
	'gateSignable',
	'outsideChange',
	'tamper',
	'chatAwaitingPaste',
	'waitElapsed',
	'interruptedSession',
	'stepReady'
];

describe('inbox order', () => {
	it('AC-F017-07: every kind lands in the specified order, however the list arrives', () => {
		const shuffled = [...SPEC_ORDER].reverse().map((k) => item(k));
		const sorted = sortInbox(shuffled).map((i) => i.kind);
		// Kinds sharing a rank keep their arrival order, so compare the ranks and the set.
		expect(sorted.map((k) => INBOX_RANK[k])).toEqual(SPEC_ORDER.map((k) => INBOX_RANK[k]));
		expect([...sorted].sort()).toEqual([...SPEC_ORDER].sort());
		const ranks = SPEC_ORDER.map((k) => INBOX_RANK[k]);
		expect([...ranks].sort((a, b) => a - b)).toEqual(ranks);
	});

	it('packet-landed and packet-invalid share a rank, as do gate-failed and gate-signable; ties keep their order', () => {
		expect(INBOX_RANK.packetLanded).toBe(INBOX_RANK.packetInvalid);
		expect(INBOX_RANK.gateFailed).toBe(INBOX_RANK.gateSignable);
		const out = sortInbox([
			item('gateSignable', A, 'first'),
			item('packetInvalid', A, 'second'),
			item('packetLanded', B, 'third'),
			item('gateFailed', B, 'fourth')
		]);
		expect(out.map((i) => i.title)).toEqual(['second', 'third', 'first', 'fourth']);
	});

	it('does not change the list it is given', () => {
		const input = [item('stepReady'), item('tamper')];
		sortInbox(input);
		expect(input.map((i) => i.kind)).toEqual(['stepReady', 'tamper']);
	});
});

describe('project level (AC-F017-10)', () => {
	it('a failed gate, an invalid packet, tamper or a read-only project is a problem', () => {
		for (const k of ['gateFailed', 'packetInvalid', 'tamper'] as const)
			expect(projectLevel([item(k)], A, false), k).toBe('problem');
		expect(projectLevel([], A, true)).toBe('problem');
	});

	it('anything waiting on Billy is attention, and only for its own project', () => {
		expect(projectLevel([item('sessionBlocked')], A, false)).toBe('waiting');
		expect(projectLevel([item('gateSignable')], A, false)).toBe('waiting');
		expect(projectLevel([item('gateSignable')], B, false)).toBe('clear');
	});

	it('nothing in the inbox, or only a ready step, is all clear', () => {
		expect(projectLevel([], A, false)).toBe('clear');
		expect(projectLevel([item('stepReady')], A, false)).toBe('clear');
	});
});
