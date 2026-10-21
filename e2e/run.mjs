// `pnpm test:e2e`: the real-app smoke suite (ADR-015 layer 5).
//
// macOS only: the embedded WebDriver server drives WKWebView, and the product is macOS-only. On
// any other OS this prints a skip notice and exits 0, so `pnpm test:e2e` never fails on Linux.
//
// On macOS it (1) builds the debug app with the `e2e` feature and the production frontend, unless
// FOREMAN_E2E_SKIP_BUILD=1, then (2) runs the specs with `node --test` (e2e/specs/*.e2e.ts, one app
// per spec file). `tauri build --debug --no-bundle` is used rather than `cargo build` because the
// CLI adds `tauri/custom-protocol`: without it the debug binary loads the Vite dev server and
// `devCsp` instead of the embedded assets and the production CSP.
import { spawnSync } from 'node:child_process';
import { existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const root = join(import.meta.dirname, '..');

if (process.platform !== 'darwin') {
	process.stdout.write(
		`test:e2e: SKIPPED (macOS only). The real-app smoke drives WKWebView through the embedded ` +
			`WebDriver server; this machine is ${process.platform}. Run it on the Mac: pnpm test:e2e ` +
			`(docs/dev-notes.md, "Real-app smoke").\n`
	);
	process.exit(0);
}

/** @param {string[]} argv */
function run(argv) {
	const [cmd, ...args] = argv;
	if (cmd === undefined) throw new Error('empty command');
	const result = spawnSync(cmd, args, { cwd: root, stdio: 'inherit' });
	if (result.error) {
		process.stderr.write(`test:e2e: could not run ${cmd}: ${result.error.message}\n`);
		return 1;
	}
	return result.status ?? 1;
}

if (process.env.FOREMAN_E2E_SKIP_BUILD !== '1') {
	process.stdout.write('test:e2e: building the debug app with --features e2e\n');
	const built = run([
		'pnpm',
		'exec',
		'tauri',
		'build',
		'--debug',
		'--no-bundle',
		'--features',
		'e2e'
	]);
	if (built !== 0) process.exit(built);
}

const app =
	process.env.FOREMAN_E2E_APP ??
	join(process.env.CARGO_TARGET_DIR ?? join(root, 'target'), 'debug', 'foreman-app');
if (!existsSync(app)) {
	process.stderr.write(`test:e2e: app binary not found at ${app}\n`);
	process.exit(1);
}

const specs = readdirSync(join(root, 'e2e', 'specs'))
	.filter((f) => f.endsWith('.e2e.ts'))
	.sort()
	.map((f) => join('e2e', 'specs', f));
// Node 22.17 needs the flag to run .ts files; later 22.x accepts it too.
process.exit(
	run([
		process.execPath,
		'--experimental-strip-types',
		'--disable-warning=ExperimentalWarning',
		'--test',
		'--test-concurrency=1',
		...specs
	])
);
