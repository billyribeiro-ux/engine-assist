// Dispatcher behind `pnpm spikes <id> [args]` (M0-12..M0-18). One npm script, seven spikes.
//
//   pnpm spikes list           the spikes, their order and what each needs
//   pnpm spikes check          prerequisites only (Claude Code version, login, platform; zero tokens)
//   pnpm spikes s1 | s3 | s7 | s8 | s12 | s23 | s0 [args]
//
// Ids are forgiving: s1, s1p, s1prime, S-1', "S-7′" all work.
import { spawnSync } from 'node:child_process';
import { join } from 'node:path';

import { Report, makeSandbox, preflight, reportPreflight } from './lib.mjs';

const here = import.meta.dirname;
/** @type {Record<string, { file: string, title: string, minutes: string, tokens: string }>} */
const SPIKES = {
	s0: {
		file: 's0.mjs',
		title: 'PTY, xterm 6, real TUI, strict CSP (needs the app; helper only)',
		minutes: '60-90',
		tokens: 'only the TUI sessions you open'
	},
	s1prime: {
		file: 's1prime.mjs',
		title: 'per-launch hooks as a contract probe + deny rules',
		minutes: '5',
		tokens: 'zero, plus one tiny session (y/N)'
	},
	s3: {
		file: 's3.mjs',
		title: 'effort on resume',
		minutes: '5',
		tokens: 'two tiny sessions (y/N)'
	},
	s7prime: {
		file: 's7prime.mjs',
		title: 'settings env and apiKeyHelper in a trusted repo',
		minutes: '5',
		tokens: 'zero'
	},
	s8: {
		file: 's8.mjs',
		title: 'graceful end, signals and forks',
		minutes: '15-25',
		tokens: 'one tiny session per scenario (y/N each)'
	},
	s12: { file: 's12.mjs', title: 'PATH in a Finder-launched build', minutes: '3', tokens: 'zero' },
	s23: {
		file: 's23.mjs',
		title: 'claude --desktop --resume',
		minutes: '10',
		tokens: 'one tiny CLI session plus one Desktop message'
	}
};

/** @param {string} raw */
function normalize(raw) {
	const t = raw
		.toLowerCase()
		.replace(/[-_′'\s]/g, '')
		.replace('prime', 'p');
	if (t === 's1' || t === 's1p') return 's1prime';
	if (t === 's7' || t === 's7p') return 's7prime';
	return t;
}

const [first, ...args] = process.argv.slice(2);
if (!first || first === 'list' || first === 'help' || first === '--help') {
	console.log(
		'Stage 5 spike kit. Run on the Mac, in this order: S-0 first, then the rest in any order.\n'
	);
	for (const [id, s] of Object.entries(SPIKES)) {
		console.log(
			`  pnpm spikes ${id.replace('prime', '').padEnd(8)} ~${s.minutes.padEnd(6)} min  ${s.title}\n  ${' '.repeat(22)}tokens: ${s.tokens}`
		);
	}
	console.log('\n  pnpm spikes check      prerequisites only\nDetails: docs/spikes/README.md');
	process.exit(0);
}
if (first === 'check') {
	const report = new Report('spike prerequisites');
	const sb = makeSandbox('check');
	const ok = reportPreflight(preflight(sb), report, false);
	console.log(ok ? '\nready.' : '\nfix the problems above first.');
	process.exit(ok ? 0 : 1);
}
const id = normalize(first);
const spike = SPIKES[id];
if (!spike) {
	console.log(`unknown spike "${first}". Try: pnpm spikes list`);
	process.exit(2);
}
const r = spawnSync(process.execPath, [join(here, spike.file), ...args], {
	stdio: 'inherit',
	shell: false
});
process.exit(r.status ?? 1);
