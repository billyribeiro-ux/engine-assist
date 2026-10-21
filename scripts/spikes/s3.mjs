// Spike S-3 (M0-14): effort on resume.
//
//   pnpm spikes s3                    A: new session --effort low; B: --resume <uuid> --effort xhigh
//   pnpm spikes s3 --level max        test another level on resume (e.g. max, which effortLevel rejects)
//   pnpm spikes s3 --dry-run          print the argv and exit (no claude, no tokens)
//
// Question: does `claude --resume <uuid> --effort <level>` run the resumed session at <level>?
// Instrument: the hook input field `effort.level` (documented for Stop / PreToolUse / PostToolUse /
// SubagentStop) and $CLAUDE_EFFORT, both logged by a SessionStart..SessionEnd logger.
//
// Decision rule (MASTER-SPEC §9): flag works -> pass `--effort` on resume. Otherwise `effortLevel`
// through the run `--settings` file; if that fails (or the level is max/ultracode, which
// `effortLevel` rejects) Foreman shows `/effort <level>` with the "saves as your default" warning.
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';

import {
	Report,
	confirmTokens,
	effortOf,
	eventsOf,
	installHookLogger,
	loggerSettings,
	makeSandbox,
	newUuid,
	parseArgs,
	preflight,
	readHookLog,
	reportPreflight,
	runClaudeInteractive,
	spikeEnv,
	writeJson
} from './lib.mjs';

const { opts, flags } = parseArgs(process.argv.slice(2), ['level', 'first', 'model']);
const level = opts.get('level') ?? 'xhigh';
const first = opts.get('first') ?? 'low';
const model = opts.get('model') ?? 'opus';
const report = new Report('S-3 effort on resume');
const sb = makeSandbox('s3');
const cwd = join(sb, 'work');
mkdirSync(cwd, { recursive: true });
const logger = installHookLogger(join(sb, 'bin'));
const sessionId = newUuid();
const prompt = 'Reply with the single word: ok';

/**
 * @param {string} name
 * @param {Record<string, unknown>} [extra] extra settings keys
 */
function settingsFile(name, extra = {}) {
	const logFile = join(sb, `events-${name}.jsonl`);
	const file = join(sb, `settings-${name}.json`);
	writeJson(file, loggerSettings({ logger, logFile, extra }));
	return { file, logFile };
}

const A = settingsFile('A');
const B = settingsFile('B');
const C = settingsFile('C', { effortLevel: level });
const argsA = [
	'--session-id',
	sessionId,
	'--model',
	model,
	'--effort',
	first,
	'--permission-mode',
	'plan',
	'--name',
	'fm-s3-a',
	'--settings',
	A.file,
	prompt
];
const argsB = [
	'--resume',
	sessionId,
	'--model',
	model,
	'--effort',
	level,
	'--permission-mode',
	'plan',
	'--name',
	'fm-s3-b',
	'--settings',
	B.file,
	prompt
];
const argsC = [
	'--resume',
	sessionId,
	'--model',
	model,
	'--permission-mode',
	'plan',
	'--name',
	'fm-s3-c',
	'--settings',
	C.file,
	prompt
];

if (flags.has('dry-run')) {
	console.log(`cwd: ${cwd}`);
	for (const [n, a] of [
		['A (new, low)', argsA],
		['B (resume --effort)', argsB],
		['C (resume, effortLevel via --settings)', argsC]
	]) {
		console.log(
			`\n${String(n)}:\n  claude ${/** @type {string[]} */ (a).map((x) => JSON.stringify(x)).join(' ')}`
		);
	}
	process.exit(0);
}

if (!reportPreflight(preflight(sb), report, flags.has('force'))) {
	report.finish('INCONCLUSIVE', 'none (preflight failed; fix the problem above and re-run)', []);
	process.exit();
}

/**
 * Evaluate one phase's hook log.
 * @param {string} name
 * @param {string} logFile
 * @param {string} expectSource SessionStart source we expect
 */
function phase(name, logFile, expectSource) {
	const log = readHookLog(logFile);
	const start = eventsOf(log, 'SessionStart')[0];
	const stops = eventsOf(log, 'Stop');
	const levels = log
		.map((r) => ({ ev: r.label, level: effortOf(r) }))
		.filter((x) => x.level !== null);
	const lastStop = stops[stops.length - 1];
	const stopLevel = lastStop ? effortOf(lastStop) : null;
	const sourceOk = start?.payload['source'] === expectSource;
	const idOk = start?.payload['session_id'] === sessionId;
	return {
		name,
		log,
		start,
		stopLevel,
		levels,
		sourceOk,
		idOk,
		events: [...new Set(log.map((r) => r.label))]
	};
}

const ok = await confirmTokens(
	`Two short sessions (plan mode, "${prompt}", model ${model}), a few hundred tokens each, in ${cwd}.\n` +
		'  You will see the Claude Code TUI. Accept a trust prompt if it appears. When Claude answers "ok", type /exit.\n' +
		`  Phase A starts at --effort ${first}; phase B resumes the same session with --effort ${level}.`
);
if (!ok) {
	report.finish('INCONCLUSIVE', 'none (not run)', []);
	process.exit();
}

console.log(`\nPhase A: new session ${sessionId} at --effort ${first}`);
await runClaudeInteractive(argsA, { cwd, env: spikeEnv(), watchdogMs: 10 * 60_000 });
const pa = phase('A', A.logFile, 'startup');
if (pa.stopLevel === null) {
	report.add(
		'phase A instrument',
		'INCONCLUSIVE',
		`no Stop hook with an effort level (events seen: ${pa.events.join(', ') || 'none'}). Without it the spike cannot read the effort; use the manual check in docs/spikes/S-3.md`
	);
} else {
	report.add(
		'phase A instrument (new session)',
		pa.stopLevel === first ? 'PASS' : 'FAIL',
		`Stop hook reported effort ${pa.stopLevel}; --effort was ${first}`
	);
}

/** @type {'flag' | 'settings' | 'slash' | 'unknown'} */
let branch = 'unknown';
if (pa.stopLevel !== null) {
	console.log(`\nPhase B: --resume ${sessionId} --effort ${level}`);
	await runClaudeInteractive(argsB, { cwd, env: spikeEnv(), watchdogMs: 10 * 60_000 });
	const pb = phase('B', B.logFile, 'resume');
	report.add(
		'phase B resumed the right session',
		pb.sourceOk && pb.idOk ? 'PASS' : 'INCONCLUSIVE',
		`SessionStart source=${String(pb.start?.payload['source'])}, session_id ${pb.idOk ? 'matches' : 'does not match'}`
	);
	report.facts['phaseB_levels'] = pb.levels;
	if (pb.stopLevel === null) {
		report.add(
			'phase B effort',
			'INCONCLUSIVE',
			'no Stop hook with an effort level; did you answer and /exit?'
		);
	} else if (pb.stopLevel === level) {
		report.add(
			'phase B: `--effort` on resume',
			'PASS',
			`Stop hook reported ${pb.stopLevel} = requested ${level}`
		);
		branch = 'flag';
	} else {
		report.add(
			'phase B: `--effort` on resume',
			'FAIL',
			`requested ${level} but the resumed session ran at ${pb.stopLevel} (phase A was ${pa.stopLevel}, model default is medium on Opus 5.5)`
		);
		// Fail branch 1: effortLevel through --settings (docs: not accepted for max / ultracode).
		if (level === 'max' || level === 'ultracode') {
			report.add(
				'fail branch 1: `effortLevel` via --settings',
				'FAIL',
				`${level} is not accepted by effortLevel (low/medium/high/xhigh only), per docs`
			);
			branch = 'slash';
		} else {
			console.log(
				`\nPhase C: --resume ${sessionId} with effortLevel "${level}" in the --settings file, no --effort`
			);
			await runClaudeInteractive(argsC, { cwd, env: spikeEnv(), watchdogMs: 10 * 60_000 });
			const pc = phase('C', C.logFile, 'resume');
			if (pc.stopLevel === null) {
				report.add(
					'phase C: `effortLevel` via --settings',
					'INCONCLUSIVE',
					'no Stop hook with an effort level'
				);
			} else if (pc.stopLevel === level) {
				report.add(
					'phase C: `effortLevel` via --settings',
					'PASS',
					`Stop hook reported ${pc.stopLevel}`
				);
				branch = 'settings';
			} else {
				report.add(
					'phase C: `effortLevel` via --settings',
					'FAIL',
					`ran at ${pc.stopLevel}, wanted ${level}`
				);
				branch = 'slash';
			}
		}
	}
}
report.note(
	'Run again with --level max (and ultracode if used by a template) only if phase B passed for xhigh: effortLevel cannot carry those, so a fail for them means the /effort line.'
);
report.note(
	'Events seen in the real sessions (copy into the S-1prime report as the sensor event list): ' +
		[...new Set([...pa.events])].join(', ')
);

const verdicts = {
	flag: /** @type {const} */ ([
		'PASS',
		'PASS: `--effort <level>` is passed on resume (AC-F010-10 records "flag")'
	]),
	settings: /** @type {const} */ ([
		'FAIL',
		'FAIL, branch 1: `effortLevel` through the run --settings file (AC-F010-10 records "settings")'
	]),
	slash: /** @type {const} */ ([
		'FAIL',
		'FAIL, branch 2: Foreman shows the `/effort <level>` line with the "saves as your default" warning (AC-F010-10 records "slash")'
	]),
	unknown: /** @type {const} */ ([
		'INCONCLUSIVE',
		'undecided: re-run, or use the manual check in docs/spikes/S-3.md'
	])
};
const [verdict, text] = verdicts[branch];
report.finish(
	verdict,
	text,
	branch === 'flag'
		? [
				'docs/adr/ADR-008 (Effort on resume: pass)',
				'docs/scope.md AC-F010-10 (branch recorded)',
				'chain::effort_on_resume_branches (M3-14) encodes "flag"'
			]
		: [
				'docs/adr/ADR-008 (Effort on resume: the branch above)',
				'docs/scope.md AC-F010-10 (branch recorded)',
				'chain::effort_on_resume_branches (M3-14) encodes the branch',
				'docs/contracts/ipc.md only if the /effort line needs a new field (check)'
			]
);
