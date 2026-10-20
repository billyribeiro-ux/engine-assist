// Spike S-23 (M0-18): `claude --desktop --resume <uuid>`.
//
//   pnpm spikes s23 prepare    1 tiny session (tokens, y/N), snapshots the transcripts, prints the Desktop steps
//   pnpm spikes s23 check      after the Desktop steps: did Desktop continue the SAME transcript or copy it?
//
// Question: when Desktop continues a CLI session started with `--session-id <uuid>`, does it append to
// the same transcript file (shared), or copy the conversation into its own session (copied)?
//
// Decision rule (MASTER-SPEC §9): shared -> the chain is handed off and the next Foreman step forks
// (AC-F010-15). Copied -> no fork is needed and the CLI chain continues unaffected (AC-F024-03 records it).
// Needs Claude Desktop installed and signed in on the same account.
import { createHash, randomBytes } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';

import {
	Report,
	askYesNo,
	confirmTokens,
	eventsOf,
	installHookLogger,
	listTranscripts,
	loggerSettings,
	makeSandbox,
	newUuid,
	parseArgs,
	preflight,
	readHookLog,
	reportPreflight,
	runClaudeInteractive,
	spikeEnv,
	spikeRoot,
	writeJson
} from './lib.mjs';

const { flags, rest } = parseArgs(process.argv.slice(2));
const mode = rest[0] ?? 'help';
const report = new Report('S-23 claude --desktop --resume');

/** @param {string} file */
function sha256(file) {
	return createHash('sha256').update(readFileSync(file)).digest('hex');
}

/**
 * Bounded walk of Claude Desktop's data folder for session-like files (.jsonl / .json).
 * @returns {{ file: string, size: number, mtimeMs: number }[]}
 */
function desktopFiles() {
	const root = join(homedir(), 'Library', 'Application Support', 'Claude');
	/** @type {{ file: string, size: number, mtimeMs: number }[]} */
	const out = [];
	const skip = new Set([
		'node_modules',
		'Cache',
		'Code Cache',
		'GPUCache',
		'IndexedDB',
		'Local Storage',
		'blob_storage',
		'Service Worker',
		'Session Storage',
		'Crashpad',
		'logs'
	]);
	let budget = 60_000;
	/** @param {string} dir @param {number} depth */
	const walk = (dir, depth) => {
		if (depth > 8 || budget <= 0) return;
		let names;
		try {
			names = readdirSync(dir, { withFileTypes: true });
		} catch {
			return;
		}
		for (const d of names) {
			budget--;
			const p = join(dir, d.name);
			if (d.isDirectory()) {
				if (!skip.has(d.name)) walk(p, depth + 1);
			} else if (d.isFile() && (d.name.endsWith('.jsonl') || d.name.endsWith('.json'))) {
				try {
					const s = statSync(p);
					out.push({ file: p, size: s.size, mtimeMs: s.mtimeMs });
				} catch {
					// vanished
				}
			}
		}
	};
	if (existsSync(root)) walk(root, 0);
	return out;
}

/**
 * @typedef {object} State
 * @property {string} id
 * @property {string} cwd
 * @property {string} transcript
 * @property {string} transcriptDir
 * @property {string} marker
 * @property {string} codeWord
 * @property {number} preparedAt
 * @property {number} sizeBefore
 * @property {string} shaBefore
 * @property {string[]} transcriptsBefore
 */

if (mode === 'prepare') {
	const sb = makeSandbox('s23');
	const cwd = join(sb, 'work');
	mkdirSync(cwd, { recursive: true });
	if (!reportPreflight(preflight(sb), report, flags.has('force'))) {
		report.finish('INCONCLUSIVE', 'none (preflight failed)', []);
		process.exit();
	}
	if (
		process.platform === 'darwin' &&
		!existsSync('/Applications/Claude.app') &&
		!existsSync(join(homedir(), 'Applications', 'Claude.app'))
	) {
		console.log(
			'WARNING: Claude.app was not found in /Applications; --desktop needs Claude Desktop installed and signed in.'
		);
	}
	const ok = await confirmTokens(
		`One tiny session in ${cwd} (plan mode). Later you send ONE message from Claude Desktop (a few hundred tokens).\n  When Claude answers "ok", type /exit.`
	);
	if (!ok) {
		report.finish('INCONCLUSIVE', 'none (not run)', []);
		process.exit();
	}
	const id = newUuid();
	const codeWord = `PLUM-${randomBytes(3).toString('hex')}`;
	const marker = `S23-MARKER-${randomBytes(4).toString('hex')}`;
	const logFile = join(sb, 'events.jsonl');
	const logger = installHookLogger(join(sb, 'bin'));
	const settings = join(sb, 'settings.json');
	writeJson(
		settings,
		loggerSettings({ logger, logFile, events: ['SessionStart', 'Stop', 'SessionEnd'] })
	);
	await runClaudeInteractive(
		[
			'--session-id',
			id,
			'--model',
			'opus',
			'--effort',
			'low',
			'--permission-mode',
			'plan',
			'--name',
			'fm-s23',
			'--settings',
			settings,
			`Remember this code word: ${codeWord}. Reply with the single word: ok`
		],
		{ cwd, env: spikeEnv(), watchdogMs: 10 * 60_000 }
	);
	const start = eventsOf(readHookLog(logFile), 'SessionStart')[0];
	const transcript =
		typeof start?.payload['transcript_path'] === 'string' ? start.payload['transcript_path'] : '';
	if (!transcript || !existsSync(transcript)) {
		console.log(
			`No transcript found (transcript_path=${transcript || 'none'}). Did the session answer before /exit? Re-run prepare.`
		);
		process.exit(2);
	}
	const transcriptDir = dirname(transcript);
	/** @type {State} */
	const state = {
		id,
		cwd,
		transcript,
		transcriptDir,
		marker,
		codeWord,
		preparedAt: Date.now(),
		sizeBefore: statSync(transcript).size,
		shaBefore: sha256(transcript),
		transcriptsBefore: listTranscripts(transcriptDir).map((t) => t.file)
	};
	writeJson(join(spikeRoot(), 's23-state.json'), state);
	console.log(
		`\nSession ${id} is saved (${state.sizeBefore} bytes, ${state.transcriptsBefore.length} transcript(s) in its folder).`
	);
	console.log('\nNow, by hand:');
	console.log(`  1. In a terminal:  cd '${cwd}' && claude --desktop --resume ${id}`);
	console.log(
		'     (leave this terminal alone afterwards; do not start another CLI session in that folder)'
	);
	console.log(
		'  2. Claude Desktop should open on this session. Check it shows the earlier exchange (the code word and "ok").'
	);
	console.log(
		`  3. In Desktop type:  ${marker}: what was the code word? Reply with it, then wait for the answer.`
	);
	console.log('  4. Back here:  pnpm spikes s23 check');
	if (
		process.stdin.isTTY &&
		(await askYesNo('Run step 1 for you now (claude --desktop --resume, 60 s watchdog)?'))
	) {
		await runClaudeInteractive(['--desktop', '--resume', id], {
			cwd,
			env: spikeEnv(),
			watchdogMs: 60_000
		});
		console.log('\nclaude returned. Continue with steps 2 to 4.');
	}
	process.exit(0);
}

if (mode === 'check') {
	const statePath = join(spikeRoot(), 's23-state.json');
	if (!existsSync(statePath)) {
		console.log('No saved state; run `pnpm spikes s23 prepare` first.');
		process.exit(2);
	}
	const st = /** @type {State} */ (JSON.parse(readFileSync(statePath, 'utf8')));
	const now = listTranscripts(st.transcriptDir);
	const sizeNow = existsSync(st.transcript) ? statSync(st.transcript).size : 0;
	const changed = existsSync(st.transcript) && sha256(st.transcript) !== st.shaBefore;
	const contains = (/** @type {string} */ file) => {
		try {
			return (
				statSync(file).size < 64 * 1024 * 1024 && readFileSync(file, 'utf8').includes(st.marker)
			);
		} catch {
			return false;
		}
	};
	const inOriginal = existsSync(st.transcript) && contains(st.transcript);
	const newTranscripts = now.filter((t) => !st.transcriptsBefore.includes(t.file));
	const markerInNew = newTranscripts.filter((t) => contains(t.file));
	const desktopHits = desktopFiles().filter((f) => f.mtimeMs >= st.preparedAt && contains(f.file));
	report.facts['session'] = st.id;
	report.facts['originalTranscript'] = {
		path: st.transcript,
		bytesBefore: st.sizeBefore,
		bytesNow: sizeNow,
		changed,
		containsMarker: inOriginal
	};
	report.facts['newTranscripts'] = newTranscripts.map((t) => t.file);
	report.facts['desktopFilesWithMarker'] = desktopHits.map((f) => f.file);
	report.add(
		'original CLI transcript grew',
		changed ? 'PASS' : 'INFO',
		`${String(st.sizeBefore)} -> ${String(sizeNow)} bytes, ${changed ? 'content changed' : 'unchanged'}`
	);
	report.add(
		'Desktop message found in the ORIGINAL transcript',
		inOriginal ? 'PASS' : 'INFO',
		inOriginal ? `${st.marker} is in ${st.transcript}` : `${st.marker} not found there`
	);
	report.add(
		'new .jsonl files in the same folder',
		'INFO',
		newTranscripts.length === 0
			? 'none'
			: newTranscripts.map((t) => `${t.file} (marker: ${String(contains(t.file))})`).join('; ')
	);
	report.add(
		'Desktop data files containing the message',
		'INFO',
		desktopHits.length === 0
			? 'none found (Desktop may store sessions elsewhere or use a database)'
			: desktopHits.map((f) => f.file).join('; ')
	);
	/** @type {'PASS' | 'FAIL' | 'INCONCLUSIVE'} */
	let verdict = 'INCONCLUSIVE';
	let branch =
		'undecided: the marker was found nowhere. Did Desktop open the session and answer? Re-run steps 2-4 (or inspect by hand, see docs/spikes/S-23.md)';
	let changes = ['(none until decided)'];
	if (inOriginal) {
		verdict = 'PASS';
		branch =
			'SHARED: Desktop appended to the same transcript, so the chain is handed off and the next Foreman step forks (AC-F010-15); AC-F024-03 records "shared"';
		changes = [
			'docs/adr/ADR-008 (Desktop handoff: shared confirmed)',
			'docs/scope.md AC-F024-03 (branch recorded), AC-F010-15 stays',
			'chain handoff test in M4-29 encodes "shared"'
		];
	} else if (markerInNew.length > 0 || desktopHits.length > 0) {
		verdict = 'FAIL';
		branch =
			'COPIED: Desktop made its own session; no fork is needed and the CLI chain continues unaffected; AC-F024-03 records "copied"';
		changes = [
			'docs/adr/ADR-008 (Desktop handoff: copied; simplify F-024)',
			'docs/scope.md AC-F024-03 (branch recorded), AC-F010-15 loses its Desktop trigger',
			'chain handoff test in M4-29 encodes "copied"'
		];
	}
	report.note(
		'MASTER-SPEC §9 lists "Shared" in the Pass column and "Copied" in the Fail column. Both are legitimate outcomes: FAIL here is not a defect, it only selects the simpler branch.'
	);
	report.finish(verdict, branch, changes);
	process.exit();
}

console.log('usage: pnpm spikes s23 <prepare|check>');
process.exit(2);
