// Spike hook logger (exec form: `command` = node, `args` = [this file, <log file>, <label>]).
// Appends one JSON line per hook event to <log file> and prints nothing. Never blocks Claude Code:
// every failure is swallowed and the exit code is always 0. Used only by the Stage 5 spikes.
import { appendFileSync, readFileSync } from 'node:fs';

/** @param {unknown} e */
function text(e) {
	return e instanceof Error ? e.message : String(e);
}

try {
	const [, , logFile, label] = process.argv;
	let raw = '';
	try {
		raw = readFileSync(0, 'utf8');
	} catch (e) {
		raw = `<<stdin unreadable: ${text(e)}>>`;
	}
	/** @type {unknown} */
	let payload = raw;
	try {
		payload = JSON.parse(raw);
	} catch {
		// keep the raw text
	}
	if (logFile) {
		const line = JSON.stringify({
			t: Date.now(),
			label: label ?? null,
			hookPid: process.pid,
			parentPid: process.ppid,
			env: {
				CLAUDE_EFFORT: process.env['CLAUDE_EFFORT'] ?? null,
				CLAUDE_PROJECT_DIR: process.env['CLAUDE_PROJECT_DIR'] ?? null
			},
			payload
		});
		appendFileSync(logFile, `${line}\n`);
	}
} catch {
	// never fail a hook
}
process.exit(0);
