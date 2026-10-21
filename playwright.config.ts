import { defineConfig, devices } from '@playwright/test';

// ADR-015 layer 4: flows against `vite preview` with VITE_IPC=mock. WebKit is the product proxy
// (WKWebView); Chromium runs where WebKit isn't installed (PLAYWRIGHT_CHROMIUM_PATH points at a
// preinstalled build). Select with `--project`.
const chromiumPath = process.env.PLAYWRIGHT_CHROMIUM_PATH;
// Parallel worktrees each run their own preview server: FM_FLOWS_PORT picks the port (default 4173).
const port = Number(process.env.FM_FLOWS_PORT ?? 4173);
const origin = `http://localhost:${port}`;

export default defineConfig({
	testDir: 'tests/flows',
	testMatch: '**/*.spec.ts',
	forbidOnly: !!process.env.CI,
	retries: 0,
	reporter: 'list',
	use: { baseURL: origin },
	webServer: {
		command:
			port === 4173
				? 'pnpm run preview:mock'
				: `pnpm exec vite build --mode mock && pnpm exec vite preview --mode mock --port ${port} --strictPort`,
		url: origin,
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
