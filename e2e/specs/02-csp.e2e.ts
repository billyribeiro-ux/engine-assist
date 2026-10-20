// Real-app smoke 2 (SR-20, ADR-014, ADR-015): the policy the app serves is the production policy
// from tauri.conf.json, and an injected inline <script> raises securitypolicyviolation and does
// not run.
//
// The page's own `connect-src` is `ipc: http://ipc.localhost` (no 'self'), so the page cannot fetch
// its own response headers. A violation event carries `originalPolicy`, the policy as served, so the
// header is read from the event for the injected script.
import assert from 'node:assert/strict';

import { $, browser } from '@wdio/globals';

import { productionCsp } from '../helpers/app.ts';
import { compareServedCsp } from '../helpers/csp.ts';
import { INJECT_INLINE_SCRIPT, TODAY_HEADING } from '../helpers/scripts.ts';

interface Violation {
	effectiveDirective: string;
	violatedDirective: string;
	blockedURI: string;
	disposition: string;
	originalPolicy: string;
}

interface InjectionResult {
	violations: Violation[];
	executed: boolean;
}

describe('content security policy (SR-20)', () => {
	let result: InjectionResult;

	before(async () => {
		// The page must have booted first: the hashed SvelteKit bootstrap script only runs if the
		// served policy pins the right hash (a missing hash leaves a blank page in release builds).
		const heading = $('h1');
		await heading.waitForDisplayed({ timeout: 30_000 });
		assert.equal((await heading.getText()).trim(), TODAY_HEADING);

		result = await browser.execute<InjectionResult, []>(INJECT_INLINE_SCRIPT);
	});

	it('does not run an injected inline <script>', () => {
		assert.equal(result.executed, false, 'the injected inline script executed');
	});

	it('raises an enforced securitypolicyviolation for it', () => {
		const violation = result.violations.find((v) => v.effectiveDirective.startsWith('script-src'));
		assert.ok(
			violation,
			`no script-src violation event: ${JSON.stringify(result.violations, null, 1)}`
		);
		assert.equal(violation.blockedURI, 'inline');
		assert.equal(violation.disposition, 'enforce');
	});

	it('serves exactly the production policy (plus Tauri hashes and nonces)', () => {
		const violation = result.violations.find((v) => v.effectiveDirective.startsWith('script-src'));
		assert.ok(violation, 'no violation event to read the served policy from');
		const problems = compareServedCsp(violation.originalPolicy, productionCsp());
		assert.deepEqual(problems, [], `served: ${violation.originalPolicy}`);
	});
});
