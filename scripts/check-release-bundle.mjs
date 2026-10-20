// `ipc::mock_not_in_release` (docs/contracts/ipc.md §7.2 rule 9) plus SR-20 hygiene on the built
// SPA: builds the release frontend, then checks that MockIpcClient's marker is absent, that the
// fallback page has exactly one inline script (the SvelteKit bootstrap Tauri hash-pins), one empty
// <style> (the style-nonce carrier) and no remote URL, and that no compiled template has an inline
// style attribute (M3-08, ADR-014, ADR-020).
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
// ADR-014: the only <style> element is app.html's empty nonce carrier (Tauri gives it the style
// nonce that xterm's runtime styles take); content would need a hash Tauri doesn't add.
const styles = [
	...html.replace(/<!--[\s\S]*?-->/g, '').matchAll(/<style\b[^>]*>([\s\S]*?)<\/style>/g)
];
if (styles.length !== 1 || !/id="fm-style-nonce"/.test(styles[0]?.[0] ?? '') || styles[0]?.[1])
	problems.push('index.html must have exactly one <style>: the empty #fm-style-nonce carrier');
// SR-20/SR-60: no remote URL in the page.
if (/(?:src|href)=["']?(?:https?:)?\/\//i.test(html))
	problems.push('index.html loads something from another origin');
// ADR-020: no static style attribute in any compiled template (SvelteKit's route announcer
// included: vite.config.ts drops its attribute).
for (const f of files.filter((p) => p.endsWith('.js'))) {
	const m = /<[a-z][a-z0-9-]*\b[^<>]*\sstyle=["\\]/i.exec(readFileSync(f, 'utf8'));
	if (m) problems.push(`inline style attribute in a template: ${f}: ${m[0].slice(0, 80)}`);
}

if (problems.length) {
	process.stderr.write(`release-bundle failed:\n  ${problems.join('\n  ')}\n`);
	process.exit(1);
}
process.stdout.write(
	`release-bundle: ${files.length} files, no mock IPC, 1 inline bootstrap script, no inline styles, 1 nonce carrier\n`
);
