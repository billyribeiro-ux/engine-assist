// Spike S-1' (M0-13): per-launch hooks as a contract probe.
//
//   pnpm spikes s1            zero-token probes, then (after y/N) one short real session for the deny-rule checks
//   pnpm spikes s1 --no-tokens   only the zero-token probes
//
// Question: does `claude --init-only --settings <file with exec-form hooks>` fire SessionStart in a
// probe folder? Plus two side questions of the same spike: do deny rules `Read(//<repo>/.foreman/**)`
// also deny the case variant `.FOREMAN/`, and do `//` deny rules parse when the repo path has a space?
//
// Decision rule (MASTER-SPEC §9): hooks fire -> the sensor ships in P2; they don't -> no sensor.
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
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
	runClaude,
	runClaudeInteractive,
	spikeEnv,
	writeJson
} from './lib.mjs';

const { flags } = parseArgs(process.argv.slice(2));
const report = new Report("S-1' per-launch hooks as a contract probe");
const sb = makeSandbox('s1');
console.log(`sandbox: ${sb}`);

if (!reportPreflight(preflight(sb), report, flags.has('force'))) {
	report.finish('INCONCLUSIVE', 'none (preflight failed; fix the problem above and re-run)', []);
	process.exit();
}

/**
 * One zero-token probe A: `claude --init-only` with Foreman's interactive flags and a marker hook.
 * @param {string} label
 * @param {string} appData directory holding the settings file, the logger and the marker (may contain a space)
 * @param {string} cwd probe folder (may contain a space)
 * @param {boolean} hooksDisabled negative control: disableAllHooks true, the marker must NOT appear
 */
function probeA(label, appData, cwd, hooksDisabled = false) {
	mkdirSync(appData, { recursive: true, mode: 0o700 });
	mkdirSync(cwd, { recursive: true, mode: 0o700 });
	const logger = installHookLogger(join(appData, 'bin'));
	const logFile = join(appData, 'marker.jsonl');
	const settings = join(appData, 'settings.json');
	writeJson(
		settings,
		loggerSettings({ logger, logFile, events: ['SessionStart'], disableAllHooks: hooksDisabled })
	);
	const id = newUuid();
	const args = [
		'--init-only',
		'--settings',
		settings,
		'--session-id',
		id,
		'--model',
		'opus',
		'--effort',
		'high',
		'--permission-mode',
		'plan',
		'--name',
		'fm-probe'
	];
	const r = runClaude(args, { cwd, env: spikeEnv(), timeoutMs: 20_000 });
	const log = readHookLog(logFile);
	const starts = eventsOf(log, 'SessionStart');
	return { label, id, r, starts, argv: `claude ${args.join(' ')}`, settings, cwd, logFile };
}

// ---- Probe A, plain paths ---------------------------------------------------------------------
console.log(
	'\nProbe A (zero tokens): claude --init-only with Foreman flags and a SessionStart marker hook'
);
const plain = probeA('plain paths', join(sb, 'appdata'), join(sb, 'probe-folder'));
const first = plain.starts[0];
const flagError = /unknown option|unrecognized|invalid value|error:|warning: unknown/i.test(
	plain.r.stderr
);
if (plain.r.timedOut) {
	report.add(
		'probe A, plain paths',
		'INCONCLUSIVE',
		`timed out after 20 s (a trust prompt?). Run \`cd ${plain.cwd} && claude\`, accept the trust dialog, /exit, then re-run this script`
	);
} else if (plain.r.status === 0 && first) {
	const p = first.payload;
	report.add(
		'probe A, plain paths',
		'PASS',
		`exit 0, SessionStart marker written; session_id ${String(p['session_id'] === plain.id ? 'equals' : 'differs from')} --session-id; source=${String(p['source'])}; session_title=${String(p['session_title'])}`
	);
	if (p['session_id'] !== plain.id) {
		report.add(
			'session id honored',
			'FAIL',
			`--session-id ${plain.id} but hook saw ${String(p['session_id'])}`
		);
	}
} else if (flagError) {
	report.add(
		'probe A, plain paths',
		'FAIL',
		`flag failure: ${plain.r.stderr.trim().slice(0, 300)}`
	);
} else {
	report.add(
		'probe A, plain paths',
		'FAIL',
		`exit ${String(plain.r.status)}, marker ${first ? 'present' : 'ABSENT'}; stderr: ${plain.r.stderr.trim().slice(0, 300)}`
	);
}
if (plain.r.status === 0 && /warning/i.test(plain.r.stderr)) {
	report.add(
		'probe A stderr warnings',
		'INFO',
		`exit 0 but claude warned (a flag value may be ignored; ADR-017 would call this a flag failure for the named flag): ${plain.r.stderr.trim().slice(0, 200)}`
	);
}
report.facts['probeA_argv'] = plain.argv;
if (first) report.facts['sessionStartPayload'] = first.payload;

// ---- Negative control ---------------------------------------------------------------------------
const neg = probeA('hooks disabled', join(sb, 'appdata-neg'), join(sb, 'probe-neg'), true);
report.add(
	'negative control (disableAllHooks: true)',
	neg.starts.length === 0 ? 'PASS' : 'FAIL',
	neg.starts.length === 0
		? 'no marker, so the marker in probe A really came from the hook'
		: 'marker written even with disableAllHooks:true; the probe cannot be trusted'
);

// ---- Probe A, app-data dir and probe folder with a space ------------------------------------------
console.log('\nProbe A again with a space in the app-data dir, the hook path and the probe folder');
const spaced = probeA(
	'paths with spaces',
	join(sb, 'Application Support', 'Foreman Probe'),
	join(sb, 'Trade Log probe')
);
report.add(
	'probe A, paths with spaces (exec form)',
	spaced.r.status === 0 && spaced.starts.length > 0
		? 'PASS'
		: spaced.r.timedOut
			? 'INCONCLUSIVE'
			: 'FAIL',
	spaced.r.status === 0 && spaced.starts.length > 0
		? 'marker written; exec-form `command`/`args` survive spaces in the settings path, hook path and cwd'
		: `exit ${String(spaced.r.status)}, marker ${spaced.starts.length > 0 ? 'present' : 'absent'}; stderr: ${spaced.r.stderr.trim().slice(0, 200)}`
);

// ---- Probe B -------------------------------------------------------------------------------------
const bId = newUuid();
const b = runClaude(['--init-only', '--resume', bId], {
	cwd: join(sb, 'probe-folder'),
	timeoutMs: 20_000
});
const bText = `${b.stdout}${b.stderr}`.trim();
if (b.timedOut)
	report.add('probe B (--init-only --resume <random uuid>)', 'INCONCLUSIVE', 'timed out');
else if (/unknown option|unrecognized|invalid value/i.test(bText)) {
	report.add(
		'probe B (--init-only --resume <random uuid>)',
		'FAIL',
		`flag failure: ${bText.slice(0, 200)}`
	);
} else if (b.status !== 0 && /no conversation|not found|no session/i.test(bText)) {
	report.add(
		'probe B (--init-only --resume <random uuid>)',
		'PASS',
		`exit ${String(b.status)}: ${bText.slice(0, 120)}`
	);
} else {
	report.add(
		'probe B (--init-only --resume <random uuid>)',
		'INFO',
		`exit ${String(b.status)}, output "${bText.slice(0, 120)}". ADR-017 expects a non-zero exit with a "no conversation found" message; ` +
			'if this is exit 0 with no output, probe B cannot tell a parsed flag from an ignored one: record it for ADR-017'
	);
}

// ---- Deny-rule checks (one short real session) ------------------------------------------------------
let denyDone = false;
if (!flags.has('no-tokens')) {
	const ok = await confirmTokens(
		'One short session (plan mode, a few thousand tokens) in a throwaway repo under ' +
			`${sb} asks Claude to Read 2 files: ${'.foreman/secret.txt and .FOREMAN/secret.txt'} while deny rules are active.\n` +
			'  You will see the Claude Code TUI. If it shows a trust prompt, accept it. When Claude has answered, type /exit.'
	);
	if (ok) {
		const denyResult = await denySession(join(sb, 'Trade Log repo'), 'path with a space');
		denyDone = true;
		if (denyResult === 'space-control-failed') {
			console.log(
				'\nThe lower-case control was not denied in the spaced path; re-running in a path without a space to isolate the cause.'
			);
			await denySession(join(sb, 'TradeLogRepo'), 'path without a space');
		}
	}
}

/**
 * @param {string} repo
 * @param {string} label
 * @returns {Promise<boolean | 'space-control-failed'>}
 */
async function denySession(repo, label) {
	mkdirSync(join(repo, '.foreman'), { recursive: true });
	const canary = `CANARY-${newUuid()}`;
	writeFileSync(join(repo, '.foreman', 'secret.txt'), `${canary}\n`);
	const lower = join(repo, '.foreman', 'secret.txt');
	const upper = join(repo, '.FOREMAN', 'secret.txt');
	const caseInsensitive = existsSync(upper);
	const appData = join(sb, `run-${label.replace(/\W+/g, '-')}`);
	mkdirSync(appData, { recursive: true });
	const logFile = join(appData, 'events.jsonl');
	const logger = installHookLogger(join(appData, 'bin'));
	const denyBase = `/${repo}`; // "//" + path without its leading slash: absolute from the filesystem root
	const settings = join(appData, 'settings.json');
	writeJson(
		settings,
		loggerSettings({
			logger,
			logFile,
			extra: {
				permissions: {
					disableBypassPermissionsMode: 'disable',
					deny: [`Read(${denyBase}/.foreman/**)`, `Edit(${denyBase}/.foreman/**)`]
				}
			}
		})
	);
	const id = newUuid();
	const prompt =
		`Use the Read tool on exactly these two absolute paths, one after the other, and do not retry any that fail: ` +
		`1) ${lower}  2) ${upper}  ` +
		'For each, answer in one line: READ <path> <first line> or DENIED <path>. No other text.';
	console.log(`\nStarting the session (${label}). deny rule: Read(${denyBase}/.foreman/**)`);
	const args = [
		'--session-id',
		id,
		'--model',
		'opus',
		'--effort',
		'low',
		'--permission-mode',
		'plan',
		'--name',
		'fm-s1-deny',
		'--settings',
		settings,
		prompt
	];
	await runClaudeInteractive(args, { cwd: repo, env: spikeEnv(), watchdogMs: 10 * 60_000 });
	const log = readHookLog(logFile);
	const reads = log.filter((r) => r.payload['tool_name'] === 'Read');
	/** @param {string} path */
	const outcome = (path) => {
		const pre = reads.filter(
			(r) => r.label === 'PreToolUse' && r.payload['tool_input']?.['file_path'] === path
		);
		const post = reads.filter(
			(r) =>
				r.label === 'PostToolUse' &&
				r.payload['tool_input']?.['file_path'] === path &&
				JSON.stringify(r.payload).includes(canary)
		);
		if (post.length > 0) return 'allowed';
		if (pre.length > 0) return 'denied';
		return 'not-attempted';
	};
	const lo = outcome(lower);
	const up = outcome(upper);
	report.facts[`deny_${label}`] = {
		rule: `Read(${denyBase}/.foreman/**)`,
		lower: lo,
		upper: up,
		caseInsensitiveFs: caseInsensitive
	};
	const events = new Set(log.map((r) => r.label));
	report.add(
		`hook events seen in a real session (${label})`,
		events.has('SessionStart') && events.has('Stop') ? 'PASS' : 'INCONCLUSIVE',
		`seen: ${[...events].join(', ')}; Stop reported effort ${String(eventsOf(log, 'Stop').map(effortOf)[0] ?? 'none')}`
	);
	report.add(
		`deny control: .foreman/ (lower case), ${label}`,
		lo === 'denied' ? 'PASS' : lo === 'allowed' ? 'FAIL' : 'INCONCLUSIVE',
		lo === 'denied'
			? 'Read was attempted and the canary never came back: the `//` rule parsed and denied'
			: lo === 'allowed'
				? 'the canary was read: the `//` deny rule did NOT apply'
				: 'Claude never attempted the Read'
	);
	if (label === 'path with a space') {
		report.add(
			'spaced repo path: `//` deny rule parses and denies',
			lo === 'denied' ? 'PASS' : lo === 'allowed' ? 'FAIL' : 'INCONCLUSIVE',
			lo === 'denied'
				? 'denied as specified'
				: 'see the control above; if a no-space run denies, the space is the cause (record the escaping rule in ADR-021 and M2-09)'
		);
	}
	if (!caseInsensitive) {
		report.add(
			'deny case variant .FOREMAN/',
			'INCONCLUSIVE',
			'this filesystem is case-sensitive, so .FOREMAN/secret.txt does not exist; run on the Mac (APFS default is case-insensitive)'
		);
	} else {
		report.add(
			'deny case variant .FOREMAN/',
			up === 'denied' ? 'PASS' : up === 'allowed' ? 'FAIL' : 'INCONCLUSIVE',
			up === 'denied'
				? 'the case variant is denied by the same rule'
				: up === 'allowed'
					? 'the canary was read through .FOREMAN/: list the case variants explicitly in the run settings (M2-09)'
					: 'Claude never attempted the Read'
		);
	}
	report.note(
		`Confirm by eye (${label}): the TUI should have shown "denied" for the lower-case path${caseInsensitive ? ' and for .FOREMAN' : ''}, and never printed ${canary}.`
	);
	return lo === 'allowed' && label === 'path with a space' ? 'space-control-failed' : true;
}

// ---- Verdict -------------------------------------------------------------------------------------
const aStatus = report.checks.find((c) => c.name === 'probe A, plain paths')?.status;
const negStatus = report.checks.find((c) => c.name.startsWith('negative control'))?.status;
/** @type {'PASS' | 'FAIL' | 'INCONCLUSIVE'} */
let verdict = 'INCONCLUSIVE';
if (aStatus === 'PASS' && negStatus === 'PASS') verdict = 'PASS';
else if (aStatus === 'FAIL') verdict = 'FAIL';
if (!denyDone) {
	report.note(
		'The deny-rule checks (case variant, spaced path) did not run; re-run without --no-tokens, or follow docs/spikes/S-1prime.md "Manual deny check".'
	);
}
report.note(
	'Only SessionStart can fire under --init-only. The other sensor events (Stop, SessionEnd, PermissionRequest ...) are logged during the real sessions of S-3 and S-8; copy their event lists into the S-3 and S-8 reports.'
);
report.finish(
	verdict,
	verdict === 'PASS'
		? 'PASS: per-launch hooks fire; the sensor ships in P2 (M4-07 to M4-10 stay)'
		: verdict === 'FAIL'
			? 'FAIL: hooks do not fire; no sensor, the manual idle confirmation stays'
			: 'undecided: re-run after fixing the first problem above',
	verdict === 'PASS'
		? [
				"docs/adr/ADR-009 (Status: Accepted + S-1' result)",
				'docs/adr/ADR-017 (probe B note, if any)',
				'docs/adr/ADR-021 (deny case variant / spaced-path result)',
				'M2-09 run settings (case variants / escaping, if the checks above say so)'
			]
		: [
				'docs/scope.md: remove AC-F011-* and the sensor part of AC-F010-14',
				'docs/tickets.md: cancel M4-07 to M4-10 except the P1 fallbacks',
				'docs/adr/ADR-009 (Status: Rejected / exit path taken)',
				'docs/adr/ADR-021 (deny-rule results still apply)'
			]
);
