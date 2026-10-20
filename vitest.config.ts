/// <reference types="node" />
import { playwright } from '@vitest/browser-playwright';
import { defineConfig, mergeConfig } from 'vitest/config';

import viteConfig from './vite.config.ts';

// ADR-015 layer 3. `unit` runs in Node; `browser` runs component tests in a real browser against
// MockIpcClient (Chromium here; WebKit is added on the Mac). PLAYWRIGHT_CHROMIUM_PATH points at a
// preinstalled Chromium when Playwright's own browser download isn't available.
const chromiumPath = process.env.PLAYWRIGHT_CHROMIUM_PATH;

export default mergeConfig(
	viteConfig,
	defineConfig({
		test: {
			projects: [
				{
					extends: true,
					test: {
						name: 'unit',
						environment: 'node',
						include: ['src/**/*.test.ts', 'tests/**/*.test.ts'],
						exclude: ['src/**/*.svelte.test.ts']
					}
				},
				{
					extends: true,
					define: { 'import.meta.env.VITE_IPC': JSON.stringify('mock') },
					test: {
						name: 'browser',
						include: ['src/**/*.svelte.test.ts'],
						setupFiles: ['src/lib/test/browser-setup.ts'],
						browser: {
							enabled: true,
							headless: true,
							provider: playwright(
								chromiumPath ? { launchOptions: { executablePath: chromiumPath } } : {}
							),
							instances: [{ browser: 'chromium' }]
						}
					}
				}
			]
		}
	})
);
