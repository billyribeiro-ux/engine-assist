// Real-app smoke 1 (EX-M2-03, ADR-015 layer 5): the built app starts and reaches the Today route.
import assert from 'node:assert/strict';
import { after, before, describe, it } from 'node:test';

import { type RunningApp, startApp } from '../app.ts';
import { isAppOrigin, readState } from '../helpers/app.ts';
import { TODAY_HEADING } from '../helpers/scripts.ts';

describe('app launch', () => {
	let app: RunningApp;
	before(
		async () => {
			app = await startApp();
		},
		{ timeout: 120_000 }
	);
	after(async () => {
		await app.stop();
	});

	it('reaches the Today route', async () => {
		const heading = await app.driver.waitForDisplayed('h1');
		assert.equal((await heading.text()).trim(), TODAY_HEADING);
	});

	it('runs from the app origin on the real IPC bridge, and the bootstrap call succeeded', async () => {
		const state = await readState(app.driver);
		assert.ok(isAppOrigin(state.origin), `origin is ${state.origin}`);
		assert.equal(state.tauriInternals, 'object', 'Tauri IPC bridge is missing');
		assert.equal(state.mockIpc, 'undefined', 'the mock IPC client is in the built app');
		assert.equal(state.alert, null, `start-up error banner: ${state.alert ?? ''}`);
	});

	it('opens exactly one window', async () => {
		assert.equal((await app.driver.getWindowHandles()).length, 1);
	});
});
