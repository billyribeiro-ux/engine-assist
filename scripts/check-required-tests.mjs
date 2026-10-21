// Required-test coverage (informational step of `pnpm verify`).
//
//   node scripts/check-required-tests.mjs [--milestone=M1] [--list-file=<cargo test --list output>]
//
// Reads every ticket's "Required tests:" lines from docs/tickets.md, takes the `module::name`
// test names (a `module::*` entry needs at least one test in that module; `name_*`, `name_<key>`
// and `name_{a,b}` need at least one test whose name starts with `name_`), and checks them against
// `cargo test --workspace -- --list` plus the Vitest and Playwright tests whose title starts with
// the name (`it('terminal::options_snapshot', …)` in `src/**/*.test.ts`, `tests/**/*.test.ts`,
// `tests/flows/*.spec.ts`). A name matches a listed test that equals it or ends with `::<name>`.
// Missing names are printed for every milestone. The step fails for a ticket marked `done` in
// docs/progress.md, and for every ticket of a milestone marked `done` except those marked `billy`
// (left for Billy's Mac: manual runs and macOS-only checks).
import { spawnSync } from 'node:child_process';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const root = join(import.meta.dirname, '..');
const argv = process.argv.slice(2);
const opt = (/** @type {string} */ k) =>
	argv.find((a) => a.startsWith(`--${k}=`))?.slice(k.length + 3);
const onlyMilestone = opt('milestone');

/** @returns {Map<string, string[]>} ticket ID → required test names */
function requiredTests() {
	const text = readFileSync(join(root, 'docs', 'tickets.md'), 'utf8');
	const out = new Map();
	let ticket = '';
	let inRequired = false;
	for (const line of text.split('\n')) {
		const h = /^### \[(M\d+-\d+)\]/.exec(line);
		if (h) {
			ticket = h[1] ?? '';
			inRequired = false;
			out.set(ticket, []);
			continue;
		}
		if (!ticket) continue;
		if (line.startsWith('Required tests:')) {
			inRequired = true;
			continue;
		}
		if (inRequired && /^[A-Z][a-zA-Z ]*:/.test(line)) inRequired = false;
		if (line.startsWith('---') || line.startsWith('## ')) inRequired = false;
		if (!inRequired) continue;
		// Names outside parentheses only (parentheticals explain, they don't add tests).
		const bare = line.replace(/\([^)]*\)/g, '');
		for (const m of bare.matchAll(
			/\b([a-z_][a-z0-9_]*(?:::[a-z_][a-zA-Z0-9_]*)*::(?:[a-zA-Z_][a-zA-Z0-9_]*|\*))(\*|<[^>]*>|\{[^}]*\})?/g
		))
			out.get(ticket).push(m[2] ? `${m[1]}*` : m[1]);
	}
	return out;
}

/** @returns {Map<string, string>} milestone or ticket → status, from docs/progress.md tables */
function progress() {
	const s = new Map();
	/** @type {string} */
	let text;
	try {
		text = readFileSync(join(root, 'docs', 'progress.md'), 'utf8');
	} catch {
		return s;
	}
	for (const line of text.split('\n')) {
		const m = /^\|\s*(M\d+(?:-\d+)?)\s*\|\s*([a-z-]+)[^|]*\|/.exec(line);
		if (m) s.set(m[1], m[2]);
	}
	return s;
}

function listedTests() {
	const file = opt('list-file');
	let out;
	if (file) out = readFileSync(file, 'utf8');
	else {
		const r = spawnSync(
			'cargo',
			['test', '--workspace', '--locked', '--', '--list', '--format', 'terse'],
			{
				cwd: root,
				encoding: 'utf8',
				stdio: ['ignore', 'pipe', 'inherit'],
				maxBuffer: 64 * 1024 * 1024
			}
		);
		if (r.status !== 0) {
			process.stderr.write('cargo test -- --list failed\n');
			process.exit(r.status ?? 1);
		}
		out = r.stdout;
	}
	return out
		.split('\n')
		.filter((l) => l.endsWith(': test'))
		.map((l) => l.slice(0, -': test'.length));
}

/** Titles of Vitest and Playwright tests that start with a `module::name` test name. */
function jsTests() {
	/** @type {string[]} */
	const files = [];
	/** @param {string} dir */
	const walk = (dir) => {
		let names;
		try {
			names = readdirSync(dir);
		} catch {
			return;
		}
		for (const n of names) {
			if (n === 'node_modules' || n.startsWith('.')) continue;
			const p = join(dir, n);
			if (statSync(p).isDirectory()) walk(p);
			else if (/\.(test|spec)\.ts$/.test(n)) files.push(p);
		}
	};
	walk(join(root, 'src'));
	walk(join(root, 'tests'));
	const out = [];
	for (const f of files)
		for (const m of readFileSync(f, 'utf8').matchAll(
			/\b(?:it|test)(?:\.\w+)?\(\s*['"`]([a-z_][a-z0-9_]*(?:::[a-zA-Z_][a-zA-Z0-9_]*)+)/g
		))
			if (m[1]) out.push(m[1]);
	return out;
}

const tests = [...listedTests(), ...jsTests()];
/** @param {string} name */
const present = (name) => {
	if (name.endsWith('::*')) {
		const mod = name.slice(0, -1);
		return tests.some((t) => t.startsWith(mod) || t.includes(`::${mod}`));
	}
	if (name.endsWith('*')) {
		const prefix = name.slice(0, -1);
		return tests.some((t) => t.startsWith(prefix) || t.includes(`::${prefix}`));
	}
	return tests.some((t) => t === name || t.endsWith(`::${name}`));
};

const status = progress();
const byMilestone = new Map();
for (const [ticket, names] of requiredTests()) {
	const ms = ticket.split('-')[0] ?? '';
	if (onlyMilestone && ms !== onlyMilestone) continue;
	const missing = names.filter((n) => !present(n));
	const entry = byMilestone.get(ms) ?? { total: 0, missing: [] };
	entry.total += names.length;
	const ts = status.get(ticket);
	const enforced = ts === 'done' || (status.get(ms) === 'done' && ts !== 'billy');
	for (const n of missing) entry.missing.push({ text: `${ticket} ${n}`, enforced, ts });
	byMilestone.set(ms, entry);
}

let failed = false;
for (const [ms, { total, missing }] of byMilestone) {
	const done = status.get(ms) === 'done';
	process.stdout.write(
		`${ms}: ${total - missing.length}/${total} required tests present${done ? ' (milestone done)' : ''}\n`
	);
	for (const m of missing) {
		process.stdout.write(
			`  missing ${m.text}${m.ts === 'billy' ? " (left for Billy's Mac)" : ''}\n`
		);
		if (m.enforced) failed = true;
	}
}
process.exit(failed ? 1 : 0);
