// CSP comparison helpers for the real-app smoke (SR-20, ADR-014, ADR-015 layer 5). Pure functions
// with no WebdriverIO imports, so Vitest covers them on any OS (tests/e2e-helpers.test.ts).

/** A parsed policy: directive name to its tokens (in source order, duplicates removed). */
export type CspDirectives = ReadonlyMap<string, readonly string[]>;

/** Tokens Tauri adds at build/serve time: script/style hashes and style nonces (ADR-014). */
const GENERATED_SOURCE = /^'(?:(?:sha256|sha384|sha512)-[A-Za-z0-9+/_=-]+|nonce-[^'\s]+)'$/;
const GENERATED_HASH = /^'(?:sha256|sha384|sha512)-[A-Za-z0-9+/_=-]+'$/;
/** Only these directives may carry Tauri-generated hashes or nonces. */
const MAY_CARRY_GENERATED = new Set(['script-src', 'style-src']);

/** Parses a `Content-Security-Policy` value. Repeated directives keep the first (as browsers do). */
export function parseCsp(policy: string): CspDirectives {
	const out = new Map<string, readonly string[]>();
	for (const part of policy.split(';')) {
		const tokens = part.trim().split(/\s+/).filter(Boolean);
		const [name, ...sources] = tokens;
		if (name === undefined) continue;
		const key = name.toLowerCase();
		if (!out.has(key)) out.set(key, [...new Set(sources)]);
	}
	return out;
}

/** Reads `app.security.csp` (or `devCsp`) from a parsed tauri.conf.json (object or string form). */
export function configuredCsp(conf: unknown, field: 'csp' | 'devCsp' = 'csp'): CspDirectives {
	const security = (conf as { app?: { security?: Record<string, unknown> } }).app?.security;
	const value = security?.[field];
	if (typeof value === 'string') return parseCsp(value);
	if (value === null || typeof value !== 'object') {
		throw new Error(`tauri.conf.json: app.security.${field} is missing`);
	}
	const out = new Map<string, readonly string[]>();
	for (const [name, sources] of Object.entries(value as Record<string, unknown>)) {
		const list =
			typeof sources === 'string'
				? sources.split(/\s+/).filter(Boolean)
				: Array.isArray(sources)
					? sources.map(String)
					: [];
		out.set(name.toLowerCase(), [...new Set(list)]);
	}
	return out;
}

const sameSet = (a: readonly string[], b: readonly string[]): boolean =>
	a.length === b.length && a.every((token) => b.includes(token));

/**
 * Compares the policy the app actually served (a `securitypolicyviolation` event's
 * `originalPolicy`) with the policy configured in tauri.conf.json. Returns the problems found; an
 * empty array means they match. Allowed differences: Tauri's generated hashes and nonces in
 * `script-src` and `style-src`. Also enforces SR-20's invariants on the served policy itself.
 */
export function compareServedCsp(servedPolicy: string, configured: CspDirectives): string[] {
	const problems: string[] = [];
	if (servedPolicy.includes(',')) {
		problems.push('served policy is a comma-joined list of several policies');
	}
	const served = parseCsp(servedPolicy);

	for (const name of configured.keys()) {
		if (!served.has(name)) problems.push(`missing directive: ${name}`);
	}
	for (const name of served.keys()) {
		if (!configured.has(name)) problems.push(`unexpected directive: ${name}`);
	}

	for (const [name, want] of configured) {
		const got = served.get(name);
		if (got === undefined) continue;
		const generated = got.filter((t) => GENERATED_SOURCE.test(t));
		if (generated.length > 0 && !MAY_CARRY_GENERATED.has(name)) {
			problems.push(`${name}: unexpected generated source(s) ${generated.join(' ')}`);
		}
		const rest = got.filter((t) => !GENERATED_SOURCE.test(t));
		if (!sameSet(rest, want)) {
			problems.push(`${name}: served [${rest.join(' ')}] but configured [${want.join(' ')}]`);
		}
	}

	const scriptSrc = served.get('script-src') ?? [];
	for (const bad of ["'unsafe-inline'", "'unsafe-eval'", "'unsafe-hashes'", "'strict-dynamic'"]) {
		if (scriptSrc.includes(bad)) problems.push(`script-src contains ${bad}`);
	}
	for (const [name, tokens] of served) {
		if (tokens.includes("'unsafe-eval'")) problems.push(`${name} contains 'unsafe-eval'`);
	}
	// SvelteKit's single inline bootstrap script must be hash-pinned by Tauri (ADR-002, ADR-014).
	if (!scriptSrc.some((t) => GENERATED_HASH.test(t))) {
		problems.push('script-src has no Tauri-generated hash for the inline bootstrap script');
	}
	return problems;
}
