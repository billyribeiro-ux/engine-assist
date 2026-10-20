// Unit tests for the real-app smoke's pure helpers (e2e/helpers/csp.ts, e2e/helpers/scripts.ts).
// The smoke itself runs on macOS only (`pnpm test:e2e`); this keeps its comparison logic proven on
// every OS, against the real tauri.conf.json.
import { readFileSync } from 'node:fs';
import { Script } from 'node:vm';

import { describe, expect, it } from 'vitest';

import { compareServedCsp, configuredCsp, parseCsp } from '../e2e/helpers/csp.ts';
import {
	INJECT_INLINE_SCRIPT,
	NAVIGATION_ATTEMPTS,
	READ_STATE,
	RELOAD_SAME_ORIGIN,
	SET_MARKER
} from '../e2e/helpers/scripts.ts';

const conf: unknown = JSON.parse(
	readFileSync(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8')
);
const production = configuredCsp(conf, 'csp');

const HASH = "'sha256-q0Sd1d9Vr2U6hJc1R7mTz1q2Qm0o8nXn1s5oS3xv0aA='";
const NONCE = "'nonce-Zm9yZW1hbg=='";

/** What Tauri serves: the configured policy plus its generated script hash and style nonce. */
function served(mutate?: (d: Map<string, string[]>) => void): string {
	const d = new Map([...production].map(([k, v]) => [k, [...v]]));
	d.get('script-src')?.push(HASH);
	d.get('style-src')?.push(NONCE);
	mutate?.(d);
	return [...d].map(([k, v]) => [k, ...v].join(' ')).join('; ');
}

describe('e2e CSP helpers (SR-20)', () => {
	it('reads the production policy from tauri.conf.json', () => {
		expect(production.get('default-src')).toEqual(["'self'"]);
		expect(production.get('script-src')).toEqual(["'self'"]);
		expect(production.get('connect-src')).toEqual(['ipc:', 'http://ipc.localhost']);
		expect(production.get('frame-ancestors')).toEqual(["'none'"]);
	});

	it('accepts the production policy with Tauri-generated hashes and nonces', () => {
		expect(compareServedCsp(served(), production)).toEqual([]);
	});

	it('is independent of directive order and spacing', () => {
		const reversed = served()
			.split('; ')
			.reverse()
			.map((p) => `  ${p}  `)
			.join(' ;');
		expect(compareServedCsp(reversed, production)).toEqual([]);
	});

	it('rejects the dev policy being served', () => {
		const dev = configuredCsp(conf, 'devCsp');
		const problems = compareServedCsp(
			[...dev].map(([k, v]) => [k, ...v, ...(k === 'script-src' ? [HASH] : [])].join(' ')).join('; '),
			production
		);
		expect(problems.join('\n')).toContain('connect-src');
	});

	it.each([
		['unsafe-inline in script-src', (d: Map<string, string[]>) => d.get('script-src')?.push("'unsafe-inline'")],
		['unsafe-eval in script-src', (d: Map<string, string[]>) => d.get('script-src')?.push("'unsafe-eval'")],
		['a remote host in script-src', (d: Map<string, string[]>) => d.get('script-src')?.push('https://cdn.example')],
		['an extra directive', (d: Map<string, string[]>) => d.set('report-uri', ['https://r.example'])],
		['a dropped directive', (d: Map<string, string[]>) => d.delete('frame-ancestors')],
		['a hash in connect-src', (d: Map<string, string[]>) => d.get('connect-src')?.push(HASH)],
		['a relaxed img-src', (d: Map<string, string[]>) => d.get('img-src')?.push('https:')]
	])('rejects %s', (_name, mutate) => {
		expect(compareServedCsp(served(mutate), production)).not.toEqual([]);
	});

	it('requires a generated hash for the inline bootstrap script', () => {
		const noHash = served((d) => d.set('script-src', ["'self'"]));
		expect(compareServedCsp(noHash, production)).toContain(
			'script-src has no Tauri-generated hash for the inline bootstrap script'
		);
	});

	it('rejects several policies joined by a comma', () => {
		expect(compareServedCsp(`${served()}, default-src 'none'`, production).join('\n')).toContain(
			'comma-joined'
		);
	});

	it('parses the first occurrence of a repeated directive, as browsers do', () => {
		expect(parseCsp("script-src 'self'; script-src 'unsafe-inline'").get('script-src')).toEqual([
			"'self'"
		]);
	});
});

describe('e2e page scripts', () => {
	it('are valid async-function bodies (the page runs them; nothing compiles them)', () => {
		const bodies: Record<string, string> = {
			...NAVIGATION_ATTEMPTS,
			INJECT_INLINE_SCRIPT,
			READ_STATE,
			RELOAD_SAME_ORIGIN,
			SET_MARKER
		};
		for (const [name, body] of Object.entries(bodies)) {
			// Compiles only (never runs), the way the WebDriver server wraps a script body.
			expect(() => new Script(`(async function () { ${body} })`), name).not.toThrow();
		}
	});
});
