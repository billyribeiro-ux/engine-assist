// Key routing for a focused terminal (AC-F009-08, ADR-006, components.md "Keyboard"; M3-11).
// xterm's custom key handler asks `routeKey` for every key event:
// - `pty`: xterm handles it and sends it to Claude Code (every key that isn't routed below);
// - `app`: xterm ignores it and the window's shortcut dispatcher acts (⌘-chords, ⌃`; ⌘⇧E is the
//   app's "return focus to the workspace", so the terminal is never a keyboard trap);
// - `copy`, `paste`, `selectAll`, `find`: ⌘C copies the selection, ⌘V pastes through Rust
//   (`pty_paste`, SR-28/SR-56), ⌘A selects the buffer, ⌘F finds in it.
export type KeyRoute = 'pty' | 'app' | 'copy' | 'paste' | 'selectAll' | 'find';

export interface KeyEventLike {
	type: string;
	key: string;
	code?: string;
	metaKey: boolean;
	ctrlKey: boolean;
	altKey: boolean;
	shiftKey: boolean;
}

function letter(e: KeyEventLike): string {
	if (e.code?.startsWith('Key')) return e.code.slice(3).toLowerCase();
	return e.key.length === 1 ? e.key.toLowerCase() : e.key;
}

/** Where a key event goes while the terminal has focus. */
export function routeKey(e: KeyEventLike): KeyRoute {
	// ⌃` toggles the drawer from anywhere.
	if (e.ctrlKey && !e.metaKey && (e.key === '`' || e.code === 'Backquote')) return 'app';
	if (!e.metaKey) return 'pty';
	// Only keydown acts; xterm must also leave the matching keypress/keyup alone.
	const plain = !e.shiftKey && !e.altKey && !e.ctrlKey;
	const k = letter(e);
	if (plain && k === 'c') return 'copy';
	if (plain && k === 'v') return 'paste';
	if (plain && k === 'a') return 'selectAll';
	if (plain && k === 'f') return 'find';
	return 'app';
}

/** xterm's answer for a custom key handler: true lets xterm process the event. */
export function xtermHandles(route: KeyRoute): boolean {
	return route === 'pty';
}
