// check-lockfile-sources.mjs (CI `pnpm-policy`): the file:/link: source scan must flag real
// file:/link: sources and must not flag package names that merely end in "file" or "link"
// (`get-caller-file: 2.0.5` is a version line in the real lockfile).
import { spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterAll, describe, expect, it } from 'vitest';

const script = join(import.meta.dirname, '..', 'scripts', 'check-lockfile-sources.mjs');
const dir = mkdtempSync(join(tmpdir(), 'fm-lock-'));
afterAll(() => {
	rmSync(dir, { recursive: true, force: true });
});

function scan(body: string): { status: number | null; stderr: string } {
	const lock = join(dir, 'pnpm-lock.yaml');
	writeFileSync(
		lock,
		`lockfileVersion: '9.0'\npackages:\n  typescript@6.0.3:\n    resolution: {}\n${body}`
	);
	const result = spawnSync(process.execPath, [script, lock], { encoding: 'utf8' });
	return { status: result.status, stderr: result.stderr };
}

describe('pnpm-policy lockfile scan', () => {
	it('accepts package names that end in "file" or "link"', () => {
		const out = scan(
			'  get-caller-file@2.0.5:\n    resolution: {}\nsnapshots:\n  get-caller-file: 2.0.5\n  some-link: 1.0.0\n'
		);
		expect(out.stderr).toBe('');
		expect(out.status).toBe(0);
	});

	it.each(['file:../evil', 'link:../evil', 'foo: file:./x.tgz'])('rejects the source %s', (src) => {
		const out = scan(`  x@1.0.0:\n    resolution: {dir: ${src}}\n    ${src}\n`);
		expect(out.status).toBe(1);
		expect(out.stderr).toContain('file:/link: source');
	});
});
