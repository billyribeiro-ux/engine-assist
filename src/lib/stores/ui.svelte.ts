// App-wide UI state: the title bar, toasts and the live region, the terminal drawer, the
// inspector, the palette, and the theme. Plain runes state shared by module (one window).
import type { Theme } from '#lib/ipc/types.ts';

export interface Chrome {
	title: string;
	sub: string;
	drawer: boolean;
	inspector: boolean;
}

export interface Toast {
	id: number;
	text: string;
	tone: 'pass' | 'fail';
	icon: 'check' | 'stamp' | 'copy' | 'cross';
}

const DRAWER_KEY = 'fm.drawerHeight';
const DRAWER_OPEN_KEY = 'fm.drawerOpen';

function readNumber(key: string, fallback: number): number {
	try {
		const v = Number(localStorage.getItem(key));
		return Number.isFinite(v) && v > 0 ? v : fallback;
	} catch {
		return fallback;
	}
}
function write(key: string, value: string): void {
	try {
		localStorage.setItem(key, value);
	} catch {
		// Storage can be unavailable; the default height applies next time.
	}
}

class Ui {
	chrome = $state<Chrome>({ title: 'Foreman', sub: '', drawer: false, inspector: false });
	toasts = $state<Toast[]>([]);
	/** Polite announcements for the shared live region (components.md "Toast"). */
	announcement = $state('');
	/** One-line reason when a shortcut doesn't apply (AC-F028-03). */
	hint = $state('');
	drawerOpen = $state(true);
	drawerHeight = $state(216);
	inspectorOpen = $state(true);
	paletteOpen = $state(false);
	/** Lifts toasts above a sticky footer (outside-change review). */
	toastLift = $state(false);
	theme = $state<Theme>('system');
	/** The stage whose stamp should play (F-014 read-back passed). */
	stampStage = $state<{ projectId: string; stage: number } | null>(null);
	private nextId = 1;

	constructor() {
		this.drawerHeight = readNumber(DRAWER_KEY, 216);
		try {
			this.drawerOpen = localStorage.getItem(DRAWER_OPEN_KEY) !== '0';
		} catch {
			this.drawerOpen = true;
		}
	}

	setChrome(c: Partial<Chrome> & { title: string }): void {
		this.chrome = { sub: '', drawer: false, inspector: false, ...c };
	}

	toast(
		text: string,
		tone: Toast['tone'] = 'pass',
		icon: Toast['icon'] = tone === 'pass' ? 'check' : 'cross'
	): void {
		const id = this.nextId++;
		this.toasts = [...this.toasts, { id, text, tone, icon }];
		this.announce(text);
		setTimeout(() => {
			this.dismiss(id);
		}, 6000);
	}
	dismiss(id: number): void {
		this.toasts = this.toasts.filter((t) => t.id !== id);
	}
	announce(text: string): void {
		this.announcement = '';
		queueMicrotask(() => {
			this.announcement = text;
		});
	}
	say(reason: string): void {
		this.hint = reason;
		this.announce(reason);
		setTimeout(() => {
			if (this.hint === reason) this.hint = '';
		}, 5000);
	}

	toggleDrawer(): void {
		this.drawerOpen = !this.drawerOpen;
		write(DRAWER_OPEN_KEY, this.drawerOpen ? '1' : '0');
	}
	setDrawerHeight(px: number): void {
		const max = Math.max(160, Math.round(window.innerHeight * 0.6));
		this.drawerHeight = Math.min(max, Math.max(120, Math.round(px)));
		write(DRAWER_KEY, String(this.drawerHeight));
	}
	/** Below 1360 px the inspector is hidden; ⌘⌥I brings it back as an overlay (A.10). */
	inspectorOverlay = $state(false);
	toggleInspector(): void {
		if (window.innerWidth < 1360) this.inspectorOverlay = !this.inspectorOverlay;
		else this.inspectorOpen = !this.inspectorOpen;
	}

	applyTheme(t: Theme): void {
		this.theme = t;
		if (t === 'system') document.documentElement.removeAttribute('data-theme');
		else document.documentElement.setAttribute('data-theme', t);
	}
}

export const ui = new Ui();
