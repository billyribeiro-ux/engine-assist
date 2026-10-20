// `pnpm verify` (AC-M0-03, M0-07): every local gate, in order; fails on any problem.
// `pnpm verify:fast` runs only the type checks and lint.
//
//   node scripts/verify.mjs [--fast] [--bail] [--only=name,name]
//
// Each step is an argv array run without a shell. A step whose tool is missing FAILS with an
// install hint (never silently skipped). Steps run with the Rust toolchain from rust-toolchain.toml
// unless RUSTUP_TOOLCHAIN is set in the environment.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = join(import.meta.dirname, '..');
const args = new Set(process.argv.slice(2));
const fast = args.has('--fast');
const bail = args.has('--bail');
const only = [...args]
	.find((a) => a.startsWith('--only='))
	?.slice(7)
	.split(',');

const bindingsDir = join(root, 'src', 'lib', 'ipc', 'bindings');

/** @param {string} dir */
function hashDir(dir) {
	const h = createHash('sha256');
	for (const f of readdirSync(dir).sort())
		h.update(f)
			.update('\0')
			.update(readFileSync(join(dir, f)));
	return h.digest('hex');
}

/**
 * @param {string[]} cmd
 * @param {string[]} probe
 */
function have(cmd, probe = ['--version']) {
	const [bin = '', ...rest] = cmd;
	const r = spawnSync(bin, [...rest, ...probe], { cwd: root, stdio: 'ignore' });
	return r.status === 0;
}

/** @type {{ name: string, fast?: boolean, run: string[] | (() => number), needs?: { cmd: string[], hint: string } }[]} */
const steps = [
	{ name: 'pnpm-policy', run: ['node', 'scripts/check-lockfile-sources.mjs'] },
	{ name: 'svelte-check', fast: true, run: ['pnpm', 'run', '--silent', 'check'] },
	{ name: 'eslint', fast: true, run: ['pnpm', 'exec', 'eslint', '--max-warnings', '0', '.'] },
	{ name: 'prettier', run: ['pnpm', 'exec', 'prettier', '--check', '--log-level', 'warn', '.'] },
	{ name: 'tsc', fast: true, run: ['pnpm', 'exec', 'tsc', '--noEmit', '-p', 'tsconfig.json'] },
	{ name: 'vitest', run: ['pnpm', 'exec', 'vitest', 'run'] },
	{ name: 'release-bundle', run: ['node', 'scripts/check-release-bundle.mjs'] },
	{ name: 'rustfmt', run: ['cargo', 'fmt', '--all', '--', '--check'] },
	{
		name: 'clippy',
		run: ['cargo', 'clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings']
	},
	{
		name: 'clippy-ts',
		run: [
			'cargo',
			'clippy',
			'-p',
			'foreman-app',
			'--all-targets',
			'--features',
			'ts',
			'--locked',
			'--',
			'-D',
			'warnings'
		]
	},
	{ name: 'cargo-test', run: ['cargo', 'test', '--workspace', '--locked'] },
	{
		name: 'bindings-drift',
		run: () => {
			const before = hashDir(bindingsDir);
			const r = spawnSync('cargo', ['test', '-p', 'foreman-app', '--features', 'ts', '--locked'], {
				cwd: root,
				stdio: 'inherit'
			});
			if (r.status !== 0) return r.status ?? 1;
			if (hashDir(bindingsDir) !== before) {
				process.stderr.write(
					'src/lib/ipc/bindings changed when regenerated: commit the regenerated bindings (ADR-004).\n'
				);
				return 1;
			}
			return 0;
		}
	},
	{ name: 'core-deps', run: ['bash', 'scripts/check-core-deps.sh'] },
	{
		name: 'cargo-deny',
		run: ['cargo', 'deny', '--workspace', 'check'],
		needs: { cmd: ['cargo', 'deny'], hint: 'cargo install --locked cargo-deny --version 0.20.2' }
	},
	{
		name: 'cargo-audit',
		run: ['cargo', 'audit', '--deny', 'warnings'],
		needs: { cmd: ['cargo', 'audit'], hint: 'cargo install --locked cargo-audit --version 0.22.2' }
	},
	{ name: 'pnpm-audit', run: ['pnpm', 'audit', '--audit-level', 'low'] },
	{
		name: 'secret-scan',
		run: [
			'gitleaks',
			'dir',
			'--config',
			'.gitleaks.toml',
			'--redact',
			'--no-banner',
			'--exit-code',
			'1',
			'.'
		],
		needs: {
			cmd: ['gitleaks'],
			hint: 'install gitleaks 8.30.1 (brew install gitleaks, or the checksum-verified release; see docs/dev-notes.md)'
		}
	},
	{ name: 'license-check', run: ['node', 'scripts/license-check.mjs'] }
];

const selected = steps.filter((s) => (only ? only.includes(s.name) : fast ? s.fast : true));
const results = [];
for (const step of selected) {
	const t0 = Date.now();
	process.stdout.write(`\n▶ ${step.name}\n`);
	let status;
	if (step.needs && !have(step.needs.cmd, ['--version'])) {
		process.stderr.write(
			`✖ ${step.name}: tool not installed: ${step.needs.cmd.join(' ')}\n  install: ${step.needs.hint}\n`
		);
		status = 127;
	} else if (typeof step.run === 'function') {
		status = step.run();
	} else {
		const [cmd = '', ...rest] = step.run;
		status = spawnSync(cmd, rest, { cwd: root, stdio: 'inherit' }).status ?? 1;
	}
	results.push({ name: step.name, ok: status === 0, secs: ((Date.now() - t0) / 1000).toFixed(1) });
	if (status !== 0 && bail) break;
}

process.stdout.write('\nverify summary\n');
for (const r of results)
	process.stdout.write(`  ${r.ok ? 'PASS' : 'FAIL'}  ${r.name.padEnd(15)} ${r.secs}s\n`);
const failed = results.filter((r) => !r.ok);
if (failed.length) {
	process.stderr.write(
		`\n${failed.length} step(s) failed: ${failed.map((r) => r.name).join(', ')}\n`
	);
	process.exit(1);
}
process.stdout.write(`\nAll ${results.length} steps passed.\n`);
