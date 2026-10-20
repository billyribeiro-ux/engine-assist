// SR-26, AC-XC-32: text from a repo, the clipboard or a terminal is shown as text, and characters
// that can hide or reorder what Billy reads (bidi controls, zero-width and other default-ignorable
// characters, C0/C1 controls, DEL) are shown as visible escapes. Rust pre-computes `escape` spans
// (ipc.md §2.3); the webview applies the same split to every plain string it renders as a second
// line of defense, so a missed character can never render invisibly.
import type { TextSpan } from '#lib/ipc/types.ts';

const NAMES: Record<number, string> = {
	0x00: 'NUL',
	0x07: 'BEL',
	0x08: 'BS',
	0x09: 'TAB',
	0x0a: 'LF',
	0x0b: 'VT',
	0x0c: 'FF',
	0x0d: 'CR',
	0x1b: 'ESC',
	0x7f: 'DEL',
	0x200b: 'ZWSP',
	0x200c: 'ZWNJ',
	0x200d: 'ZWJ',
	0x200e: 'LRM',
	0x200f: 'RLM',
	0x202a: 'LRE',
	0x202b: 'RLE',
	0x202c: 'PDF',
	0x202d: 'LRO',
	0x202e: 'RLO',
	0x2060: 'WJ',
	0x2066: 'LRI',
	0x2067: 'RLI',
	0x2068: 'FSI',
	0x2069: 'PDI',
	0x061c: 'ALM',
	0xfeff: 'BOM',
	0x00ad: 'SHY',
	0x034f: 'CGJ',
	0x180e: 'MVS'
};

/** True for characters that must be shown as an escape (TAB and LF stay as layout). */
export function isHidden(cp: number, keepLayout = true): boolean {
	if (keepLayout && (cp === 0x09 || cp === 0x0a)) return false;
	if (cp < 0x20 || (cp >= 0x7f && cp <= 0x9f)) return true; // C0, DEL, C1
	if (cp === 0x00ad || cp === 0x034f || cp === 0x061c || cp === 0x180e) return true;
	if (cp >= 0x200b && cp <= 0x200f) return true; // zero-width, LRM, RLM
	if (cp >= 0x202a && cp <= 0x202e) return true; // bidi embeddings and overrides
	if (cp >= 0x2060 && cp <= 0x206f) return true; // word joiner, invisible operators, isolates
	if (cp === 0xfeff) return true;
	if (cp >= 0xfe00 && cp <= 0xfe0f) return true; // variation selectors
	if (cp >= 0xe0000 && cp <= 0xe0fff) return true; // tags and supplementary variation selectors
	return false;
}

export function escapeLabel(cp: number): string {
	const name = NAMES[cp];
	const hex = cp.toString(16).toUpperCase().padStart(4, '0');
	return name ? `<${name} U+${hex}>` : `<U+${hex}>`;
}

/** Splits text into plain and escape spans. */
export function visibleSpans(text: string, keepLayout = true): TextSpan[] {
	const out: TextSpan[] = [];
	let plain = '';
	for (const ch of text) {
		const cp = ch.codePointAt(0) ?? 0;
		if (isHidden(cp, keepLayout)) {
			if (plain) out.push({ kind: 'plain', text: plain });
			plain = '';
			out.push({ kind: 'escape', codepoint: cp, label: escapeLabel(cp) });
		} else plain += ch;
	}
	if (plain || out.length === 0) out.push({ kind: 'plain', text: plain });
	return out;
}

/** The text with every hidden character replaced by its visible label (for single-line contexts). */
export function visibleText(text: string, keepLayout = true): string {
	return visibleSpans(text, keepLayout)
		.map((s) => (s.kind === 'escape' ? s.label : s.kind === 'plain' ? s.text : ''))
		.join('');
}

export function hasHidden(text: string): boolean {
	for (const ch of text) if (isHidden(ch.codePointAt(0) ?? 0)) return true;
	return false;
}

/** Plain text of a span tree (for accessible names and copying). */
export function spansText(spans: readonly TextSpan[]): string {
	return spans
		.map((s) => {
			switch (s.kind) {
				case 'plain':
				case 'code':
				case 'link':
					return s.text;
				case 'escape':
					return s.label;
				case 'strong':
				case 'emph':
					return spansText(s.children);
			}
		})
		.join('');
}
