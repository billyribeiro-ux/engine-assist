// Spike S-7' (M0-15): settings `env` and `apiKeyHelper` in a trusted repo.
//
//   pnpm spikes s7                   run the case matrix (zero tokens), then inventory ~/.claude (metadata only)
//   pnpm spikes s7 --trusted         same, after you trusted the fixture folder (see the prompt it prints)
//   pnpm spikes s7 --prepare c03     only write case c03 into the fixture and print how to inspect it by hand
//   pnpm spikes s7 --inventory-only  only the ~/.claude and ~/.claude.json inventory
//
// Questions: (1) do SR-06/SR-07 (fingerprint + block list) catch every billing-changing input a trusted
// repo can set through `.claude/settings.json` (`env`, `apiKeyHelper`, ...)? (2) how big are Billy's real
// ~/.claude and ~/.claude.json, and which fingerprint sources are symlinks (ADR-021 caps and symlink rule)?
//
// Decision rule (MASTER-SPEC §9): rules suffice -> as specified. A miss -> also block any project `env`
// block until Billy approves it (AC-F008-07 is kept and built in M3-03).
// Zero tokens: only `claude auth status` runs. The fake secrets below are placeholders and the base URL
// points at a closed local port, so nothing leaves the machine.
import {
	chmodSync,
	existsSync,
	lstatSync,
	mkdirSync,
	readFileSync,
	readdirSync,
	readlinkSync,
	realpathSync,
	rmSync,
	statSync,
	writeFileSync
} from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';

import {
	Report,
	claudeConfigDir,
	findOnPath,
	makeSandbox,
	parseArgs,
	preflight,
	reportPreflight,
	runClaude,
	runSync,
	spikeEnv,
	writeJson
} from './lib.mjs';

const { opts, flags } = parseArgs(process.argv.slice(2), ['prepare']);
const report = new Report("S-7' settings env and apiKeyHelper in a trusted repo");

// ---- SR-07 as written in docs/requirements.md ---------------------------------------------------
const BLOCKED_KEYS = [
	'apiKeyHelper',
	'awsAuthRefresh',
	'awsCredentialExport',
	'gcpAuthRefresh',
	'otelHeadersHelper'
];
const BLOCKED_ENV = [
	/^ANTHROPIC_/,
	/^CLAUDE_CODE_/,
	/^CLAUDE_CONFIG_DIR$/,
	/^AWS_/,
	/^GOOGLE_/,
	/^CLOUDSDK_/,
	/^AZURE_/,
	/_PROXY$/,
	/_proxy$/,
	/^NODE_EXTRA_CA_CERTS$/,
	/^NODE_TLS_REJECT_UNAUTHORIZED$/,
	/^SSL_CERT_FILE$/,
	/^SSL_CERT_DIR$/,
	/^NODE_OPTIONS$/,
	/^DYLD_/
];

/**
 * What SR-07 would block in one parsed settings object.
 * @param {Record<string, unknown>} settings
 * @returns {string[]}
 */
function sr07Hits(settings) {
	/** @type {string[]} */
	const hits = [];
	for (const k of BLOCKED_KEYS) if (k in settings) hits.push(k);
	const env = settings['env'];
	if (env && typeof env === 'object') {
		for (const k of Object.keys(env))
			if (BLOCKED_ENV.some((re) => re.test(k))) hits.push(`env.${k}`);
	}
	return hits;
}

// ---- Case matrix ---------------------------------------------------------------------------------
const FAKE_KEY = ['sk', 'ant', 'spike', 'fake', 'not', 'a', 'real', 'key'].join('-');
const FAKE_TOKEN = ['spike', 'fake', 'token', 'not', 'real'].join('-');
const DEAD_URL = 'http://127.0.0.1:9';

/**
 * @typedef {object} Case
 * @property {string} id
 * @property {string} title
 * @property {'settings.json' | 'settings.local.json'} file
 * @property {(helper: string, sb: string) => Record<string, unknown>} settings
 * @property {boolean} [candidate] a key SR-07 does not name, tested to look for a miss
 */
/** @type {Case[]} */
const CASES = [
	{
		id: 'c00',
		title: 'baseline: no project settings',
		file: 'settings.json',
		settings: () => ({})
	},
	{
		id: 'c01',
		title: 'apiKeyHelper in .claude/settings.json',
		file: 'settings.json',
		settings: (h) => ({ apiKeyHelper: h })
	},
	{
		id: 'c02',
		title: 'apiKeyHelper in .claude/settings.local.json',
		file: 'settings.local.json',
		settings: (h) => ({ apiKeyHelper: h })
	},
	{
		id: 'c03',
		title: 'env.ANTHROPIC_API_KEY',
		file: 'settings.json',
		settings: () => ({ env: { ANTHROPIC_API_KEY: FAKE_KEY } })
	},
	{
		id: 'c04',
		title: 'env.ANTHROPIC_AUTH_TOKEN',
		file: 'settings.json',
		settings: () => ({ env: { ANTHROPIC_AUTH_TOKEN: FAKE_TOKEN } })
	},
	{
		id: 'c05',
		title: 'env.ANTHROPIC_BASE_URL (closed local port)',
		file: 'settings.json',
		settings: () => ({ env: { ANTHROPIC_BASE_URL: DEAD_URL } })
	},
	{
		id: 'c06',
		title: 'env.CLAUDE_CODE_USE_BEDROCK=1',
		file: 'settings.json',
		settings: () => ({ env: { CLAUDE_CODE_USE_BEDROCK: '1' } })
	},
	{
		id: 'c07',
		title: 'env.CLAUDE_CODE_USE_VERTEX=1',
		file: 'settings.json',
		settings: () => ({ env: { CLAUDE_CODE_USE_VERTEX: '1' } })
	},
	{
		id: 'c08',
		title: 'env.CLAUDE_CODE_OAUTH_TOKEN (fake)',
		file: 'settings.json',
		settings: () => ({ env: { CLAUDE_CODE_OAUTH_TOKEN: FAKE_TOKEN } })
	},
	{
		id: 'c09',
		title: 'env.CLAUDE_CONFIG_DIR (an empty alternate config dir)',
		file: 'settings.json',
		settings: (_h, sb) => ({ env: { CLAUDE_CONFIG_DIR: join(sb, 'alt-config') } })
	},
	{
		id: 'c10',
		title: 'env.HTTPS_PROXY (closed local port)',
		file: 'settings.json',
		settings: () => ({ env: { HTTPS_PROXY: DEAD_URL } })
	},
	{
		id: 'c11',
		title: 'env.AWS_PROFILE',
		file: 'settings.json',
		settings: () => ({ env: { AWS_PROFILE: 'spike' } })
	},
	{
		id: 'c12',
		title: 'awsAuthRefresh (fake command)',
		file: 'settings.json',
		settings: (h) => ({ awsAuthRefresh: h })
	},
	{
		id: 'c13',
		title: 'candidate: forceLoginMethod "console" (not named in SR-07)',
		file: 'settings.json',
		candidate: true,
		settings: () => ({ forceLoginMethod: 'console' })
	},
	{
		id: 'c14',
		title: 'candidate: env.HOME pointing at an empty dir (not named in SR-07)',
		file: 'settings.json',
		candidate: true,
		settings: (_h, sb) => ({ env: { HOME: join(sb, 'alt-home') } })
	},
	{
		id: 'c15',
		title: 'candidate: env.XDG_CONFIG_HOME (not named in SR-07)',
		file: 'settings.json',
		candidate: true,
		settings: (_h, sb) => ({ env: { XDG_CONFIG_HOME: join(sb, 'alt-xdg') } })
	},
	{
		id: 'c16',
		title: 'benign env.SPIKE_BENIGN (the fail branch would also block this)',
		file: 'settings.json',
		candidate: true,
		settings: () => ({ env: { SPIKE_BENIGN: '1' } })
	}
];

/**
 * Read the one trust boolean for `dir` (or an ancestor) from ~/.claude.json. Nothing else is kept.
 * @param {string} dir
 * @returns {boolean | null} null when it cannot be read
 */
function trustRecorded(dir) {
	const file = join(homedir(), '.claude.json');
	try {
		if (statSync(file).size > 64 * 1024 * 1024) return null;
		const j = /** @type {{ projects?: Record<string, { hasTrustDialogAccepted?: unknown }> }} */ (
			JSON.parse(readFileSync(file, 'utf8'))
		);
		let cur = dir;
		for (;;) {
			if (j.projects?.[cur]?.hasTrustDialogAccepted === true) return true;
			const up = join(cur, '..');
			if (up === cur || cur === '/') break;
			cur = up;
		}
		return false;
	} catch {
		return null;
	}
}

/** @param {string} text */
function normalize(text) {
	try {
		return JSON.stringify(JSON.parse(text), Object.keys(JSON.parse(text)).sort());
	} catch {
		return text.trim();
	}
}

/**
 * Field-level difference between two `claude auth status` outputs (it holds no secrets).
 * @param {string} a
 * @param {string} b
 */
function diffAuth(a, b) {
	try {
		const x = /** @type {Record<string, unknown>} */ (JSON.parse(a));
		const y = /** @type {Record<string, unknown>} */ (JSON.parse(b));
		/** @type {string[]} */
		const out = [];
		for (const k of new Set([...Object.keys(x), ...Object.keys(y)])) {
			if (JSON.stringify(x[k]) !== JSON.stringify(y[k])) {
				out.push(`${k}: ${JSON.stringify(x[k])} -> ${JSON.stringify(y[k])}`);
			}
		}
		return out.join('; ');
	} catch {
		return 'output not JSON';
	}
}

// ---- Part 2: inventory of the fingerprint sources (metadata only) ---------------------------------
const CAP_FILE = 256 * 1024;
const CAP_CLAUDE_JSON = 64 * 1024 * 1024;

/**
 * @typedef {object} Entry
 * @property {string} path
 * @property {string} kind file | dir | symlink | other
 * @property {number} size
 * @property {string} [target]
 * @property {string} [real]
 * @property {string} [targetKind]
 */

/**
 * @param {string} root
 * @param {Entry[]} out
 * @param {number} depth
 */
function walk(root, out, depth = 0) {
	if (out.length > 30_000 || depth > 10) return;
	let names;
	try {
		names = readdirSync(root);
	} catch {
		return;
	}
	for (const n of names) {
		const p = join(root, n);
		/** @type {Entry} */
		const e = describe(p);
		out.push(e);
		if (e.kind === 'dir') walk(p, out, depth + 1);
	}
}

/**
 * lstat one path; for a symlink also record the link text, the resolved target and its kind.
 * @param {string} p
 * @returns {Entry}
 */
function describe(p) {
	try {
		const l = lstatSync(p);
		if (l.isSymbolicLink()) {
			/** @type {Entry} */
			const e = { path: p, kind: 'symlink', size: 0, target: readlinkSync(p) };
			try {
				e.real = realpathSync(p);
				const t = statSync(p);
				e.targetKind = t.isFile() ? 'file' : t.isDirectory() ? 'dir' : 'other';
				e.size = t.size;
			} catch {
				e.targetKind = 'broken';
			}
			return e;
		}
		return { path: p, kind: l.isFile() ? 'file' : l.isDirectory() ? 'dir' : 'other', size: l.size };
	} catch {
		return { path: p, kind: 'missing', size: 0 };
	}
}

function inventory() {
	const cfg = claudeConfigDir();
	const home = homedir();
	/** @type {Entry[]} */
	const entries = [];
	for (const f of [
		join(cfg, 'settings.json'),
		join(cfg, 'settings.local.json'),
		join(cfg, 'CLAUDE.md'),
		join(home, 'CLAUDE.md'),
		join(home, 'CLAUDE.local.md'),
		join(home, '.claude.json'),
		join(cfg, '.claude.json'),
		'/Library/Application Support/ClaudeCode/managed-settings.json'
	]) {
		const e = describe(f);
		if (e.kind !== 'missing') entries.push(e);
	}
	for (const d of ['agents', 'skills', 'commands', 'rules', 'output-styles', 'plugins']) {
		const root = join(cfg, d);
		const e = describe(root);
		if (e.kind === 'missing') continue;
		entries.push(e);
		if (e.kind === 'dir') walk(root, entries);
	}
	const managedD = '/Library/Application Support/ClaudeCode/managed-settings.d';
	if (describe(managedD).kind === 'dir') walk(managedD, entries);

	const files = entries.filter(
		(e) => e.kind === 'file' || (e.kind === 'symlink' && e.targetKind === 'file')
	);
	const overCap = files.filter(
		(e) => e.size > (e.path.endsWith('.claude.json') ? CAP_CLAUDE_JSON : CAP_FILE)
	);
	const links = entries.filter((e) => e.kind === 'symlink');
	const dirLinks = links.filter((e) => e.targetKind !== 'file');
	const claudeJson = entries.find(
		(e) => e.path === join(home, '.claude.json') || e.path === join(cfg, '.claude.json')
	);
	const biggest = [...files].sort((a, b) => b.size - a.size).slice(0, 5);

	console.log(`\nInventory of fingerprint sources under ${cfg} (metadata only, no contents read):`);
	console.log(
		`  entries: ${String(entries.length)}, regular files (incl. through symlinks): ${String(files.length)}`
	);
	if (claudeJson) {
		console.log(
			`  ${claudeJson.path}: ${(claudeJson.size / 1024 / 1024).toFixed(2)} MB (cap 64 MB) ${claudeJson.kind === 'symlink' ? `[symlink -> ${claudeJson.real ?? claudeJson.target ?? '?'}]` : ''}`
		);
	} else {
		console.log('  no ~/.claude.json found');
	}
	console.log(
		`  largest files: ${biggest.map((e) => `${e.path} ${(e.size / 1024).toFixed(1)} KB`).join('; ') || 'none'}`
	);
	console.log(
		`  files over their cap (256 KB, or 64 MB for .claude.json): ${overCap.length === 0 ? 'none' : overCap.map((e) => `${e.path} ${(e.size / 1024).toFixed(1)} KB`).join('; ')}`
	);
	console.log(`  symlinks: ${links.length === 0 ? 'none' : ''}`);
	for (const l of links)
		console.log(
			`    ${l.path} -> ${l.target ?? '?'}  (resolves to ${l.real ?? 'BROKEN'}, ${l.targetKind ?? '?'})`
		);
	return {
		entries: entries.length,
		files: files.length,
		overCap,
		links,
		dirLinks,
		claudeJsonMb: claudeJson ? claudeJson.size / 1024 / 1024 : null
	};
}

// ---- main ----------------------------------------------------------------------------------------
const invOnly = flags.has('inventory-only');

if (invOnly) {
	const inv0 = inventory();
	report.add(
		'inventory of ~/.claude and ~/.claude.json',
		'INFO',
		`${String(inv0.entries)} entries, ${String(inv0.links.length)} symlinks, ${String(inv0.overCap.length)} over cap`
	);
	report.finish('INCONCLUSIVE', 'inventory only; run the case matrix too', [
		'docs/adr/ADR-021 (caps / symlink rule checked against this machine)'
	]);
	process.exit();
}

// not wiped between runs: Claude Code keys its trust record by path, so the fixture must keep its path
const sb = makeSandbox('s7', { fresh: false });
const fixture = join(sb, 'fixture-repo');
mkdirSync(join(fixture, '.claude'), { recursive: true });
const helper = join(fixture, 'fake-key-helper.sh');
const helperLog = join(sb, 'helper-ran.log');
if (!existsSync(join(fixture, '.git'))) {
	const git = findOnPath('git', process.env['PATH'] ?? '');
	if (git) runSync(git, ['init', '-q'], { cwd: fixture, timeoutMs: 15_000 }); // a repo, as the ticket says
}
writeFileSync(helper, `#!/bin/sh\necho ran >> '${helperLog}'\necho '${FAKE_KEY}'\n`);
chmodSync(helper, 0o755);
mkdirSync(join(sb, 'alt-config'), { recursive: true });
mkdirSync(join(sb, 'alt-home'), { recursive: true });
mkdirSync(join(sb, 'alt-xdg'), { recursive: true });

/** @param {Case} c */
function writeCase(c) {
	for (const f of ['settings.json', 'settings.local.json'])
		rmSync(join(fixture, '.claude', f), { force: true });
	if (Object.keys(c.settings(helper, sb)).length > 0)
		writeJson(join(fixture, '.claude', c.file), c.settings(helper, sb));
}

const prep = opts.get('prepare');
if (prep) {
	const c = CASES.find((x) => x.id === prep);
	if (!c) {
		console.log(`unknown case ${prep}; known: ${CASES.map((x) => x.id).join(', ')}`);
		process.exit(2);
	}
	writeCase(c);
	console.log(`Case ${c.id} (${c.title}) written to ${join(fixture, '.claude', c.file)}.`);
	console.log(
		`Inspect by hand (zero tokens):\n  cd '${fixture}'\n  claude auth status\n  claude        # then /status ; look at the auth line; /exit`
	);
	process.exit(0);
}

const pf = preflight(sb);
if (!reportPreflight(pf, report, flags.has('force'))) {
	report.finish('INCONCLUSIVE', 'none (preflight failed; fix the problem above and re-run)', []);
	process.exit();
}

const trusted = trustRecorded(fixture);
const isTrusted = flags.has('trusted') ? trusted !== false : trusted === true;
report.facts['fixture'] = fixture;
report.facts['trustRecorded'] = trusted;

console.log(`\nFixture repo: ${fixture}`);
console.log(
	`Trust recorded in ~/.claude.json: ${String(trusted)}${flags.has('trusted') ? ' (--trusted given)' : ''}`
);
if (!isTrusted) {
	console.log(
		"\nThe fixture is NOT trusted yet, so the matrix below measures an untrusted folder. For the real S-7' answer:\n" +
			`  1. cd '${fixture}' && claude     (accept the trust dialog, then /exit; zero tokens)\n` +
			'  2. pnpm spikes s7 --trusted\n'
	);
}

let baseline = '';
let baselineText = '';
/** @type {{ c: Case, hits: string[], changed: boolean, authMethod: string, exit: number | null, helperRan: boolean, diff: string }[]} */
const rows = [];
for (const c of CASES) {
	writeCase(c);
	rmSync(helperLog, { force: true });
	const r = runClaude(['auth', 'status'], { cwd: fixture, env: spikeEnv(), timeoutMs: 15_000 });
	const text = r.stdout || r.stderr;
	if (c.id === 'c00') {
		baseline = `${String(r.status)}|${normalize(text)}`;
		baselineText = text;
	}
	const sig = `${String(r.status)}|${normalize(text)}`;
	/** @type {string} */
	let authMethod;
	try {
		authMethod = String(/** @type {{ authMethod?: unknown }} */ (JSON.parse(r.stdout)).authMethod);
	} catch {
		authMethod = r.timedOut ? 'timeout' : 'unparsed';
	}
	const hits = sr07Hits(c.settings(helper, sb));
	const row = {
		c,
		hits,
		changed: c.id !== 'c00' && sig !== baseline,
		authMethod,
		exit: r.status,
		helperRan: existsSync(helperLog),
		diff: c.id !== 'c00' && sig !== baseline ? diffAuth(baselineText, text) : ''
	};
	rows.push(row);
	console.log(
		`  ${c.id}  authMethod=${authMethod.padEnd(12)} exit=${String(r.status).padEnd(4)} changed=${row.changed ? 'YES' : 'no '} SR-07=${hits.length > 0 ? `blocks (${hits.join(', ')})` : 'no block'}  helperRan=${row.helperRan ? 'yes' : 'no'}  ${c.title}`
	);
	if (row.diff) console.log(`        diff vs baseline: ${row.diff}`);
}
rmSync(join(fixture, '.claude', 'settings.json'), { force: true });
rmSync(join(fixture, '.claude', 'settings.local.json'), { force: true });
report.facts['baseline'] = baseline;

const missedAndChanged = rows.filter((r) => r.changed && r.hits.length === 0);
const keyed = rows.filter((r) => !r.c.candidate && r.c.id !== 'c00');
const specMisses = keyed.filter((r) => r.hits.length === 0);
const candidatesUnblocked = rows.filter((r) => r.c.candidate && r.hits.length === 0 && !r.changed);
const changedCases = rows.filter((r) => r.changed);

report.add(
	'baseline `claude auth status` (no project settings)',
	baseline.includes('oauth_token') || baseline.includes('claude.ai') ? 'PASS' : 'INCONCLUSIVE',
	baseline.slice(0, 140)
);
report.add(
	'every case SR-07 names is blocked by the rules as written',
	specMisses.length === 0 ? 'PASS' : 'FAIL',
	specMisses.length === 0
		? `${String(keyed.length)} cases, all blocked`
		: `not blocked: ${specMisses.map((r) => r.c.id).join(', ')}`
);
report.add(
	'cases that changed `auth status` vs baseline',
	'INFO',
	changedCases.length === 0
		? 'none: `claude auth status` did not react to any project setting (see the manual /status check in docs/spikes/S-7prime.md)'
		: changedCases
				.map((r) => `${r.c.id}: ${r.diff} (blocked: ${r.hits.length > 0 ? 'yes' : 'NO'})`)
				.join('; ')
);
report.add(
	'a billing-relevant input that auth status shows but SR-06/07 miss',
	missedAndChanged.length === 0 ? 'PASS' : 'FAIL',
	missedAndChanged.length === 0
		? 'none found'
		: missedAndChanged.map((r) => `${r.c.id} ${r.c.title}`).join('; ')
);
if (candidatesUnblocked.length > 0) {
	report.note(
		`Candidate keys SR-07 does not name and that showed no change in \`auth status\`: ${candidatesUnblocked.map((r) => r.c.id).join(', ')}. ` +
			'`auth status` may not load project env at all, so run the manual /status check for c13 and c14 before calling this a pass.'
	);
}
const helperCases = rows.filter((r) => r.helperRan).map((r) => r.c.id);
report.add(
	'project apiKeyHelper executed by `claude auth status`',
	'INFO',
	helperCases.length > 0
		? `yes, in ${helperCases.join(', ')} (${isTrusted ? 'trusted' : 'untrusted'} folder)`
		: `no (${isTrusted ? 'trusted' : 'untrusted'} folder)`
);

const inv = inventory();
report.add(
	'~/.claude.json size',
	inv.claudeJsonMb === null ? 'INFO' : inv.claudeJsonMb < 64 ? 'PASS' : 'FAIL',
	inv.claudeJsonMb === null
		? 'not found'
		: `${inv.claudeJsonMb.toFixed(2)} MB against the 64 MB cap`
);
report.add(
	'fingerprint files over the 256 KB cap',
	inv.overCap.length === 0 ? 'PASS' : 'FAIL',
	inv.overCap.length === 0 ? 'none' : inv.overCap.map((e) => e.path).join('; ')
);
report.add(
	'symlinks among fingerprint sources (ADR-021 rule check; does not change the branch)',
	inv.dirLinks.length === 0 ? 'INFO' : 'FAIL',
	inv.links.length === 0
		? 'none'
		: `${String(inv.links.length)} symlink(s): ${inv.links.map((l) => `${l.path} -> ${l.real ?? 'BROKEN'}`).join('; ')}${inv.dirLinks.length > 0 ? ' [dir/broken/non-file targets would block a launch under ADR-021]' : ''}`
);

/** @type {'PASS' | 'FAIL' | 'INCONCLUSIVE'} */
let verdict;
if (missedAndChanged.length > 0 || specMisses.length > 0) verdict = 'FAIL';
else if (!isTrusted) verdict = 'INCONCLUSIVE';
else verdict = 'PASS';
if (verdict === 'PASS') {
	report.note(
		'PASS covers what `claude auth status` can show. Also do the two-minute manual /status check in docs/spikes/S-7prime.md for c03 and c13 and tick it in the report.'
	);
}
report.finish(
	verdict,
	verdict === 'PASS'
		? 'PASS: the fingerprint and block rules suffice, as specified (AC-F008-07 stays conditional)'
		: verdict === 'FAIL'
			? 'FAIL: also block any project `env` block until Billy approves it (AC-F008-07 is kept and built in M3-03); add any missed non-env key to SR-07'
			: 'undecided: trust the fixture folder and re-run with --trusted',
	verdict === 'FAIL'
		? [
				'docs/scope.md AC-F008-07 (fail branch recorded)',
				'docs/requirements.md SR-07 (missed keys, if any)',
				'docs/adr/ADR-021 (fail branch / exit path)',
				'preflight::blocks_<key> tests in M3-03'
			]
		: [
				"docs/adr/ADR-021 (S-7' result; caps and symlink rule checked against this machine)",
				'docs/scope.md AC-F008-07 stays inactive'
			]
);
