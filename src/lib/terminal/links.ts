// Plain URLs in terminal output (AC-F009-17, SR-29; M3-12): the job @xterm/addon-web-links does,
// done with xterm's own `registerLinkProvider` so no addon is added. A found link only hands its
// text to `activate`, which sends it to Rust (`terminal_link_open`): Rust decides whether it may
// open (https, or http to localhost, no user info) and asks in a native dialog first. Nothing in
// the webview opens a window or navigates.
import type { ILink, ILinkProvider, Terminal } from '@xterm/xterm';

/** `http(s)://` up to whitespace or a quote/bracket that can't be part of the link. */
const URL_RE = /\bhttps?:\/\/[^\s"'`<>()[\]{}]+/gi;
/** Punctuation that ends a sentence rather than the URL. */
const TRAILING = /[.,;:!?]+$/;

export interface FoundUrl {
	/** 0-based string index in the line's text. */
	start: number;
	text: string;
}

/** The URLs in one line of terminal text. */
export function findUrls(line: string): FoundUrl[] {
	const out: FoundUrl[] = [];
	for (const m of line.matchAll(URL_RE)) {
		const text = m[0].replace(TRAILING, '');
		if (text.length > 'https://'.length) out.push({ start: m.index, text });
	}
	return out;
}

/** A link provider that finds plain URLs in a buffer line and hands them to `activate`. */
export function urlLinks(term: Terminal, activate: (url: string) => void): ILinkProvider {
	return {
		provideLinks(y, callback) {
			const line = term.buffer.active.getLine(y - 1)?.translateToString(true) ?? '';
			const links: ILink[] = findUrls(line).map((u) => ({
				range: { start: { x: u.start + 1, y }, end: { x: u.start + u.text.length, y } },
				text: u.text,
				activate: (_e, text) => {
					activate(text);
				}
			}));
			callback(links.length > 0 ? links : undefined);
		}
	};
}
