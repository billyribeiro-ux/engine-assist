// Shared helpers for the smoke specs: reading page state and the production CSP.
import { readFileSync } from 'node:fs';

import type { WebDriver } from '../webdriver.ts';
import { configuredCsp, type CspDirectives } from './csp.ts';
import { READ_STATE } from './scripts.ts';

/** What `READ_STATE` returns. */
export interface PageState {
	href: string;
	origin: string;
	heading: string | null;
	alert: string | null;
	marker: string | null;
	tauriInternals: string;
	mockIpc: string;
}

export function readState(driver: WebDriver): Promise<PageState> {
	return driver.execute<PageState>(READ_STATE);
}

/** The production policy: `app.security.csp` from the repo's tauri.conf.json (not `devCsp`). */
export function productionCsp(): CspDirectives {
	const raw = readFileSync(new URL('../../src-tauri/tauri.conf.json', import.meta.url), 'utf8');
	return configuredCsp(JSON.parse(raw) as unknown, 'csp');
}

/** The app origin per SR-29: `tauri://localhost` on macOS (`http://tauri.localhost` elsewhere). */
export function isAppOrigin(origin: string): boolean {
	return origin === 'tauri://localhost' || origin === 'http://tauri.localhost';
}
