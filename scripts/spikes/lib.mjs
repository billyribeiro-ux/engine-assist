// Shared helpers for the Stage 5 spike kit (M0-12..M0-18). Node 22 ESM, no dependencies.
//
// Safety rules every spike script gets from here:
//   - child processes are started with argv arrays only (never a shell string);
//   - `claude` only ever runs with a cwd inside a sandbox under <tmpdir>/foreman-spikes/, never in a
//     real project folder (assertSandboxCwd), and never with --bare, -p/--print or any bypass flag
//     (assertSafeClaudeArgs);
//   - the child environment is an allowlist like SR-05 (no ANTHROPIC_*, AWS_*, ... from the shell);
//   - anything that spends tokens asks for y/N on stdin first (confirmTokens);
//   - every script ends with a PASS / FAIL / INCONCLUSIVE verdict and the decision-rule branch it implies.
import { spawn, spawnSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import {
	accessSync,
	constants,
	existsSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	realpathSync,
	rmSync,
	statSync,
	writeFileSync
} from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { delimiter, dirname, join, resolve, sep } from 'node:path';
import { createInterface } from 'node:readline';

/** Minimum Claude Code version (ADR-017 floor with the current kit). */
export const FLOOR = '2.1.293';
const SANDBOX_MARKER = '.foreman-spike-sandbox';

/**
 * Hook events the logger subscribes to (AC-F011-01 list plus UserPromptSubmit and PreToolUse).
 * @type {readonly string[]}
 */
export const LOG_EVENTS = [
	'SessionStart',
	'UserPromptSubmit',
	'PreToolUse',
	'PostToolUse',
	'PermissionRequest',
	'Notification',
	'Stop',
	'StopFailure',
	'SubagentStart',
	'SubagentStop',
	'TaskCreated',
	'TaskCompleted',
	'SessionEnd'
];

/** @typedef {'PASS' | 'FAIL' | 'INCONCLUSIVE'} Verdict */
/** @typedef {'PASS' | 'FAIL' | 'INCONCLUSIVE' | 'INFO'} Status */
/** @typedef {{ name: string, status: Status, detail: string }} Check */
/**
 * @typedef {object} RunResult
 * @property {number | null} status
 * @property {NodeJS.Signals | null} signal
 * @property {string} stdout
 * @property {string} stderr
 * @property {boolean} timedOut
 * @property {number} ms
 * @property {string | null} error
 */

/** @param {unknown} e */
export function errMsg(e) {
	return e instanceof Error ? e.message : String(e);
}

/**
 * Tiny argv parser: `--name value`, `--flag`, `--name=value`; everything else is positional.
 * @param {string[]} argv
 * @param {readonly string[]} valueOptions option names that take a value
 * @returns {{ opts: Map<string, string>, flags: Set<string>, rest: string[] }}
 */
export function parseArgs(argv, valueOptions = []) {
	/** @type {Map<string, string>} */
	const opts = new Map();
	/** @type {Set<string>} */
	const flags = new Set();
	/** @type {string[]} */
	const rest = [];
	for (let i = 0; i < argv.length; i++) {
		const a = argv[i] ?? '';
		if (!a.startsWith('--')) {
			rest.push(a);
			continue;
		}
		const eq = a.indexOf('=');
		const name = eq === -1 ? a.slice(2) : a.slice(2, eq);
		if (eq !== -1) opts.set(name, a.slice(eq + 1));
		else if (valueOptions.includes(name)) opts.set(name, argv[++i] ?? '');
		else flags.add(name);
	}
	return { opts, flags, rest };
}

// ---------------------------------------------------------------------------------------------
// Sandboxes

/** The directory that holds every spike sandbox: <realpath tmpdir>/foreman-spikes. */
export function spikeRoot() {
	return join(realpathSync(tmpdir()), 'foreman-spikes');
}

/**
 * Create (or reset) a sandbox directory <tmp>/foreman-spikes/<name> with a marker file.
 * An existing directory is deleted only if it already carries the marker.
 * @param {string} name a single path segment, e.g. "s1"
 * @param {{ fresh?: boolean }} [options] fresh (default true) wipes an existing sandbox
 */
export function makeSandbox(name, options = {}) {
	if (!/^[A-Za-z0-9._-]+$/.test(name)) throw new Error(`bad sandbox name: ${name}`);
	const root = spikeRoot();
	mkdirSync(root, { recursive: true, mode: 0o700 });
	const dir = join(root, name);
	if (existsSync(dir) && (options.fresh ?? true)) {
		if (!existsSync(join(dir, SANDBOX_MARKER))) {
			throw new Error(`${dir} exists but is not a spike sandbox; refusing to delete it`);
		}
		rmSync(dir, { recursive: true, force: true });
	}
	mkdirSync(dir, { recursive: true, mode: 0o700 });
	const marker = join(dir, SANDBOX_MARKER);
	if (!existsSync(marker)) writeFileSync(marker, 'created by scripts/spikes; safe to delete\n');
	return realpathSync(dir);
}

/**
 * Throw unless `dir` is inside a spike sandbox (marker present) under the temp root, and no folder
 * above the temp root carries project configuration that Claude Code would pick up.
 * @param {string} dir
 */
export function assertSandboxCwd(dir) {
	const real = realpathSync(resolve(dir));
	const root = spikeRoot();
	if (!real.startsWith(root + sep)) {
		throw new Error(
			`refusing to use ${real}: not inside ${root} (spikes never run in a real project)`
		);
	}
	let cur = real;
	let marked = false;
	while (cur.startsWith(root + sep)) {
		if (existsSync(join(cur, SANDBOX_MARKER))) marked = true;
		cur = dirname(cur);
	}
	if (!marked) throw new Error(`refusing to use ${real}: no sandbox marker`);
	// cur is now the spike root; look at everything above it.
	const home = homedir();
	for (let up = cur; ; up = dirname(up)) {
		for (const name of ['.git', 'CLAUDE.md', 'CLAUDE.local.md', '.mcp.json', '.claude']) {
			if (name === '.claude' && up === home) continue; // user-level config, not a project
			if (existsSync(join(up, name))) {
				throw new Error(
					`refusing to use ${real}: ${join(up, name)} exists above the temp folder, so Claude Code ` +
						'would load it as project configuration. Set TMPDIR to a clean location.'
				);
			}
		}
		if (dirname(up) === up) break;
	}
	return real;
}

// ---------------------------------------------------------------------------------------------
// Environment and child processes

const ENV_ALLOW = new Set([
	'HOME',
	'USER',
	'LOGNAME',
	'SHELL',
	'TMPDIR',
	'LANG',
	'PATH',
	'TERM',
	'COLORTERM',
	'CLAUDE_CODE_OAUTH_TOKEN',
	'CLAUDE_CONFIG_DIR'
]);

/**
 * Environment for a spike child, built like SR-05: an allowlist, nothing else from the shell.
 * @param {Record<string, string>} [extra]
 * @returns {Record<string, string>}
 */
export function spikeEnv(extra = {}) {
	/** @type {Record<string, string>} */
	const env = {};
	for (const [k, v] of Object.entries(process.env)) {
		if (v === undefined) continue;
		if (ENV_ALLOW.has(k) || k.startsWith('LC_')) env[k] = v;
	}
	env['TERM'] ??= 'xterm-256color';
	env['COLORTERM'] ??= 'truecolor';
	return { ...env, ...extra };
}

/** Names of environment variables in the current shell that spikeEnv() drops and that change billing. */
export function droppedBillingEnv() {
	const re =
		/^(ANTHROPIC_|CLAUDE_CODE_(?!OAUTH_TOKEN$)|AWS_|GOOGLE_|GCLOUD_|CLOUDSDK_|AZURE_|DYLD_|NODE_OPTIONS$|NODE_EXTRA_CA_CERTS$|NODE_TLS_REJECT_UNAUTHORIZED$|SSL_CERT_)|_PROXY$|_proxy$/;
	return Object.keys(process.env).filter((k) => re.test(k));
}

/**
 * Throw if argv contains anything the spikes must never pass to `claude`.
 * @param {readonly string[]} args
 */
export function assertSafeClaudeArgs(args) {
	const banned = new Set([
		'--bare',
		'-p',
		'--print',
		'--dangerously-skip-permissions',
		'--allow-dangerously-skip-permissions',
		'bypassPermissions'
	]);
	for (const a of args) {
		if (banned.has(a)) throw new Error(`refusing to pass ${a} to claude`);
	}
}

/**
 * Search `pathValue` for an executable regular file called `name` (no shell, no `which`).
 * @param {string} name
 * @param {string} pathValue
 * @returns {string | null}
 */
export function findOnPath(name, pathValue) {
	for (const dir of pathValue.split(delimiter)) {
		if (!dir || !dir.startsWith('/')) continue;
		const candidate = join(dir, name);
		try {
			if (statSync(candidate).isFile()) {
				accessSync(candidate, constants.X_OK);
				return candidate;
			}
		} catch {
			// next
		}
	}
	return null;
}

/**
 * Run a program with an argv array, capture output, enforce a timeout. Never uses a shell.
 * @param {string} bin absolute path or a name found by the OS
 * @param {readonly string[]} args
 * @param {{ cwd?: string, env?: Record<string, string>, timeoutMs?: number }} [options]
 * @returns {RunResult}
 */
export function runSync(bin, args, options = {}) {
	const started = Date.now();
	const r = spawnSync(bin, [...args], {
		cwd: options.cwd,
		env: options.env ?? spikeEnv(),
		encoding: 'utf8',
		timeout: options.timeoutMs ?? 30_000,
		stdio: ['ignore', 'pipe', 'pipe'],
		shell: false,
		maxBuffer: 16 * 1024 * 1024
	});
	const err = r.error;
	return {
		status: r.status,
		signal: r.signal,
		stdout: r.stdout,
		stderr: r.stderr,
		timedOut: err !== undefined && 'code' in err && err.code === 'ETIMEDOUT',
		ms: Date.now() - started,
		error: err ? err.message : null
	};
}

/** @returns {string | null} absolute path of `claude` found on the terminal's PATH */
export function claudePath() {
	return findOnPath('claude', process.env['PATH'] ?? '');
}

/**
 * @param {string} a
 * @param {string} b
 * @returns {number} negative if a < b
 */
export function compareVersions(a, b) {
	const pa = a.split('.').map(Number);
	const pb = b.split('.').map(Number);
	for (let i = 0; i < 3; i++) {
		const d = (pa[i] ?? 0) - (pb[i] ?? 0);
		if (d !== 0) return d;
	}
	return 0;
}

/**
 * Run `claude` non-interactively (zero tokens: only init-only / auth / version style calls).
 * @param {string[]} args
 * @param {{ cwd: string, env?: Record<string, string>, timeoutMs?: number }} options
 */
export function runClaude(args, options) {
	assertSafeClaudeArgs(args);
	const bin = claudePath();
	if (!bin) throw new Error('claude not found on PATH');
	const cwd = assertSandboxCwd(options.cwd);
	return runSync(bin, args, { ...options, cwd });
}

/** Put the terminal back in a sane mode (a killed TUI can leave it raw). No-op without a TTY. */
export function resetTerminal() {
	if (!process.stdin.isTTY) return;
	spawnSync('stty', ['sane'], { stdio: ['inherit', 'ignore', 'ignore'], shell: false });
}

/**
 * Run `claude` interactively in the user's terminal (stdio inherited). Spends tokens, so callers
 * must have called confirmTokens(). The parent ignores SIGINT/SIGTSTP while the child runs so a
 * Ctrl-C reaches only Claude Code.
 * @param {string[]} args
 * @param {{ cwd: string, env?: Record<string, string>, watchdogMs?: number,
 *   onSpawn?: (child: import('node:child_process').ChildProcess) => void }} options
 * @returns {Promise<{ code: number | null, signal: NodeJS.Signals | null, startedAt: number, endedAt: number, watchdog: boolean }>}
 */
export function runClaudeInteractive(args, options) {
	assertSafeClaudeArgs(args);
	const bin = claudePath();
	if (!bin) throw new Error('claude not found on PATH');
	const cwd = assertSandboxCwd(options.cwd);
	return new Promise((resolvePromise, reject) => {
		const ignore = () => {
			/* keep running; the signal belongs to the child */
		};
		process.on('SIGINT', ignore);
		process.on('SIGTSTP', ignore);
		const startedAt = Date.now();
		const child = spawn(bin, args, {
			cwd,
			env: options.env ?? spikeEnv(),
			stdio: 'inherit',
			shell: false
		});
		let watchdog = false;
		const timer = setTimeout(
			() => {
				watchdog = true;
				child.kill('SIGTERM');
			},
			options.watchdogMs ?? 15 * 60_000
		);
		options.onSpawn?.(child);
		const cleanup = () => {
			clearTimeout(timer);
			process.off('SIGINT', ignore);
			process.off('SIGTSTP', ignore);
		};
		child.on('error', (e) => {
			cleanup();
			reject(e);
		});
		child.on('exit', (code, signal) => {
			cleanup();
			resetTerminal();
			resolvePromise({ code, signal, startedAt, endedAt: Date.now(), watchdog });
		});
	});
}

// ---------------------------------------------------------------------------------------------
// Prompts on stdin

/**
 * @param {string} question
 * @returns {Promise<string>}
 */
export function ask(question) {
	if (!process.stdin.isTTY) return Promise.resolve('');
	const rl = createInterface({ input: process.stdin, output: process.stdout });
	return new Promise((res) => {
		rl.question(question, (answer) => {
			rl.close();
			res(answer.trim());
		});
	});
}

/**
 * @param {string} question
 * @returns {Promise<boolean>} true only for an explicit y/yes
 */
export async function askYesNo(question) {
	const a = (await ask(`${question} [y/N] `)).toLowerCase();
	return a === 'y' || a === 'yes';
}

/**
 * Ask before any step that spends tokens. Returns false (and the caller must stop) unless the
 * answer is y/yes. Non-interactive stdin counts as "no".
 * @param {string} what what will run and roughly how much
 */
export async function confirmTokens(what) {
	console.log('\nTOKENS: this step starts a real Claude Code session on your subscription.');
	console.log(`  ${what}`);
	return askYesNo('Run it now?');
}

// ---------------------------------------------------------------------------------------------
// Hooks, settings, logs

/**
 * Copy the hook logger into `dir` (which may contain spaces) and return its path.
 * @param {string} dir
 */
export function installHookLogger(dir) {
	mkdirSync(dir, { recursive: true, mode: 0o700 });
	const dest = join(dir, 'hooklog.mjs');
	writeFileSync(dest, readFileSync(new URL('./hooklog.mjs', import.meta.url)));
	return dest;
}

/**
 * A settings object with exec-form hooks (`command` + `args`, no shell) for each event.
 * @param {{ logger: string, logFile: string, events?: readonly string[],
 *   extra?: Record<string, unknown>, disableAllHooks?: boolean }} o
 */
export function loggerSettings(o) {
	/** @type {Record<string, unknown[]>} */
	const hooks = {};
	for (const ev of o.events ?? LOG_EVENTS) {
		hooks[ev] = [
			{
				hooks: [{ type: 'command', command: process.execPath, args: [o.logger, o.logFile, ev] }]
			}
		];
	}
	return { disableAllHooks: o.disableAllHooks ?? false, hooks, ...(o.extra ?? {}) };
}

/**
 * @param {string} file
 * @param {unknown} value
 */
export function writeJson(file, value) {
	mkdirSync(dirname(file), { recursive: true, mode: 0o700 });
	writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
}

/**
 * @typedef {object} HookRecord
 * @property {number} t
 * @property {string | null} label
 * @property {{ CLAUDE_EFFORT: string | null }} env
 * @property {Record<string, any>} payload
 */

/**
 * Read a hook log written by hooklog.mjs. A missing file is an empty log.
 * @param {string} file
 * @returns {HookRecord[]}
 */
export function readHookLog(file) {
	if (!existsSync(file)) return [];
	/** @type {HookRecord[]} */
	const out = [];
	for (const line of readFileSync(file, 'utf8').split('\n')) {
		if (!line.trim()) continue;
		try {
			const rec = /** @type {HookRecord} */ (JSON.parse(line));
			if (typeof rec.payload !== 'object' || rec.payload === null) rec.payload = {};
			out.push(rec);
		} catch {
			// a torn line; ignore
		}
	}
	return out;
}

/**
 * @param {HookRecord[]} log
 * @param {string} event
 */
export function eventsOf(log, event) {
	return log.filter((r) => r.label === event || r.payload['hook_event_name'] === event);
}

/**
 * The effort level a hook reported: payload.effort.level, else the $CLAUDE_EFFORT variable.
 * @param {HookRecord} rec
 * @returns {string | null}
 */
export function effortOf(rec) {
	const eff = rec.payload['effort'];
	if (eff && typeof eff === 'object' && typeof eff.level === 'string') return eff.level;
	return rec.env.CLAUDE_EFFORT;
}

/**
 * Wait until `predicate()` is true or the timeout passes.
 * @param {() => boolean} predicate
 * @param {number} timeoutMs
 * @param {number} [stepMs]
 */
export async function waitFor(predicate, timeoutMs, stepMs = 250) {
	const end = Date.now() + timeoutMs;
	while (Date.now() < end) {
		if (predicate()) return true;
		await new Promise((r) => setTimeout(r, stepMs));
	}
	return predicate();
}

/** @returns {string} the directory Claude Code keeps transcripts in */
export function claudeConfigDir() {
	return process.env['CLAUDE_CONFIG_DIR'] ?? join(homedir(), '.claude');
}

/**
 * Newest-first list of transcript files in the project folder for `cwd`.
 * Claude Code names the folder after the cwd with every non-alphanumeric character as "-".
 * @param {string} cwd
 * @returns {{ file: string, size: number, mtimeMs: number }[]}
 */
export function transcriptsFor(cwd) {
	const dir = join(claudeConfigDir(), 'projects', cwd.replace(/[^A-Za-z0-9]/g, '-'));
	return listTranscripts(dir);
}

/**
 * @param {string} dir
 * @returns {{ file: string, size: number, mtimeMs: number }[]}
 */
export function listTranscripts(dir) {
	/** @type {{ file: string, size: number, mtimeMs: number }[]} */
	const out = [];
	let names;
	try {
		names = readdirSync(dir);
	} catch {
		return out;
	}
	for (const n of names) {
		if (!n.endsWith('.jsonl')) continue;
		const file = join(dir, n);
		try {
			const s = statSync(file);
			out.push({ file, size: s.size, mtimeMs: s.mtimeMs });
		} catch {
			// vanished
		}
	}
	return out.sort((x, y) => y.mtimeMs - x.mtimeMs);
}

// ---------------------------------------------------------------------------------------------
// Preflight

/**
 * Check version, platform and auth before a spike runs. Returns problems (empty = good to go) and
 * the facts, which every report records.
 * @param {string} cwd a sandbox directory
 */
export function preflight(cwd) {
	/** @type {string[]} */
	const problems = [];
	/** @type {Record<string, string>} */
	const facts = {
		node: process.version,
		platform: `${process.platform} ${process.arch}`
	};
	if (process.platform !== 'darwin') {
		problems.push(`platform is ${process.platform}; the spikes decide things about Billy's Mac`);
	}
	const bin = claudePath();
	if (!bin) {
		problems.push('claude is not on PATH');
		return { problems, facts, claude: null };
	}
	facts['claudePath'] = bin;
	const v = runClaude(['-v'], { cwd, timeoutMs: 15_000 });
	const m = /(\d+\.\d+\.\d+)/.exec(v.stdout);
	if (!m?.[1]) {
		problems.push(`could not read the claude version from: ${v.stdout.trim() || v.stderr.trim()}`);
		return { problems, facts, claude: bin };
	}
	facts['claudeVersion'] = m[1];
	if (compareVersions(m[1], FLOOR) < 0) problems.push(`claude ${m[1]} is below the floor ${FLOOR}`);
	const a = runClaude(['auth', 'status'], { cwd, timeoutMs: 15_000 });
	try {
		const j = /** @type {{ authMethod?: unknown, loggedIn?: unknown }} */ (JSON.parse(a.stdout));
		facts['authMethod'] = String(j.authMethod);
		if (j.authMethod !== 'claude.ai' && j.authMethod !== 'oauth_token') {
			problems.push(
				`auth method is ${String(j.authMethod)}; the spikes must run on the subscription (claude.ai / oauth_token)`
			);
		}
	} catch {
		problems.push(`could not parse \`claude auth status\` output (exit ${String(a.status)})`);
	}
	const dropped = droppedBillingEnv();
	if (dropped.length > 0) {
		const shown = dropped.slice(0, 6).join(', ');
		facts['droppedEnv'] =
			`${String(dropped.length)} variable(s) not passed to claude: ${shown}${dropped.length > 6 ? ', ...' : ''}`;
	}
	return { problems, facts, claude: bin };
}

// ---------------------------------------------------------------------------------------------
// Reporting

export class Report {
	/** @param {string} title e.g. "S-3 effort on resume" */
	constructor(title) {
		this.title = title;
		/** @type {Check[]} */
		this.checks = [];
		/** @type {string[]} */
		this.notes = [];
		/** @type {Record<string, unknown>} */
		this.facts = {};
	}

	/**
	 * @param {string} name
	 * @param {Status} status
	 * @param {string} detail
	 */
	add(name, status, detail) {
		this.checks.push({ name, status, detail });
		console.log(`  [${status}] ${name}: ${detail}`);
	}

	/** @param {string} line */
	note(line) {
		this.notes.push(line);
		console.log(`  note: ${line}`);
	}

	/**
	 * Print the verdict, the branch and a Markdown block to paste into docs/spikes/<id>.md.
	 * @param {Verdict} verdict
	 * @param {string} branch the decision-rule branch the verdict implies
	 * @param {string[]} changes documents that branch changes
	 */
	finish(verdict, branch, changes) {
		const line = '='.repeat(78);
		console.log(`\n${line}\n${this.title}: ${verdict}\n${line}`);
		if (process.platform !== 'darwin') {
			console.log(
				`NOT DECISIVE: this ran on ${process.platform}, not on Billy's Mac. Treat it as a rehearsal and re-run on the Mac.`
			);
		}
		console.log(`Branch implied: ${branch}`);
		console.log('Documents this branch changes:');
		for (const c of changes) console.log(`  - ${c}`);
		console.log('\n--- paste into the Results section of the report ---');
		console.log(`Verdict: ${verdict}`);
		console.log(`Branch: ${branch}`);
		console.log(`Run at: ${new Date().toISOString()}`);
		for (const [k, v] of Object.entries(this.facts)) console.log(`${k}: ${JSON.stringify(v)}`);
		console.log('\n| Check | Result | Detail |\n| --- | --- | --- |');
		for (const c of this.checks) {
			console.log(
				`| ${c.name} | ${c.status} | ${c.detail.replace(/\|/g, '\\|').replace(/\n/g, ' ')} |`
			);
		}
		for (const n of this.notes) console.log(`\nNote: ${n}`);
		console.log('--- end ---\n');
		process.exitCode = verdict === 'PASS' ? 0 : verdict === 'FAIL' ? 1 : 2;
	}
}

/**
 * Print preflight problems and stop the script when there are any (unless `force`).
 * @param {ReturnType<typeof preflight>} pf
 * @param {Report} report
 * @param {boolean} force
 * @returns {boolean} true when the script may continue
 */
export function reportPreflight(pf, report, force) {
	Object.assign(report.facts, pf.facts);
	console.log(`preflight: ${JSON.stringify(pf.facts)}`);
	if (pf.facts['droppedEnv']) {
		console.log(`  note (same as Foreman, SR-05): ${pf.facts['droppedEnv']}`);
	}
	if (pf.problems.length === 0) return true;
	for (const p of pf.problems) console.log(`  PROBLEM: ${p}`);
	if (force) {
		console.log('  --force given: continuing anyway; say so in the report.');
		return true;
	}
	return false;
}

export { randomUUID as newUuid };
