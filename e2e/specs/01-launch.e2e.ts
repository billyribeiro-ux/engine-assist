// Real-app smoke 1 (EX-M2-03, ADR-015 layer 5): the built app starts and reaches the Today route.
import assert from 'node:assert/strict';

import { $, browser } from '@wdio/globals';

import { isAppOrigin, readState } from '../helpers/app.ts';
import { TODAY_HEADING } from '../helpers/scripts.ts';

describe('app launch', () => {
	it('reaches the Today route', async () => {
		const heading = $('h1');
		await heading.waitForDisplayed({ timeout: 30_000, timeoutMsg: 'no <h1> within 30 s' });
		assert.equal((await heading.getText()).trim(), TODAY_HEADING);
	});

	it('runs from the app origin on the real IPC bridge, and the bootstrap call succeeded', async () => {
		const state = await readState();
		assert.ok(isAppOrigin(state.origin), `origin is ${state.origin}`);
		assert.equal(state.tauriInternals, 'object', 'Tauri IPC bridge is missing');
		assert.equal(state.mockIpc, 'undefined', 'the mock IPC client is in the built app');
		assert.equal(state.alert, null, `start-up error banner: ${state.alert ?? ''}`);
	});

	it('opens exactly one window', async () => {
		assert.equal((await browser.getWindowHandles()).length, 1);
	});
});
