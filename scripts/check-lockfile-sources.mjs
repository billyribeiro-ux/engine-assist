// CI `pnpm-policy` (ADR-024, SR-61, AC-M0-17): exact versions in package.json, no exotic sources
// in pnpm-lock.yaml (git, tarball URLs off the registry, file:/link: outside the workspace), and
// exactly one TypeScript version (ADR-019).
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = join(import.meta.dirname, '..');
const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
// Optional argument: a lockfile path (used by scripts/plants/run.sh to prove the scan fails).
const lockPath = process.argv[2] ?? join(root, 'pnpm-lock.yaml');
const lock = readFileSync(lockPath, 'utf8');
const problems = [];

const EXACT = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/;
for (const field of [
	'dependencies',
	'devDependencies',
	'optionalDependencies',
	'peerDependencies'
]) {
	for (const [name, spec] of Object.entries(pkg[field] ?? {})) {
		if (!EXACT.test(spec)) problems.push(`${field}.${name}: "${spec}" is not an exact version`);
	}
}
if (pkg.packageManager !== 'pnpm@12.14.0') problems.push(`packageManager is ${pkg.packageManager}`);

const lines = lock.split('\n');
lines.forEach((line, i) => {
	const at = `${lockPath}:${i + 1}`;
	if (/\btarball:\s*(?!https:\/\/registry\.npmjs\.org\/)/.test(line))
		problems.push(`${at}: tarball outside the npm registry: ${line.trim()}`);
	if (
		/(git\+|git:\/\/|codeload\.github\.com|github:|gitlab:|bitbucket:|\btype:\s*git\b)/.test(line)
	)
		problems.push(`${at}: git source: ${line.trim()}`);
	if (/(?<![\w-])(file|link):(?!\.\/?$)/.test(line) && !/version: link:\./.test(line))
		problems.push(`${at}: file:/link: source: ${line.trim()}`);
});

const tsVersions = new Set(
	[...lock.matchAll(/^ {2}typescript@(\d+\.\d+\.\d+[^:\s]*):/gm)].map((m) => m[1])
);
if (tsVersions.size !== 1 || !tsVersions.has('6.0.3'))
	problems.push(
		`TypeScript versions in the lockfile: ${[...tsVersions].join(', ') || 'none'} (want exactly 6.0.3)`
	);

if (problems.length) {
	process.stderr.write(`pnpm-policy failed:\n  ${problems.join('\n  ')}\n`);
	process.exit(1);
}
process.stdout.write(
	'pnpm-policy: exact versions, registry-only sources, one TypeScript (6.0.3)\n'
);
