// Spike S-8 (M0-16): graceful end, signals and forks.
//
//   pnpm spikes s8 argv        zero tokens: flag-combination rules for --resume / --fork-session / --session-id
//   pnpm spikes s8 fork        2 tiny sessions: `--resume <uuid> --fork-session --session-id <new>`
//   pnpm spikes s8 exit        1 tiny session: text in the input box, Ctrl-C, then /exit (add --background)
//   pnpm spikes s8 sigterm     1 tiny session: SIGTERM sent by this script (add --background)
//   pnpm spikes s8 sighup      1 tiny session: SIGHUP sent by this script (add --background)
//   pnpm spikes s8 all         argv, fork, exit, exit --background, sigterm, sighup in turn (about 8 minutes)
//
// Decision rule (MASTER-SPEC §9): everything works as specified -> re-arm as specified and fork IDs are
// chosen by Foreman. Otherwise: graceful end = Billy ends the session, then Foreman relaunches; fork IDs =
// the only new transcript after launch (ambiguous -> a fresh chain with context).
// The keystroke parts (Ctrl-C, /exit, typing a draft) are done by you in the TUI; this script times and
// records what the hooks, the process table and the transcript folder show, then asks you what you saw.
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';

import {
	Report,
	ask,
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
	runClaude,
	runClaudeInteractive,
	runSync,
	spikeEnv,
	waitFor,
	writeJson
} from './lib.mjs';

const { flags, rest } = parseArgs(process.argv.slice(2));
const which = rest[0] ?? 'help';
const bgFlag = flags.has('background');
const SLEEP_MARK = 'sleep 31337';

if (!['argv', 'fork', 'exit', 'sigterm', 'sighup', 'all'].includes(which)) {
	console.log(
		'usage: pnpm spikes s8 <argv|fork|exit|sigterm|sighup|all> [--background] [--window <seconds>]'
	);
	process.exit(2);
}

const report = new Report(
	`S-8 graceful end, signals and forks (${which}${bgFlag ? ' --background' : ''})`
);
const sb = makeSandbox('s8');
const cwd = join(sb, 'work');
mkdirSync(cwd, { recursive: true });
const logger = installHookLogger(join(sb, 'bin'));

if (!reportPreflight(preflight(sb), report, flags.has('force'))) {
	report.finish('INCONCLUSIVE', 'none (preflight failed; fix the problem above and re-run)', []);
	process.exit();
}

/** @type {Record<string, 'PASS' | 'FAIL' | 'INCONCLUSIVE'>} */
const results = {};

/** @param {string} name */
function setupSession(name) {
	const logFile = join(sb, `events-${name}.jsonl`);
	const settings = join(sb, `settings-${name}.json`);
	writeJson(settings, loggerSettings({ logger, logFile }));
	return { logFile, settings, id: newUuid() };
}

/** @returns {{ pid: number, command: string }[]} processes whose command line contains the sleep marker */
function sleepers() {
	const r = runSync('/bin/ps', ['-axo', 'pid=,command='], { timeoutMs: 10_000 });
	/** @type {{ pid: number, command: string }[]} */
	const out = [];
	for (const line of r.stdout.split('\n')) {
		const m = /^\s*(\d+)\s+(.*)$/.exec(line);
		if (m?.[1] && m[2] !== undefined && m[2].includes(SLEEP_MARK) && !m[2].includes('ps -axo')) {
			out.push({ pid: Number(m[1]), command: m[2] });
		}
	}
	return out;
}

/** @param {string} file */
function sha256(file) {
	return createHash('sha256').update(readFileSync(file)).digest('hex');
}

/** @param {string} file true when the last non-empty line parses as JSON */
function transcriptTailOk(file) {
	try {
		const lines = readFileSync(file, 'utf8')
			.split('\n')
			.filter((l) => l.trim());
		JSON.parse(lines[lines.length - 1] ?? '');
		return true;
	} catch {
		return false;
	}
}

const promptPlain = 'Reply with the single word: ok';
const promptBg =
	`Use the Bash tool with run_in_background set to true to run exactly this command: ${SLEEP_MARK}  ` +
	'Do not wait for it. Then reply with the single word: started';

// ---------------------------------------------------------------------------------------------
async function scenarioArgv() {
	console.log('\n[argv] zero tokens: flag-combination rules');
	const x = newUuid();
	const y = newUuid();
	const a = runClaude(['--init-only', '--resume', x, '--session-id', y], {
		cwd,
		timeoutMs: 20_000
	});
	const aText = `${a.stdout}${a.stderr}`.trim();
	report.add(
		'--resume + --session-id WITHOUT --fork-session is refused by Claude Code',
		a.status !== 0 && /fork-session/.test(aText) ? 'PASS' : 'INCONCLUSIVE',
		`exit ${String(a.status)}: ${aText.slice(0, 160)}`
	);
	const b = runClaude(['--init-only', '--resume', x, '--fork-session', '--session-id', y], {
		cwd,
		timeoutMs: 20_000
	});
	const bText = `${b.stdout}${b.stderr}`.trim();
	report.add(
		'--resume + --fork-session + --session-id <new> is accepted (parse level)',
		b.status === 0 ? 'PASS' : 'FAIL',
		`exit ${String(b.status)}${bText ? `: ${bText.slice(0, 160)}` : ' (no output)'}. Note: this does not prove the forked ID is honored; the fork scenario does.`
	);
	results['argv'] = a.status !== 0 && b.status === 0 ? 'PASS' : 'INCONCLUSIVE';
}

// ---------------------------------------------------------------------------------------------
async function scenarioFork() {
	const ok = await confirmTokens(
		'Two tiny sessions in ' +
			cwd +
			' (plan mode, one-word replies).\n' +
			'  Session 1 creates a transcript; session 2 forks it with --session-id <new>. Type /exit after each answer.'
	);
	if (!ok) {
		results['fork'] = 'INCONCLUSIVE';
		return;
	}
	const one = setupSession('fork1');
	const two = setupSession('fork2');
	console.log(`\nFork step 1/2: new session ${one.id}. When it answers "ok", type /exit.`);
	await runClaudeInteractive(
		[
			'--session-id',
			one.id,
			'--model',
			'opus',
			'--effort',
			'low',
			'--permission-mode',
			'plan',
			'--name',
			'fm-s8-fork-1',
			'--settings',
			one.settings,
			promptPlain
		],
		{ cwd, env: spikeEnv(), watchdogMs: 10 * 60_000 }
	);
	const start1 = eventsOf(readHookLog(one.logFile), 'SessionStart')[0];
	const t1Path =
		typeof start1?.payload['transcript_path'] === 'string' ? start1.payload['transcript_path'] : '';
	const dir = t1Path ? dirname(t1Path) : '';
	if (!t1Path || !existsSync(t1Path)) {
		report.add(
			'fork: session 1 transcript exists',
			'INCONCLUSIVE',
			`transcript_path=${t1Path || 'none'}`
		);
		results['fork'] = 'INCONCLUSIVE';
		return;
	}
	const before = new Map(listTranscripts(dir).map((t) => [t.file, t.size]));
	const hash1 = sha256(t1Path);
	const size1 = statSync(t1Path).size;
	console.log(
		`\nFork step 2/2: --resume ${one.id} --fork-session --session-id ${two.id}. When it answers, type /exit.`
	);
	await runClaudeInteractive(
		[
			'--resume',
			one.id,
			'--fork-session',
			'--session-id',
			two.id,
			'--model',
			'opus',
			'--effort',
			'low',
			'--permission-mode',
			'plan',
			'--name',
			'fm-s8-fork-2',
			'--settings',
			two.settings,
			'Reply with the single word: forked'
		],
		{ cwd, env: spikeEnv(), watchdogMs: 10 * 60_000 }
	);
	const start2 = eventsOf(readHookLog(two.logFile), 'SessionStart')[0];
	const after = listTranscripts(dir);
	const created = after.filter((t) => !before.has(t.file));
	const t2 = join(dir, `${two.id}.jsonl`);
	report.facts['fork'] = {
		original: one.id,
		requestedNewId: two.id,
		hookSource: start2?.payload['source'],
		hookSessionId: start2?.payload['session_id'],
		hookTranscript: start2?.payload['transcript_path'],
		newTranscripts: created.map((c) => c.file)
	};
	report.add(
		'fork: SessionStart reports source "fork"',
		start2?.payload['source'] === 'fork' ? 'PASS' : 'INCONCLUSIVE',
		`source=${String(start2?.payload['source'])}`
	);
	const idHonored = start2?.payload['session_id'] === two.id && existsSync(t2);
	report.add(
		"fork: the forked transcript uses Foreman's ID (--session-id <new>)",
		idHonored ? 'PASS' : start2 ? 'FAIL' : 'INCONCLUSIVE',
		idHonored
			? `hook session_id = ${two.id} and ${t2} exists`
			: `hook session_id=${String(start2?.payload['session_id'])}; ${t2} ${existsSync(t2) ? 'exists' : 'missing'}; new transcripts: ${created.map((c) => c.file).join(', ') || 'none'}`
	);
	const unchanged =
		existsSync(t1Path) && sha256(t1Path) === hash1 && statSync(t1Path).size === size1;
	report.add(
		'fork: the original transcript was not modified',
		unchanged ? 'PASS' : 'FAIL',
		unchanged
			? 'same size and SHA-256 as before the fork'
			: 'the original changed: the fork appended to it'
	);
	report.add(
		'fork fallback evidence: exactly one new .jsonl appeared after launch',
		created.length === 1 ? 'PASS' : 'INFO',
		`${String(created.length)} new transcript file(s): ${created.map((c) => c.file.split('/').pop()).join(', ') || 'none'} (this is the fail-branch rule "the only new transcript after launch")`
	);
	results['fork'] = idHonored && unchanged ? 'PASS' : start2 ? 'FAIL' : 'INCONCLUSIVE';
}

// ---------------------------------------------------------------------------------------------
/**
 * @param {'exit' | 'sigterm' | 'sighup'} kind
 * @param {boolean} background run a background Bash task first
 */
async function scenarioEnd(kind, background) {
	const label = `${kind}${background ? '-bg' : ''}`;
	const windowSec = Number.parseInt(
		parseArgs(process.argv.slice(2), ['window']).opts.get('window') ?? '25',
		10
	);
	const steps =
		kind === 'exit'
			? [
					background
						? '1. Approve the Bash permission prompt when it appears (the command is `sleep 31337`, harmless).'
						: null,
					'2. Wait for Claude to answer.',
					'3. Type the text  draft text  in the input box. DO NOT press Enter.',
					'4. Press Ctrl-C once. Note what happens to the text and whether it asks "press again to exit".',
					'5. Type  /exit  and press Enter. Note whether any confirmation appears (for example about background tasks).'
				]
			: [
					background
						? '1. Approve the Bash permission prompt when it appears (the command is `sleep 31337`, harmless).'
						: null,
					'2. Wait for Claude to answer.',
					`3. Type the text  draft text  in the input box and DO NOT press Enter. Then do nothing: about ${String(windowSec)} s after Claude's answer this script sends ${kind === 'sigterm' ? 'SIGTERM' : 'SIGHUP'} to Claude.`
				];
	const ok = await confirmTokens(
		`One tiny session in ${cwd} (${background ? 'default mode, one background Bash task' : 'plan mode, one-word reply'}).\n  Steps:\n    ` +
			steps.filter(Boolean).join('\n    ')
	);
	if (!ok) {
		results[label] = 'INCONCLUSIVE';
		return;
	}
	const s = setupSession(label);
	const startedAt = Date.now();
	let signalSentAt = 0;
	const args = [
		'--session-id',
		s.id,
		'--model',
		'opus',
		'--effort',
		'low',
		'--permission-mode',
		background ? 'default' : 'plan',
		'--name',
		`fm-s8-${label}`,
		'--settings',
		s.settings,
		background ? promptBg : promptPlain
	];
	const run = await runClaudeInteractive(args, {
		cwd,
		env: spikeEnv(),
		watchdogMs: 10 * 60_000,
		onSpawn: (child) => {
			if (kind === 'exit') return;
			void (async () => {
				const stopped = await waitFor(
					() => eventsOf(readHookLog(s.logFile), 'Stop').length > 0,
					8 * 60_000,
					500
				);
				if (!stopped || child.exitCode !== null) return;
				await new Promise((r) => setTimeout(r, windowSec * 1000));
				if (child.exitCode !== null) return;
				signalSentAt = Date.now();
				child.kill(kind === 'sigterm' ? 'SIGTERM' : 'SIGHUP');
			})();
		}
	});
	const log = readHookLog(s.logFile);
	const stops = eventsOf(log, 'Stop');
	const ends = eventsOf(log, 'SessionEnd');
	const lastStop = stops[stops.length - 1];
	const end = ends[0];
	const start = eventsOf(log, 'SessionStart')[0];
	const tPath =
		typeof start?.payload['transcript_path'] === 'string' ? start.payload['transcript_path'] : '';
	const orphans = sleepers();
	const exitMs = run.endedAt;
	console.log(
		`\n${label}: exit code=${String(run.code)} signal=${String(run.signal)} total ${((run.endedAt - run.startedAt) / 1000).toFixed(1)} s`
	);
	const tailMs = end ? exitMs - end.t : null;
	report.facts[label] = {
		exitCode: run.code,
		exitSignal: run.signal,
		secondsSignalToExit: signalSentAt ? (exitMs - signalSentAt) / 1000 : null,
		secondsSessionEndHookToExit: tailMs === null ? null : tailMs / 1000,
		secondsLastStopToExit: lastStop ? (exitMs - lastStop.t) / 1000 : null,
		sessionEndFired: ends.length > 0,
		sessionEndPayload: end?.payload ?? null,
		eventsSeen: [...new Set(log.map((r) => r.label))],
		orphanProcesses: orphans,
		startedAt
	};
	report.add(
		`${label}: SessionEnd hook fired`,
		ends.length > 0 ? 'PASS' : 'INFO',
		ends.length > 0
			? `payload ${JSON.stringify(end?.payload)}`
			: 'no SessionEnd event (the reason field name stays UNVERIFIED in ADR-009)'
	);
	if (tPath) {
		report.add(
			`${label}: transcript intact`,
			existsSync(tPath) && transcriptTailOk(tPath) ? 'PASS' : 'FAIL',
			existsSync(tPath)
				? `${tPath} last line parses: ${String(transcriptTailOk(tPath))}`
				: 'transcript missing'
		);
	}
	if (background) {
		report.add(
			`${label}: background \`${SLEEP_MARK}\` after Claude exited`,
			'INFO',
			orphans.length > 0
				? `STILL RUNNING (pid ${orphans.map((o) => o.pid).join(', ')}): ending Claude did not stop background work`
				: 'gone: ending Claude also ended its background task'
		);
		for (const o of orphans) {
			try {
				process.kill(o.pid, 'SIGTERM');
			} catch {
				// already gone
			}
		}
	}
	if (kind === 'exit') {
		const within = await askYesNo('Did Claude exit within 10 s of you pressing Enter on /exit?');
		const prompted = await askYesNo(
			'Did Claude show any extra confirmation or prompt before exiting?'
		);
		const lost =
			(await ask('What did Ctrl-C do to the text in the input box? (cleared / kept / other): ')) ||
			'not answered';
		report.facts[`${label}_observed`] = {
			exitedWithin10s: within,
			extraPrompt: prompted,
			ctrlCOnDraft: lost
		};
		report.add(
			`${label}: Ctrl-C then /exit ends the session within 10 s`,
			within && !prompted && run.code === 0 ? 'PASS' : 'FAIL',
			`within 10 s: ${String(within)}; extra prompt: ${String(prompted)}; exit code ${String(run.code)}; Ctrl-C on draft: ${lost}`
		);
		results[label] = within && !prompted && run.code === 0 ? 'PASS' : 'FAIL';
	} else {
		const secs = signalSentAt ? (exitMs - signalSentAt) / 1000 : null;
		const clean = run.code === 0 || (run.signal === null && run.code !== null);
		const note =
			(await ask(
				'Anything odd on screen (message, broken terminal, lost draft)? Enter to skip: '
			)) || 'nothing noted';
		report.facts[`${label}_observed`] = note;
		report.add(
			`${label}: ${kind === 'sigterm' ? 'SIGTERM' : 'SIGHUP'} behavior`,
			'INFO',
			signalSentAt
				? `exit code ${String(run.code)} signal ${String(run.signal)} ${secs === null ? '' : `${secs.toFixed(2)} s after the signal`}; SessionEnd ${ends.length > 0 ? 'fired' : 'did not fire'}; clean exit: ${String(clean)}; operator note: ${note}`
				: 'the signal was never sent (Claude exited or no Stop event arrived); re-run'
		);
		results[label] = signalSentAt ? 'PASS' : 'INCONCLUSIVE';
		report.note(
			`${label}: the spec forbids signalling a non-idle session without confirmation whatever this shows; record the measured behavior in ADR-006/ADR-008.`
		);
	}
}

// ---------------------------------------------------------------------------------------------
/** @type {string[]} */
const plan =
	which === 'all'
		? ['argv', 'fork', 'exit', 'exit-bg', 'sigterm', 'sigterm-bg', 'sighup']
		: which === 'argv' || which === 'fork'
			? [which]
			: [`${which}${bgFlag ? '-bg' : ''}`];
for (const step of plan) {
	if (step === 'argv') await scenarioArgv();
	else if (step === 'fork') await scenarioFork();
	else {
		const bg = step.endsWith('-bg');
		const kind = /** @type {'exit' | 'sigterm' | 'sighup'} */ (step.replace('-bg', ''));
		await scenarioEnd(kind, bg);
	}
}

// ---- verdict -------------------------------------------------------------------------------------
const graceful = ['exit', 'exit-bg'].map((k) => results[k]);
const forkR = results['fork'];
const gracefulVerdict = graceful.every((r) => r === 'PASS')
	? 'PASS'
	: graceful.some((r) => r === 'FAIL')
		? 'FAIL'
		: 'INCONCLUSIVE';
const forkVerdict = forkR ?? 'INCONCLUSIVE';
report.note(
	`graceful end (Ctrl-C then /exit, with a draft and with background work): ${gracefulVerdict}${gracefulVerdict === 'INCONCLUSIVE' ? ' (not all runs done)' : ''}`
);
report.note(`fork IDs chosen by Foreman: ${forkVerdict}`);
/** @type {'PASS' | 'FAIL' | 'INCONCLUSIVE'} */
const verdict =
	gracefulVerdict === 'FAIL' || forkVerdict === 'FAIL'
		? 'FAIL'
		: gracefulVerdict === 'PASS' && forkVerdict === 'PASS'
			? 'PASS'
			: 'INCONCLUSIVE';
const failBits = [
	gracefulVerdict === 'FAIL'
		? 'graceful end: Billy ends the session, then Foreman relaunches (AC-F010-11)'
		: null,
	forkVerdict === 'FAIL'
		? 'fork IDs: the only new transcript after launch; ambiguous -> fresh chain with context (AC-F010-15)'
		: null
].filter(Boolean);
report.finish(
	verdict,
	verdict === 'PASS'
		? 'PASS: re-arm as specified (Ctrl-C then /exit, ≤ 10 s); fork IDs chosen by Foreman (--resume <uuid> --fork-session --session-id <new>)'
		: verdict === 'FAIL'
			? `FAIL: ${failBits.join('; ')}`
			: 'undecided: run the missing scenarios (pnpm spikes s8 all) and combine the pasted blocks',
	verdict === 'PASS'
		? [
				'docs/adr/ADR-008 (S-8 result: pass)',
				'docs/scope.md AC-F010-04, AC-F010-11, AC-F010-15 (branch recorded)',
				'rearm::ctrl_c_then_exit_order and chain::fork_argv stay as specified'
			]
		: [
				'docs/adr/ADR-008 (S-8 fail branch; Exit path taken)',
				'docs/scope.md AC-F010-04, AC-F010-11, AC-F010-15',
				'rearm::* and chain::fork_argv tests encode the branch (M3-14, M4-29)'
			]
);
