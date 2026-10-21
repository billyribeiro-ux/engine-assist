// Keyboard shortcuts (Master Spec A.7, components.md): one table, one dispatcher. Screens register
// handlers for the actions they support; the newest registration wins (a screen over the app). A
// handler returns `true` when it acted, or a one-line reason when the action doesn't apply to the
// current selection; the reason is shown and announced, and nothing else happens (AC-F028-03).

export type ActionId =
	| 'palette'
	| 'run'
	| 'copyPrompt'
	| 'copyCommand'
	| 'nextStep'
	| 'prevStep'
	| 'openStep'
	| 'nextStage'
	| 'prevStage'
	| 'jumpStage'
	| 'toggleDrawer'
	| 'toggleInspector'
	| 'openGate'
	| 'newProject'
	| 'settings'
	| 'doNext'
	| 'pasteBack'
	| 'markDone'
	| 'focusMode'
	| 'returnFocus'
	| 'pinned';

export type Handler = (arg?: number) => true | string | Promise<true | string>;

export interface Shortcut {
	id: ActionId;
	label: string;
	keys: string;
}

/** Master Spec A.7, plus ⌘⇧↵, ⌘⇧V, ⌘D, ⌘⇧T, ⌘⇧E and ⌘1–⌘9. */
export const SHORTCUTS: readonly Shortcut[] = [
	{ id: 'palette', label: 'Command palette', keys: '⌘K' },
	{ id: 'run', label: 'Run the selected step', keys: '⌘↵' },
	{ id: 'copyPrompt', label: 'Copy prompt', keys: '⌘⇧C' },
	{ id: 'copyCommand', label: 'Copy command', keys: '⌘⌥C' },
	{ id: 'nextStep', label: 'Next step', keys: 'J' },
	{ id: 'prevStep', label: 'Previous step', keys: 'K' },
	{ id: 'nextStage', label: 'Next stage', keys: '⌘]' },
	{ id: 'prevStage', label: 'Previous stage', keys: '⌘[' },
	{ id: 'jumpStage', label: 'Jump to a stage', keys: '⌘G' },
	{ id: 'toggleDrawer', label: 'Toggle terminal drawer', keys: '⌃`' },
	{ id: 'toggleInspector', label: 'Toggle inspector', keys: '⌘⌥I' },
	{ id: 'openGate', label: 'Open gate for this stage', keys: '⌘⇧G' },
	{ id: 'newProject', label: 'New project', keys: '⌘N' },
	{ id: 'settings', label: 'Settings', keys: '⌘,' },
	{ id: 'doNext', label: 'Do next', keys: '⌘⇧↵' },
	{ id: 'pasteBack', label: 'Paste back into the selected chat step', keys: '⌘⇧V' },
	{ id: 'markDone', label: 'Mark a chat step done', keys: '⌘D' },
	{ id: 'focusMode', label: 'Terminal focus mode', keys: '⌘⇧T' },
	{ id: 'returnFocus', label: 'Return focus from the terminal', keys: '⌘⇧E' },
	{ id: 'pinned', label: 'Open pinned project', keys: '⌘1–⌘9' }
];

export interface KeyLike {
	key: string;
	code?: string;
	metaKey: boolean;
	ctrlKey: boolean;
	altKey: boolean;
	shiftKey: boolean;
}

/** Maps a key event to an action (and a digit for ⌘1–⌘9). Plain J/K/↓/↑ only when `plainKeys`. */
export function match(e: KeyLike, plainKeys: boolean): { id: ActionId; arg?: number } | null {
	const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
	const code = e.code ?? '';
	if (e.ctrlKey && !e.metaKey && (k === '`' || code === 'Backquote')) return { id: 'toggleDrawer' };
	if (e.metaKey && !e.ctrlKey) {
		if (e.altKey && !e.shiftKey && (k === 'c' || code === 'KeyC' || k === 'ç'))
			return { id: 'copyCommand' };
		if (e.altKey && !e.shiftKey && (k === 'i' || code === 'KeyI' || k === 'ˆ'))
			return { id: 'toggleInspector' };
		if (e.altKey) return null;
		if (e.shiftKey) {
			if (k === 'Enter') return { id: 'doNext' };
			if (k === 'c') return { id: 'copyPrompt' };
			if (k === 'v') return { id: 'pasteBack' };
			if (k === 'g') return { id: 'openGate' };
			if (k === 't') return { id: 'focusMode' };
			if (k === 'e') return { id: 'returnFocus' };
			return null;
		}
		if (k === 'k') return { id: 'palette' };
		if (k === 'Enter') return { id: 'run' };
		if (k === ']') return { id: 'nextStage' };
		if (k === '[') return { id: 'prevStage' };
		if (k === 'g') return { id: 'jumpStage' };
		if (k === 'n') return { id: 'newProject' };
		if (k === ',') return { id: 'settings' };
		if (k === 'd') return { id: 'markDone' };
		if (/^[1-9]$/.test(k)) return { id: 'pinned', arg: Number(k) };
		return null;
	}
	if (!plainKeys || e.metaKey || e.ctrlKey || e.altKey) return null;
	if (k === 'j' || k === 'ArrowDown') return { id: 'nextStep' };
	if (k === 'k' || k === 'ArrowUp') return { id: 'prevStep' };
	return null;
}

type Layer = Partial<Record<ActionId, Handler>>;
const layers: Layer[] = [];

/** Registers handlers; returns the unregister function (call it from the component's cleanup). */
export function registerShortcuts(layer: Layer): () => void {
	layers.push(layer);
	return () => {
		const i = layers.indexOf(layer);
		if (i >= 0) layers.splice(i, 1);
	};
}

export function handlerFor(id: ActionId): Handler | undefined {
	for (let i = layers.length - 1; i >= 0; i--) {
		const h = layers[i]?.[id];
		if (h) return h;
	}
	return undefined;
}

/** Elements where plain letters are text, not shortcuts. */
export function isTyping(target: EventTarget | null): boolean {
	if (!(target instanceof Element)) return false;
	if (target.closest('.xterm, [data-terminal]')) return true;
	if (target.closest('[role="dialog"]')) return true;
	const tag = target.tagName;
	return (
		tag === 'INPUT' ||
		tag === 'TEXTAREA' ||
		tag === 'SELECT' ||
		(target as HTMLElement).isContentEditable
	);
}

export const NOT_HERE: Record<ActionId, string> = {
	palette: '',
	run: 'Select a step to run it.',
	copyPrompt: 'Select a step to copy its prompt.',
	copyCommand: 'Select a CLI step to copy its command.',
	nextStep: '',
	prevStep: '',
	openStep: '',
	nextStage: 'Open a project to move between stages.',
	prevStage: 'Open a project to move between stages.',
	jumpStage: 'Open a project to jump to a stage.',
	toggleDrawer: '',
	toggleInspector: 'This screen has no inspector.',
	openGate: 'Open a project to see its gate.',
	newProject: '',
	settings: '',
	doNext: '',
	pasteBack: 'Paste back works on a chat step that writes a packet or an output.',
	markDone: 'Mark done works on a chat step.',
	focusMode: 'Focus mode needs a terminal session.',
	returnFocus: '',
	pinned: 'No project is pinned to that number.'
};
