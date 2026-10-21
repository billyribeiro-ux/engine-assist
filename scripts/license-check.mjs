// License check for the JS tree (AC-M0-03). Rust licenses are checked by `cargo deny check licenses`
// against deny.toml. Allowed: permissive licenses only, mirroring deny.toml, plus BlueOak-1.0.0.
import { execFileSync } from 'node:child_process';

const ALLOWED = new Set([
	'MIT',
	'Apache-2.0',
	'BSD-2-Clause',
	'BSD-3-Clause',
	'ISC',
	'Unicode-3.0',
	'Zlib',
	'MPL-2.0',
	'CC0-1.0',
	'0BSD',
	'BlueOak-1.0.0'
]);

/**
 * @param {string} expr SPDX expression
 * @returns {boolean}
 */
function ok(expr) {
	const e = expr.replace(/[()]/g, '').trim();
	if (e.includes(' OR ')) return e.split(' OR ').some(ok);
	if (e.includes(' AND ')) return e.split(' AND ').every(ok);
	return ALLOWED.has(e);
}

const out = execFileSync('pnpm', ['licenses', 'list', '--json'], { encoding: 'utf8' });
/** @type {Record<string, { name: string, versions: string[] }[]>} */
const byLicense = JSON.parse(out);
const bad = [];
for (const [license, pkgs] of Object.entries(byLicense)) {
	if (!ok(license)) for (const p of pkgs) bad.push(`${p.name}@${p.versions.join(',')}: ${license}`);
}
if (bad.length) {
	process.stderr.write(`license-check: disallowed licenses:\n  ${bad.join('\n  ')}\n`);
	process.exit(1);
}
const n = Object.values(byLicense).reduce((a, p) => a + p.length, 0);
process.stdout.write(`license-check: ${n} JS packages, all with allowed licenses\n`);
