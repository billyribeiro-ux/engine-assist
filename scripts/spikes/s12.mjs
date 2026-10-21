// Spike S-12 (M0-17): PATH in a Finder-launched build.
//
//   pnpm spikes s12                  build the §6 PATH, resolve the five tools, and capture a LaunchServices PATH
//   pnpm spikes s12 --finder         same, but you double-click the stub app in Finder (the faithful way)
//   pnpm spikes s12 --from-file f    skip the stub; read the `env` output you captured yourself
//
// Question: with the PATH Foreman builds per MASTER-SPEC §6 / ADR-007 (`/etc/paths`, `/etc/paths.d/*`,
// `/opt/homebrew/bin`, `/opt/homebrew/sbin`, `~/.cargo/bin`, the pnpm home; sanitized; NO login-shell
// capture), do `claude`, `git`, `node`, `pnpm` and `cargo` resolve and run?
//
// How it sees "the PATH of a Finder-launched app" without launchctl: it writes a tiny unsigned stub
// bundle whose executable is `/usr/bin/env > <file>` and starts it through LaunchServices (`open -n -W`,
// or you double-click it with --finder). A LaunchServices-launched process gets launchd's environment,
// not Terminal's, which is exactly what Foreman gets from Finder.
//
// LIMITS (also in docs/spikes/S-12.md): (1) the stub is not the Foreman release build; the final
// confirmation is AC-F019-14 in M4-14 (Doctor shows the real PATH); (2) PATH building is re-implemented
// here from the spec text, not the Rust code; (3) it cannot know which folders are "registered projects"
// (SR-05), so that sanitizing rule is not applied; (4) a tool that exists only through nvm/fnm/volta/asdf/
// mise shims set up in a shell profile will correctly FAIL: that is the answer the spike wants.
//
// Decision rule (MASTER-SPEC §9): all five resolve -> PATH as specified. Any missing -> login-PATH
// capture is mandatory at first run, and AC-F019-13 moves into M2 (M4-14's capture work moves before M2-16).
import {
	chmodSync,
	existsSync,
	mkdirSync,
	readFileSync,
	readdirSync,
	statSync,
	writeFileSync
} from 'node:fs';
import { homedir } from 'node:os';
import { delimiter, join } from 'node:path';

import {
	Report,
	findOnPath,
	makeSandbox,
	parseArgs,
	reportPreflight,
	preflight,
	runSync,
	waitFor
} from './lib.mjs';

const { opts, flags } = parseArgs(process.argv.slice(2), ['from-file', 'method']);
const report = new Report('S-12 PATH in a Finder-launched build');
const sb = makeSandbox('s12');
const home = homedir();

/** @param {string} file */
function readLines(file) {
	try {
		return readFileSync(file, 'utf8')
			.split('\n')
			.map((l) => l.trim())
			.filter(Boolean);
	} catch {
		return [];
	}
}

/**
 * Build the PATH the way MASTER-SPEC §6 / ADR-007 describe, then sanitize it.
 * @returns {{ entries: string[], dropped: { entry: string, why: string }[] }}
 */
function buildForemanPath() {
	/** @type {string[]} */
	const raw = [...readLines('/etc/paths')];
	try {
		for (const f of readdirSync('/etc/paths.d').sort())
			raw.push(...readLines(join('/etc/paths.d', f)));
	} catch {
		// no /etc/paths.d
	}
	raw.push(
		'/opt/homebrew/bin',
		'/opt/homebrew/sbin',
		join(home, '.cargo', 'bin'),
		join(home, 'Library', 'pnpm')
	);
	/** @type {string[]} */
	const entries = [];
	/** @type {{ entry: string, why: string }[]} */
	const dropped = [];
	for (const e of raw) {
		if (!e.startsWith('/')) {
			dropped.push({ entry: e, why: 'not absolute' });
			continue;
		}
		try {
			const s = statSync(e);
			if (!s.isDirectory()) {
				dropped.push({ entry: e, why: 'not a directory' });
				continue;
			}
			if ((s.mode & 0o002) !== 0) {
				dropped.push({ entry: e, why: 'world-writable' });
				continue;
			}
		} catch {
			dropped.push({ entry: e, why: 'does not exist' });
			continue;
		}
		if (!entries.includes(e)) entries.push(e);
	}
	return { entries, dropped };
}

/** @param {string} text the output of `env` */
function pathFromEnv(text) {
	for (const line of text.split('\n')) if (line.startsWith('PATH=')) return line.slice(5);
	return null;
}

// ---- Finder / LaunchServices PATH -------------------------------------------------------------------
/** @type {string | null} */
let finderPath = null;
/** @type {Record<string, string>} */
const finderEnv = {};
let finderMethod = 'none';
const fromFile = opts.get('from-file');
if (fromFile) {
	const text = readFileSync(fromFile, 'utf8');
	finderPath = pathFromEnv(text);
	for (const l of text.split('\n')) {
		const i = l.indexOf('=');
		if (i > 0 && ['HOME', 'SHELL', 'USER', 'TMPDIR', 'LANG'].includes(l.slice(0, i)))
			finderEnv[l.slice(0, i)] = l.slice(i + 1);
	}
	finderMethod = `captured file ${fromFile}`;
} else if (process.platform === 'darwin') {
	const app = join(sb, 'FM Spike PATH.app');
	const out = join(sb, 'finder-env.txt');
	if (out.includes("'")) throw new Error('refusing a sandbox path containing a quote');
	mkdirSync(join(app, 'Contents', 'MacOS'), { recursive: true });
	writeFileSync(
		join(app, 'Contents', 'Info.plist'),
		`<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>fm-spike-path</string>
<key>CFBundleIdentifier</key><string>local.foreman.spike.path</string>
<key>CFBundleName</key><string>FM Spike PATH</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleVersion</key><string>1</string>
</dict></plist>
`
	);
	const exe = join(app, 'Contents', 'MacOS', 'fm-spike-path');
	writeFileSync(exe, `#!/bin/sh\n/usr/bin/env > '${out}' 2>&1\n`);
	chmodSync(exe, 0o755);
	runSync('/usr/bin/codesign', ['--force', '--sign', '-', app], { timeoutMs: 20_000 }); // best effort ad-hoc signature
	const finder = flags.has('finder') || opts.get('method') === 'finder';
	if (finder) {
		runSync('/usr/bin/open', ['-R', app], { timeoutMs: 10_000 });
		console.log(
			`\nFinder is showing the stub. Double-click "FM Spike PATH" now (a Gatekeeper prompt means right-click > Open). Waiting up to 3 minutes...`
		);
		await waitFor(() => existsSync(out) && statSync(out).size > 0, 180_000, 500);
		finderMethod = 'Finder double-click';
	} else {
		const r = runSync('/usr/bin/open', ['-n', '-W', app], { timeoutMs: 30_000 });
		finderMethod = 'open -n -W (LaunchServices)';
		if (r.status !== 0) console.log(`open exited ${String(r.status)}: ${r.stderr.trim()}`);
	}
	if (existsSync(out)) {
		const text = readFileSync(out, 'utf8');
		finderPath = pathFromEnv(text);
		for (const l of text.split('\n')) {
			const i = l.indexOf('=');
			if (i > 0 && ['HOME', 'SHELL', 'USER', 'TMPDIR', 'LANG'].includes(l.slice(0, i)))
				finderEnv[l.slice(0, i)] = l.slice(i + 1);
		}
		report.facts['finderEnvFile'] = out;
	}
}

// ---- Resolve ------------------------------------------------------------------------------------------
const built = buildForemanPath();
const builtPath = built.entries.join(delimiter);
const shellPath = process.env['PATH'] ?? '';
report.facts['finderMethod'] = finderMethod;
report.facts['finderPath'] = finderPath;
report.facts['foremanPath'] = built.entries;
report.facts['droppedFromPath'] = built.dropped;
report.facts['finderEnv'] = finderEnv;
console.log(
	`\nPATH as a LaunchServices/Finder app sees it (${finderMethod}): ${finderPath ?? 'NOT CAPTURED'}`
);
console.log(`PATH Foreman would build per §6:\n  ${built.entries.join('\n  ')}`);
if (built.dropped.length > 0)
	console.log(
		`dropped by sanitizing: ${built.dropped.map((d) => `${d.entry} (${d.why})`).join(', ')}`
	);

const pf = preflight(sb);
reportPreflight(pf, report, true);

const TOOLS = [
	{
		name: 'claude',
		allow: [join(home, '.local', 'bin', 'claude'), '/opt/homebrew/bin/claude'],
		version: ['-v']
	},
	{ name: 'git', allow: ['/usr/bin/git', '/opt/homebrew/bin/git'], version: ['--version'] },
	{ name: 'node', allow: [], version: ['--version'] },
	{ name: 'pnpm', allow: [], version: ['--version'] },
	{ name: 'cargo', allow: [], version: ['--version'] }
];
const cltOk =
	process.platform !== 'darwin' ||
	runSync('/usr/bin/xcode-select', ['-p'], { timeoutMs: 10_000 }).status === 0;
const runEnv = {
	HOME: home,
	USER: process.env['USER'] ?? '',
	LOGNAME: process.env['LOGNAME'] ?? '',
	TMPDIR: process.env['TMPDIR'] ?? '/tmp',
	LANG: process.env['LANG'] ?? 'en_US.UTF-8',
	PATH: builtPath
};

/** @type {{ tool: string, ok: boolean }[]} */
const outcome = [];
for (const t of TOOLS) {
	const viaBuilt = findOnPath(t.name, builtPath);
	const viaAllow = t.allow.find((p) => existsSync(p)) ?? null;
	const viaFinder = finderPath ? findOnPath(t.name, finderPath) : null;
	const viaShell = findOnPath(t.name, shellPath);
	const resolved = viaAllow ?? viaBuilt;
	let ran = 'not run';
	let runs = false;
	if (resolved) {
		if (t.name === 'git' && !cltOk && resolved === '/usr/bin/git') {
			ran =
				'skipped: Command Line Tools are not installed, so /usr/bin/git would open an install dialog';
		} else {
			const r = runSync(resolved, t.version, { cwd: sb, env: runEnv, timeoutMs: 20_000 });
			runs = r.status === 0;
			ran = runs
				? (r.stdout.trim().split('\n')[0] ?? 'ok').slice(0, 60)
				: `exit ${String(r.status)}: ${(r.stderr || r.stdout).trim().slice(0, 100)}`;
		}
	}
	let shebang = '';
	if (resolved) {
		try {
			const head = readFileSync(resolved, { encoding: 'utf8', flag: 'r' }).slice(0, 120);
			if (head.startsWith('#!')) shebang = (head.split('\n')[0] ?? '').trim();
		} catch {
			// binary
		}
	}
	const ok = resolved !== null && runs;
	outcome.push({ tool: t.name, ok });
	report.facts[`tool_${t.name}`] = {
		viaForemanPath: viaBuilt,
		viaAllowlist: viaAllow,
		viaFinderPathOnly: viaFinder,
		viaTerminalPath: viaShell,
		ran,
		shebang
	};
	report.add(
		`${t.name} resolves and runs with the §6 PATH`,
		ok ? 'PASS' : 'FAIL',
		`${resolved ?? 'NOT FOUND'}${shebang ? ` [${shebang}]` : ''}; runs: ${ran}. Terminal PATH finds: ${viaShell ?? 'nothing'}${finderPath ? `; Finder-only PATH finds: ${viaFinder ?? 'nothing'}` : ''}`
	);
	if (!resolved && viaShell) {
		report.note(
			`${t.name} exists at ${viaShell} in your terminal PATH but not on the §6 PATH: a login-shell PATH (version manager, custom prefix) is what supplies it. That is the S-12 fail branch.`
		);
	}
	if (
		resolved &&
		shebang.includes('env') &&
		!findOnPath('node', builtPath) &&
		/node/.test(shebang)
	) {
		report.note(
			`${t.name} starts with "${shebang}" but node is not on the §6 PATH, so it cannot run when launched from Foreman.`
		);
	}
}

if (!finderPath) {
	report.add(
		'captured a LaunchServices PATH',
		process.platform === 'darwin' ? 'INCONCLUSIVE' : 'INFO',
		'not captured (non-Mac, or the stub did not run). Use --finder, or `--from-file` with the output of `env` from a Finder-launched app. The §6 result above does not depend on it.'
	);
} else {
	report.add('captured a LaunchServices PATH', 'PASS', finderPath);
}

const allOk = outcome.every((o) => o.ok);
const missing = outcome.filter((o) => !o.ok).map((o) => o.tool);
const verdict = allOk ? 'PASS' : 'FAIL';
report.finish(
	verdict,
	allOk
		? 'PASS: PATH as specified (§6 without login-PATH capture resolves claude, git, node, pnpm, cargo)'
		: `FAIL (${missing.join(', ')}): login-PATH capture is mandatory at first run; AC-F019-13 moves into M2 (M4-14's capture work moves before M2-16)`,
	allOk
		? [
				'docs/adr/ADR-007 (S-12 result: pass)',
				'docs/scope.md AC-F019-14 stays in M4 (re-checked in M4-14 against the real Doctor PATH)'
			]
		: [
				'docs/scope.md AC-F019-13 (moves into M2) and AC-F019-14',
				'docs/tickets.md: move M4-14 capture work before M2-16',
				'docs/adr/ADR-007 (S-12 fail branch: login-PATH capture mandatory)',
				'docs/MASTER-SPEC.md §6 / §9 wording if Billy agrees'
			]
);
