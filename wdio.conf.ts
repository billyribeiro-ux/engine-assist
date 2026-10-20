// WebdriverIO config for the real-app smoke suite (ADR-015 layer 5, macOS only).
//
// `@wdio/tauri-service` with the embedded driver provider launches the *debug* build made with
// `--features e2e` (`pnpm test:e2e` builds it first, see e2e/run.mjs). That build embeds
// `tauri-plugin-wdio-webdriver`, a W3C WebDriver server on 127.0.0.1 that the service starts on
// TAURI_WEBDRIVER_PORT. No `tauri-driver`, no CrabNebula. The plugin never exists in release builds
// (release::no_webdriver).
//
// Only the WebDriver plugin is used. The service's other half, `tauri-plugin-wdio` plus
// `@wdio/tauri-plugin`, needs `withGlobalTauri: true`, a `wdio:default` capability and a frontend
// import, which SR-21/SR-22/SR-23 forbid. So `browser.tauri.*` (execute, mock, log capture) is not
// available; the specs use plain WebDriver commands (`$`, `browser.execute`, window handles).
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

import type { TauriCapabilities, TauriServiceOptions } from '@wdio/tauri-service';

const root = fileURLToPath(new URL('.', import.meta.url));
const targetDir = process.env.CARGO_TARGET_DIR ?? join(root, 'target');

/** The debug binary of `foreman-app` (bin name in src-tauri/Cargo.toml), built with `--features e2e`. */
const appBinary = process.env.FOREMAN_E2E_APP ?? join(targetDir, 'debug', 'foreman-app');

const tauriService: TauriServiceOptions = {
	driverProvider: 'embedded',
	appBinaryPath: appBinary,
	embeddedPort: 4445,
	// The first launch of a fresh debug binary can be slow (Gatekeeper, WKWebView start-up).
	startTimeout: 90_000,
	captureBackendLogs: true,
	backendLogLevel: 'info'
};

const capability: TauriCapabilities = {
	browserName: 'tauri',
	'tauri:options': { application: appBinary }
};

export const config: WebdriverIO.Config = {
	runner: 'local',
	specs: ['./e2e/specs/**/*.e2e.ts'],
	// One app at a time: the embedded WebDriver server listens on a fixed local port.
	maxInstances: 1,
	capabilities: [capability],
	services: [['@wdio/tauri-service', tauriService]],
	framework: 'mocha',
	mochaOpts: { ui: 'bdd', timeout: 90_000 },
	reporters: ['spec'],
	logLevel: 'warn',
	waitforTimeout: 15_000,
	connectionRetryTimeout: 120_000,
	connectionRetryCount: 1
};
