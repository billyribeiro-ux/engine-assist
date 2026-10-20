// Buffer text (AC-F009-06; M3-11): the archive a re-arm leaves behind, and terminal find (⌘F).
// The archive is plain text read from xterm's buffer (escape sequences already interpreted) and is
// only ever shown as text, never as HTML (SR-26).
import { type IBuffer, Terminal } from '@xterm/xterm';

import { WINDOW_OPTIONS, scrollbackCap } from './options.ts';

export interface Archive {
	/** What ran in the buffer ("2.2 Convene the council"). */
	label: string;
	/** The buffer as text, capped at the scrollback cap in lines. */
	text: string;
	lines: number;
}

/** A buffer's lines as text, without trailing blank lines. */
export function bufferText(buf: IBuffer): string[] {
	const out: string[] = [];
	for (let i = 0; i < buf.length; i++) out.push(buf.getLine(i)?.translateToString(true) ?? '');
	while (out.length > 0 && out.at(-1) === '') out.pop();
	return out;
}

/**
 * Replays a run's raw output into a detached xterm (never opened, nothing subscribed to its input,
 * the same window options as a tab) and returns the result as text, at most `scrollback` lines.
 */
export async function archiveOf(
	label: string,
	chunks: readonly Uint8Array[],
	cols: number,
	scrollback: number
): Promise<Archive> {
	const cap = scrollbackCap(scrollback);
	const t = new Terminal({
		cols: Math.max(2, Math.min(1000, cols)),
		rows: 24,
		scrollback: cap,
		allowProposedApi: false,
		logLevel: 'off',
		windowOptions: { ...WINDOW_OPTIONS }
	});
	try {
		for (const c of chunks) await new Promise<void>((done) => t.write(c, done));
		const lines = bufferText(t.buffer.active).slice(-cap);
		return { label, text: lines.join('\n'), lines: lines.length };
	} finally {
		t.dispose();
	}
}

export interface Match {
	row: number;
	col: number;
	length: number;
}

/**
 * The next case-insensitive match of `query` after `from` (row, col), wrapping to the top; null when
 * there is none. Columns are string indexes in the line's text, which equal cells for the ASCII and
 * single-width text a query is matched against.
 */
export function findNext(
	lines: readonly string[],
	query: string,
	from: { row: number; col: number } | null
): Match | null {
	const q = query.toLowerCase();
	if (!q) return null;
	const n = lines.length;
	const start = from ? from.row : 0;
	for (let k = 0; k <= n; k++) {
		const row = (start + k) % Math.max(1, n);
		const line = (lines[row] ?? '').toLowerCase();
		const after = k === 0 && from ? from.col + 1 : 0;
		const col = line.indexOf(q, after);
		if (col >= 0) return { row, col, length: query.length };
	}
	return null;
}
