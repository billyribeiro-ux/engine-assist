// Required-test coverage (informational step of `pnpm verify`).
//
//   node scripts/check-required-tests.mjs [--milestone=M1] [--list-file=<cargo test --list output>]
//
// Reads every ticket's "Required tests:" lines from docs/tickets.md, takes the `module::name`
// test names (a `module::*` entry needs at least one test in that module), and checks them against
// `cargo test --workspace -- --list`. A name matches a listed test that equals it or ends with
// `::<name>`. A name ending in `_` or `_*` (`ipc::fuzz_*`, `confirm_required_for_{a,b}`) is a prefix:
// at least one test must start with it. Tests written in Vitest (`it('render::escapes_html', …)`
// under src/ or tests/, ticket files "src/lib/render/…") count too, by their quoted title; they are
// found by reading the test files, not by running them (`pnpm test` runs them). Missing names are printed for every milestone; the step fails only for milestones
// marked `done` in docs/progress.md.
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
			/\b([a-z_][a-z0-9_]*(?:::[a-z_][a-z0-9_]*)*::(?:[a-z_][a-z0-9_]*\*?|\*))/g
		))
			out.get(ticket).push(m[1]);
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
		const m = /^\|\s*(M\d+(?:-\d+)?)\s*\|\s*([a-z-]+)\s*\|/.exec(line);
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

/** Quoted `module::name` titles of Vitest `it(...)`/`test(...)` calls under src/ and tests/. */
function vitestTitles() {
	const out = [];
	/** @param {string} dir */
	const walk = (dir) => {
		for (const name of readdirSync(dir)) {
			if (name === 'node_modules' || name.startsWith('.')) continue;
			const p = join(dir, name);
			if (statSync(p).isDirectory()) walk(p);
			else if (/\.(test|spec)\.ts$/.test(name)) {
				const text = readFileSync(p, 'utf8');
				for (const m of text.matchAll(
					/\b(?:it|test)(?:\.\w+)*\(\s*['"`]([a-z_][a-z0-9_]*(?:::[a-z_][a-z0-9_]*)+)['"`]/g
				))
					out.push(m[1]);
			}
		}
	};
	for (const d of ['src', 'tests']) walk(join(root, d));
	return out;
}

const tests = [...listedTests(), ...vitestTitles()];
/** @param {string} name */
const present = (name) => {
	if (name.endsWith('::*')) {
		const mod = name.slice(0, -1);
		return tests.some((t) => t.startsWith(mod) || t.includes(`::${mod}`));
	}
	if (name.endsWith('*') || name.endsWith('_')) {
		const prefix = name.replace(/\*$/, '');
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
	for (const n of missing) entry.missing.push(`${ticket} ${n}`);
	byMilestone.set(ms, entry);
}

let failed = false;
for (const [ms, { total, missing }] of byMilestone) {
	const done = status.get(ms) === 'done';
	process.stdout.write(
		`${ms}: ${total - missing.length}/${total} required tests present${done ? ' (milestone done)' : ''}\n`
	);
	if (missing.length) {
		for (const m of missing) process.stdout.write(`  missing ${m}\n`);
		if (done) failed = true;
	}
}
process.exit(failed ? 1 : 0);
