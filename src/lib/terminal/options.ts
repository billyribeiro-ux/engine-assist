// xterm options (F-009, ADR-006, SR-29; M3-11). Every security-relevant option is fixed here, so
// one snapshot (`terminal::options_snapshot`) pins them all:
// - `windowOptions` all false: no window manipulation and no window reports, the title report
//   (`CSI 21 t`) included, so terminal output can't make xterm type a reply it chose (T-096);
// - no `@xterm/addon-clipboard` (no OSC 52 reads or writes, T-094), no `onTitleChange` (T-095);
// - `ignoreBracketedPasteMode: false`: Rust strips ESC from a paste, then xterm brackets it (SR-28);
// - `allowProposedApi: false`, `logLevel: 'off'` (xterm never writes terminal text to the console).
// The rest comes from Settings: font size, scrollback (the cap, AC-F009-06), Option as Meta and
// Screen reader mode (AC-F008-20).
import type { ILinkHandler, ITerminalOptions, ITheme, IWindowOptions } from '@xterm/xterm';

/** The default scrollback cap in lines (AC-F009-06; S-0's pass branch). */
export const SCROLLBACK_DEFAULT = 10_000;
/** Settings' bounds for the scrollback cap (ipc.md `settings_update`, files.md §4.1). */
export const SCROLLBACK_MIN = 1_000;
export const SCROLLBACK_MAX = 100_000;

/** Every `IWindowOptions` key of @xterm/xterm 6.0.0, each switched off. */
export const WINDOW_OPTIONS: Readonly<Required<IWindowOptions>> = Object.freeze({
	restoreWin: false,
	minimizeWin: false,
	setWinPosition: false,
	setWinSizePixels: false,
	raiseWin: false,
	lowerWin: false,
	refreshWin: false,
	setWinSizeChars: false,
	maximizeWin: false,
	fullscreenWin: false,
	getWinState: false,
	getWinPosition: false,
	getWinSizePixels: false,
	getScreenSizePixels: false,
	getCellSizePixels: false,
	getWinSizeChars: false,
	getScreenSizeChars: false,
	getIconTitle: false,
	getWinTitle: false,
	pushTitle: false,
	popTitle: false,
	setWinLines: false
});

export interface TerminalPrefs {
	fontFamily: string;
	fontSize: number;
	scrollback: number;
	macOptionIsMeta: boolean;
	screenReaderMode: boolean;
	/** False for an exited run: the buffer is read-only. */
	live: boolean;
	theme?: ITheme;
	linkHandler?: ILinkHandler | null;
}

/** The scrollback cap for a Settings value: a whole number of lines within Settings' bounds. */
export function scrollbackCap(lines: number): number {
	if (!Number.isFinite(lines)) return SCROLLBACK_DEFAULT;
	return Math.min(SCROLLBACK_MAX, Math.max(SCROLLBACK_MIN, Math.floor(lines)));
}

/** The options a tab's xterm is built with. */
export function terminalOptions(p: TerminalPrefs): ITerminalOptions {
	return {
		fontFamily: p.fontFamily,
		fontSize: p.fontSize,
		lineHeight: 1.25,
		scrollback: scrollbackCap(p.scrollback),
		screenReaderMode: p.screenReaderMode,
		macOptionIsMeta: p.macOptionIsMeta,
		disableStdin: !p.live,
		cursorBlink: false,
		// WCAG 2.2 AA for terminal text too, dim (SGR 2) included: xterm adjusts the foreground.
		minimumContrastRatio: 4.5,
		allowProposedApi: false,
		ignoreBracketedPasteMode: false,
		logLevel: 'off',
		windowOptions: { ...WINDOW_OPTIONS },
		...(p.theme ? { theme: p.theme } : {}),
		linkHandler: p.linkHandler ?? null
	};
}
