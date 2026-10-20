// Real-app smoke 3 (SR-29, ADR-014): window.open and navigation to a remote URL are denied.
//
// The target uses the reserved `.invalid` TLD, so even if a denial regressed nothing would reach a
// network. Each attempt is judged by its outcome: no new window, the same document (a marker set
// on `window` survives), the same URL. A positive control shows that navigation inside the app
// origin still works, so a pass cannot come from navigation being broken in general.
//
// Caveat: WebDriver script calls carry no user activation, so WebKit may refuse the window.open and
// link clicks on its own. The unit-level predicates (webview::navigation_denied_to_remote) prove the
// rule; this spec proves the handlers are wired into the built window and nothing regressed.
import assert from 'node:assert/strict';

import { $, browser } from '@wdio/globals';

import { isAppOrigin, readState } from '../helpers/app.ts';
import {
	NAVIGATION_ATTEMPTS,
	RELOAD_SAME_ORIGIN,
	SET_MARKER,
	TODAY_HEADING
} from '../helpers/scripts.ts';

const REMOTE = 'https://example.invalid/';
/** Time for a (wrongly) allowed navigation or new window to show itself. */
const SETTLE_MS = 1500;

describe('remote navigation is denied (SR-29)', () => {
	let startUrl: string;

	before(async () => {
		const heading = $('h1');
		await heading.waitForDisplayed({ timeout: 30_000 });
		assert.ok(isAppOrigin((await readState()).origin));
		startUrl = await browser.execute<string, []>(SET_MARKER);
		assert.equal((await browser.getWindowHandles()).length, 1);
	});

	for (const [name, script] of Object.entries(NAVIGATION_ATTEMPTS)) {
		it(`${name} to a remote URL changes nothing`, async () => {
			const returned = await browser.execute<string, [string]>(script, REMOTE);
			await browser.pause(SETTLE_MS);

			assert.notEqual(returned, 'window', `${name} returned a window object`);
			assert.equal(
				(await browser.getWindowHandles()).length,
				1,
				`${name} opened another window (returned ${returned})`
			);
			const state = await readState();
			assert.equal(state.href, startUrl, `${name} navigated the main webview`);
			assert.equal(state.marker, 'alive', `${name} replaced the document`);
			assert.equal(state.heading, TODAY_HEADING);
		});
	}

	it('control: navigating within the app origin still works', async () => {
		await browser.execute<string, []>(RELOAD_SAME_ORIGIN);
		await browser.waitUntil(
			async () => {
				const state = await readState();
				return state.marker === null && state.heading === TODAY_HEADING;
			},
			{ timeout: 20_000, timeoutMsg: 'same-origin navigation did not reload the app' }
		);
		assert.ok(isAppOrigin((await readState()).origin));
	});
});
