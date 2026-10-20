// `ipc::mock_not_in_release` (docs/contracts/ipc.md §7.2 rule 9) plus SR-20 hygiene on the built
// SPA: builds the release frontend, then checks that MockIpcClient's marker is absent, that the
// fallback page has exactly one inline script (the SvelteKit bootstrap Tauri hash-pins) and no
// inline style attributes.
import { spawnSync } from 'node:child_process';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const root = join(import.meta.dirname, '..');
const build = spawnSync('pnpm', ['exec', 'vite', 'build', '--logLevel', 'warn'], {
	cwd: root,
	stdio: 'inherit',
	env: { ...process.env, VITE_IPC: '' }
});
if (build.status !== 0) process.exit(build.status ?? 1);

const out = join(root, 'build');
const files = [];
(function walk(d) {
	for (const n of readdirSync(d)) {
		const p = join(d, n);
		if (statSync(p).isDirectory()) walk(p);
		else files.push(p);
	}
})(out);

const problems = [];
for (const f of files.filter((p) => /\.(js|html|css)$/.test(p))) {
	if (readFileSync(f, 'utf8').includes('__FOREMAN_MOCK_IPC__'))
		problems.push(`mock IPC in release: ${f}`);
}
const html = readFileSync(join(out, 'index.html'), 'utf8');
const scripts = html.match(/<script\b[^>]*>/g) ?? [];
const inline = scripts.filter((s) => !/\bsrc=/.test(s));
if (inline.length !== 1)
	problems.push(`index.html has ${inline.length} inline scripts (expected the 1 bootstrap)`);
if (/\sstyle=/.test(html))
	problems.push('index.html has an inline style attribute (style-src is self, SR-20)');

if (problems.length) {
	process.stderr.write(`release-bundle failed:\n  ${problems.join('\n  ')}\n`);
	process.exit(1);
}
process.stdout.write(
	`release-bundle: ${files.length} files, no mock IPC, 1 inline bootstrap script, no inline styles\n`
);
