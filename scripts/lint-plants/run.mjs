// Planted-violation proof for the security lint rules (M0-03; AC-XC-01, AC-XC-30, AC-F030-04).
// Each plant is copied into a real source location, linted, and removed again (always, in
// `finally`). The plants live in ./fixtures, which every build and lint config excludes.
//
//   node scripts/lint-plants/run.mjs            # ESLint and clippy plants
//   node scripts/lint-plants/run.mjs --eslint   # ESLint only (also run by Vitest)
//   node scripts/lint-plants/run.mjs --clippy   # clippy only (needs the Rust toolchain)
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
export const root = join(here, '..', '..');
/**
 * @typedef {{ fixture: string, target: string, rules: string[] }} EslintPlant
 * @typedef {{ fixture: string, crate: string, target: string, modFile: string, mod: string }} ClippyPlant
 * @typedef {{ target: string, ok: boolean, line: string }} PlantResult
 */
/** @type {{ eslint: EslintPlant[], clippy: ClippyPlant[] }} */
const plants = JSON.parse(readFileSync(join(here, 'plants.json'), 'utf8'));

/**
 * @param {string} fixture
 * @param {string} target
 * @returns {() => void} cleanup
 */
function place(fixture, target) {
	const abs = join(root, target);
	if (existsSync(abs)) throw new Error(`refusing to overwrite ${target}`);
	/** @type {string[]} */
	const created = [];
	let dir = dirname(abs);
	while (!existsSync(dir)) {
		created.unshift(dir);
		dir = dirname(dir);
	}
	for (const d of created) mkdirSync(d);
	copyFileSync(join(here, 'fixtures', fixture), abs);
	return () => {
		rmSync(abs, { force: true });
		for (const d of created.reverse()) rmSync(d, { recursive: true, force: true });
	};
}

/**
 * Lints every ESLint plant in place.
 * @returns {Promise<(PlantResult & { expected: string[], got: string[] })[]>}
 */
export async function runEslintPlants() {
	const { ESLint } = await import('eslint');
	const results = [];
	for (const p of plants.eslint) {
		const cleanup = place(p.fixture, p.target);
		try {
			const eslint = new ESLint({ cwd: root });
			const [res] = await eslint.lintFiles([join(root, p.target)]);
			const got = (res?.messages ?? []).map((m) => m.ruleId ?? `fatal: ${m.message}`);
			const ok = p.rules.every(
				(r) => got.filter((g) => g === r).length >= p.rules.filter((x) => x === r).length
			);
			results.push({ target: p.target, expected: p.rules, got, ok, line: got.join(', ') });
		} finally {
			cleanup();
		}
	}
	return results;
}

/**
 * Runs clippy with each Rust plant in place.
 * @returns {PlantResult[]}
 */
export function runClippyPlants() {
	const results = [];
	for (const p of plants.clippy) {
		const modPath = join(root, p.modFile);
		const original = readFileSync(modPath, 'utf8');
		const cleanup = place(p.fixture, p.target);
		try {
			writeFileSync(modPath, `${original}\npub mod ${p.mod};\n`);
			const r = spawnSync(
				'cargo',
				['clippy', '-p', p.crate, '--all-targets', '--', '-D', 'warnings'],
				{ cwd: root, encoding: 'utf8', env: process.env }
			);
			const out = `${r.stdout}${r.stderr}`;
			const ok = r.status !== 0 && /disallowed[_ ]method/.test(out);
			results.push({
				target: p.target,
				ok,
				line: ok ? 'clippy::disallowed_methods' : out.slice(-2000)
			});
		} finally {
			writeFileSync(modPath, original);
			cleanup();
		}
	}
	return results;
}

async function main() {
	const args = new Set(process.argv.slice(2));
	const all = !args.has('--eslint') && !args.has('--clippy');
	/** @type {PlantResult[]} */
	const results = [];
	if (all || args.has('--eslint')) results.push(...(await runEslintPlants()));
	if (all || args.has('--clippy')) results.push(...runClippyPlants());
	let failed = 0;
	for (const r of results) {
		process.stdout.write(
			`${r.ok ? 'CAUGHT ' : 'MISSED '} ${relative(root, join(root, r.target))}: ${r.line}\n`
		);
		if (!r.ok) failed++;
	}
	if (failed) {
		process.stderr.write(`${failed} plant(s) were not caught by lint.\n`);
		process.exit(1);
	}
	process.stdout.write(`All ${results.length} plants were caught.\n`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
	await main();
}
