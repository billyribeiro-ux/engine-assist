import { defineConfig, devices } from '@playwright/test';

// ADR-015 layer 4: flows against `vite preview` with VITE_IPC=mock. WebKit is the product proxy
// (WKWebView); Chromium runs where WebKit isn't installed (PLAYWRIGHT_CHROMIUM_PATH points at a
// preinstalled build). Select with `--project`.
const chromiumPath = process.env.PLAYWRIGHT_CHROMIUM_PATH;

export default defineConfig({
	testDir: 'tests/flows',
	testMatch: '**/*.spec.ts',
	forbidOnly: !!process.env.CI,
	retries: 0,
	reporter: 'list',
	use: { baseURL: 'http://localhost:4173' },
	webServer: {
		command: 'pnpm run preview:mock',
		url: 'http://localhost:4173',
		reuseExistingServer: !process.env.CI,
		timeout: 120_000
	},
	projects: [
		{ name: 'webkit', use: { ...devices['Desktop Safari'] } },
		{
			name: 'chromium',
			use: {
				...devices['Desktop Chrome'],
				...(chromiumPath ? { launchOptions: { executablePath: chromiumPath } } : {})
			}
		}
	]
});
