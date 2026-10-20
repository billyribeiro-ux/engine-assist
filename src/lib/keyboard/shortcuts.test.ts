// M2-26 / M4-33 (AC-F028-01, AC-F028-03): one shortcut table, one matcher, layered handlers.
import { describe, expect, it } from 'vitest';

import {
	type KeyLike,
	handlerFor,
	match,
	NOT_HERE,
	registerShortcuts,
	SHORTCUTS
} from './registry.ts';

const key = (k: string, mods: Partial<KeyLike> = {}): KeyLike => ({
	key: k,
	metaKey: false,
	ctrlKey: false,
	altKey: false,
	shiftKey: false,
	...mods
});
const cmd = (k: string, mods: Partial<KeyLike> = {}) => key(k, { metaKey: true, ...mods });

describe('shortcuts', () => {
	it('maps every A.7 shortcut and the additions', () => {
		const cases: [KeyLike, string, number?][] = [
			[cmd('k'), 'palette'],
			[cmd('Enter'), 'run'],
			[cmd('C', { shiftKey: true }), 'copyPrompt'],
			[cmd('ç', { altKey: true, code: 'KeyC' }), 'copyCommand'],
			[cmd(']'), 'nextStage'],
			[cmd('['), 'prevStage'],
			[cmd('g'), 'jumpStage'],
			[key('`', { ctrlKey: true, code: 'Backquote' }), 'toggleDrawer'],
			[cmd('ˆ', { altKey: true, code: 'KeyI' }), 'toggleInspector'],
			[cmd('G', { shiftKey: true }), 'openGate'],
			[cmd('n'), 'newProject'],
			[cmd(','), 'settings'],
			[cmd('Enter', { shiftKey: true }), 'doNext'],
			[cmd('V', { shiftKey: true }), 'pasteBack'],
			[cmd('d'), 'markDone'],
			[cmd('T', { shiftKey: true }), 'focusMode'],
			[cmd('E', { shiftKey: true }), 'returnFocus'],
			[cmd('3'), 'pinned', 3],
			[cmd('9'), 'pinned', 9]
		];
		for (const [e, id, arg] of cases)
			expect(match(e, true), id).toEqual(arg === undefined ? { id } : { id, arg });
		const listed = new Set(SHORTCUTS.map((s) => s.id));
		for (const [, id] of cases) expect(listed.has(id as never), id).toBe(true);
	});

	it('plain J/K and arrows move only when plain keys are allowed (not while typing)', () => {
		expect(match(key('j'), true)).toEqual({ id: 'nextStep' });
		expect(match(key('ArrowUp'), true)).toEqual({ id: 'prevStep' });
		expect(match(key('j'), false)).toBeNull();
		expect(match(key('j', { altKey: true }), true)).toBeNull();
		expect(match(cmd('0'), true)).toBeNull();
		expect(match(cmd('x', { altKey: true }), true)).toBeNull();
	});

	it('the newest layer wins and unregistering restores the one below', () => {
		const app = () => 'app' as const;
		const screen = () => true as const;
		const offApp = registerShortcuts({ run: app, markDone: app });
		const offScreen = registerShortcuts({ run: screen });
		expect(handlerFor('run')).toBe(screen);
		expect(handlerFor('markDone')).toBe(app);
		offScreen();
		expect(handlerFor('run')).toBe(app);
		offApp();
		expect(handlerFor('run')).toBeUndefined();
	});

	it('every action that can be unavailable has a one-line reason (AC-F028-03)', () => {
		for (const id of [
			'run',
			'copyPrompt',
			'copyCommand',
			'pasteBack',
			'markDone',
			'focusMode',
			'pinned'
		] as const) {
			expect(NOT_HERE[id]).toMatch(/^[A-Z].+\.$/);
		}
	});
});
