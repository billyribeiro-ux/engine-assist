// Spike S-0 (M0-12) helper. S-0 itself needs the real app on the Mac (docs/spikes/S-0.md); this script
// only prepares and measures the parts a script can:
//
//   pnpm spikes s0 fixtures            write the paste fixtures (5 KB text, the SR-28 fixture, a control-character text)
//   pnpm spikes s0 clip 5kb|sr28|ctrl  put one fixture on the clipboard (pbcopy)
//   pnpm spikes s0 pwn arm             delete /tmp/pwn so a later appearance means the paste executed
//   pnpm spikes s0 pwn check           PASS if /tmp/pwn still does not exist after the SR-28 paste
//   pnpm spikes s0 mem [--minutes 30] [--interval 60] [--match foreman]
//                                      sample memory of the app, its claude children and WebKit processes
//
// Decision rule (MASTER-SPEC §9): every S-0 check passes -> xterm 6 + portable-pty as specified. Any failure
// -> the fail-branch items in docs/spikes/S-0.md; a fundamental TUI failure reopens the shell choice.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { Report, makeSandbox, parseArgs, runSync } from './lib.mjs';

const { opts, rest } = parseArgs(process.argv.slice(2), ['minutes', 'interval', 'match']);
const cmd = rest[0] ?? 'help';

/** Fixtures as exact bytes. */
function fixtures() {
	// 5 KB of printable multi-line text, so a truncated or reordered paste is visible.
	let five = '';
	for (let i = 1; five.length < 5120; i++)
		five += `LINE ${String(i).padStart(3, '0')} the quick brown fox jumps over the lazy dog 0123456789\n`;
	five = `${five.slice(0, 5102)}\nEND-OF-5KB-PASTE\n`;
	return {
		'paste-5kb.txt': Buffer.from(five, 'utf8'),
		// SR-28 / AC-F009-15 fixture: ESC [ 2 0 1 ~ ends a bracketed paste early; ^C then "!touch /tmp/pwn" + CR would run.
		'paste-sr28.txt': Buffer.from('\x1b[201~\x03!touch /tmp/pwn\r', 'latin1'),
		// C0, DEL and C1 controls between letters: Foreman's handler must strip them with a visible notice.
		'paste-ctrl.txt': Buffer.from('a\x01b\x07c\x7fd\u0085e\u009bf\tg\nh\r\n', 'utf8')
	};
}

const pwnPaths = ['/tmp/pwn', '/private/tmp/pwn'];

if (cmd === 'fixtures') {
	const dir = join(makeSandbox('s0', { fresh: false }), 'fixtures');
	mkdirSync(dir, { recursive: true });
	for (const [name, buf] of Object.entries(fixtures())) {
		writeFileSync(join(dir, name), buf);
		console.log(
			`${join(dir, name)}  ${String(buf.length)} bytes  sha256 ${createHash('sha256').update(buf).digest('hex').slice(0, 16)}`
		);
	}
	console.log('\nLoad one on the clipboard with:  pnpm spikes s0 clip 5kb | sr28 | ctrl');
} else if (cmd === 'clip') {
	const which = rest[1] ?? '';
	const map = /** @type {Record<string, string>} */ ({
		'5kb': 'paste-5kb.txt',
		sr28: 'paste-sr28.txt',
		ctrl: 'paste-ctrl.txt'
	});
	const name = map[which];
	const buf = name ? Object.entries(fixtures()).find(([n]) => n === name)?.[1] : undefined;
	if (!buf) {
		console.log('usage: pnpm spikes s0 clip 5kb|sr28|ctrl');
		process.exit(2);
	}
	if (process.platform !== 'darwin') {
		console.log('pbcopy exists only on macOS; open the fixture file and copy it by hand.');
		process.exit(2);
	}
	const r = spawnSync('/usr/bin/pbcopy', [], { input: buf, shell: false });
	console.log(
		r.status === 0
			? `clipboard now holds ${name} (${String(buf.length)} bytes). Paste with the terminal focused.`
			: 'pbcopy failed'
	);
} else if (cmd === 'pwn') {
	const sub = rest[1];
	if (sub === 'arm') {
		for (const p of pwnPaths) rmSync(p, { force: true });
		console.log(
			'/tmp/pwn removed (or absent). Now paste the SR-28 fixture into a real Claude session in the app, then run: pnpm spikes s0 pwn check'
		);
	} else if (sub === 'check') {
		const report = new Report('S-0 SR-28 fixture paste');
		const found = pwnPaths.filter(
			(p) =>
				existsSync(p) ||
				(() => {
					try {
						return lstatSync(p).isSymbolicLink();
					} catch {
						return false;
					}
				})()
		);
		report.add(
			'/tmp/pwn does not exist after pasting `\\x1b[201~\\x03!touch /tmp/pwn\\r`',
			found.length === 0 ? 'PASS' : 'FAIL',
			found.length === 0
				? 'nothing ran'
				: `${found.join(', ')} EXISTS: the paste executed a command`
		);
		report.note(
			'Also confirm by eye that the TUI input box shows the sanitized text and Foreman showed a "removed N control characters" notice.'
		);
		report.finish(
			found.length === 0 ? 'PASS' : 'FAIL',
			found.length === 0
				? 'this check passes (pty::paste_strips_escape_sequences manual part)'
				: 'FAIL: the paste handler does not stop the escape; treat as a fundamental security failure of the sanitizing path and stop M1 until fixed (SR-28)',
			['docs/spikes/S-0.md', 'docs/adr/ADR-006 (sanitizing path)']
		);
	} else {
		console.log('usage: pnpm spikes s0 pwn arm|check');
		process.exit(2);
	}
} else if (cmd === 'mem') {
	const minutes = Number(opts.get('minutes') ?? '30');
	const interval = Number(opts.get('interval') ?? '60');
	const re = new RegExp(opts.get('match') ?? 'foreman', 'i');
	const out = join(makeSandbox('s0', { fresh: false }), 'mem.csv');
	writeFileSync(out, 'time,app_mb,claude_mb,webkit_mb,total_mb,claude_count\n');
	console.log(
		`Sampling every ${String(interval)} s for ${String(minutes)} min. App match: /${re.source}/i. Close Safari and other WebKit apps first (their WebContent processes are counted).`
	);
	console.log('Stop early with Ctrl-C; the summary still prints.\n');
	/** @type {{ app: number, claude: number, webkit: number, total: number, claudeCount: number }[]} */
	const samples = [];
	const sample = () => {
		const r = runSync('/bin/ps', ['-axo', 'pid=,ppid=,rss=,comm='], { timeoutMs: 15_000 });
		/** @type {{ pid: number, ppid: number, rss: number, comm: string }[]} */
		const procs = [];
		for (const line of r.stdout.split('\n')) {
			const m = /^\s*(\d+)\s+(\d+)\s+(\d+)\s+(.*)$/.exec(line);
			if (m?.[1] && m[2] && m[3] && m[4])
				procs.push({ pid: +m[1], ppid: +m[2], rss: +m[3], comm: m[4] });
		}
		const appPids = new Set(
			procs.filter((p) => re.test(p.comm) && !/spikes|node$/.test(p.comm)).map((p) => p.pid)
		);
		const desc = new Set(appPids);
		for (let changed = true; changed;) {
			changed = false;
			for (const p of procs)
				if (!desc.has(p.pid) && desc.has(p.ppid)) {
					desc.add(p.pid);
					changed = true;
				}
		}
		const mb = (/** @type {{ rss: number }[]} */ l) => l.reduce((a, p) => a + p.rss, 0) / 1024;
		const appProcs = procs.filter((p) => appPids.has(p.pid));
		const claudeProcs = procs.filter(
			(p) => desc.has(p.pid) && !appPids.has(p.pid) && /(^|\/)claude$/.test(p.comm)
		);
		const webkit = procs.filter((p) =>
			/com\.apple\.WebKit\.(WebContent|GPU|Networking)/.test(p.comm)
		);
		const s = {
			app: mb(appProcs),
			claude: mb(claudeProcs),
			webkit: mb(webkit),
			total: 0,
			claudeCount: claudeProcs.length
		};
		s.total = s.app + s.claude + s.webkit;
		samples.push(s);
		const t = new Date().toISOString();
		writeFileSync(
			out,
			`${t},${s.app.toFixed(0)},${s.claude.toFixed(0)},${s.webkit.toFixed(0)},${s.total.toFixed(0)},${String(s.claudeCount)}\n`,
			{ flag: 'a' }
		);
		console.log(
			`${t}  app ${s.app.toFixed(0)} MB  claude x${String(s.claudeCount)} ${s.claude.toFixed(0)} MB  WebKit ${s.webkit.toFixed(0)} MB  total ${s.total.toFixed(0)} MB`
		);
		if (appProcs.length === 0)
			console.log(
				'  (no process matched the app; is it running? use --match <part of its executable path>)'
			);
	};
	const finish = () => {
		const report = new Report('S-0 memory with 3 tabs (ps RSS)');
		const last = samples[samples.length - 1];
		const peak = Math.max(0, ...samples.map((s) => s.total));
		report.facts['csv'] = out;
		report.facts['samples'] = samples.length;
		report.facts['durationMinutes'] = (samples.length * interval) / 60;
		if (!last) {
			report.finish('INCONCLUSIVE', 'no samples', []);
			return;
		}
		const long = (samples.length - 1) * interval >= minutes * 60 * 0.95;
		const threes = last.claudeCount === 3;
		report.add(
			'ran for the full duration',
			long ? 'PASS' : 'INCONCLUSIVE',
			`${String(((samples.length - 1) * interval) / 60)} min of ${String(minutes)}`
		);
		report.add(
			'3 claude child processes',
			threes ? 'PASS' : 'INCONCLUSIVE',
			`${String(last.claudeCount)} found at the end (open exactly 3 tabs)`
		);
		report.add(
			'NFR-23: total <= 300 MB at the end (app + claude + WebKit RSS)',
			last.total <= 300 ? 'PASS' : 'FAIL',
			`end ${last.total.toFixed(0)} MB, peak ${peak.toFixed(0)} MB. RSS double-counts shared pages: cross-check with Activity Monitor (Memory column) and record both`
		);
		const ok = long && threes && last.total <= 300;
		report.finish(
			ok ? 'PASS' : last.total > 300 ? 'FAIL' : 'INCONCLUSIVE',
			ok
				? 'memory check passes (NFR-23 as specified)'
				: last.total > 300
					? `FAIL: memory NFR raised to measured + 20% (${(last.total * 1.2).toFixed(0)} MB); AC/NFR-23 updated in docs/scope.md`
					: 'undecided: rerun with 3 tabs for the full 30 minutes',
			['docs/spikes/S-0.md (memory row)', 'docs/scope.md NFR-23 (only on FAIL)']
		);
	};
	process.on('SIGINT', () => {
		finish();
		process.exit();
	});
	sample();
	const end = Date.now() + minutes * 60_000;
	while (Date.now() < end) {
		await new Promise((r) => setTimeout(r, interval * 1000));
		sample();
	}
	finish();
} else {
	console.log(
		'usage: pnpm spikes s0 <fixtures|clip|pwn|mem>   (see docs/spikes/S-0.md for the full checklist)'
	);
	process.exit(2);
}
