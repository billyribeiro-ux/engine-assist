// SR-60 / ADR-016 decision 5 (M0-11): the frontend makes no request outside its own origin. The P1
// flow extends this in M2; the Tauri build is additionally restricted by CSP connect-src.
import { expect, test } from '@playwright/test';

test('no request leaves the app origin', async ({ page, baseURL }) => {
	const origin = new URL(baseURL ?? 'http://localhost:4173').origin;
	const external: string[] = [];
	page.on('request', (r) => {
		if (new URL(r.url()).origin !== origin) external.push(r.url());
	});
	await page.goto('/');
	await expect(page.getByRole('heading', { level: 1, name: 'Today' })).toBeVisible();
	await page.waitForLoadState('networkidle');
	expect(external).toEqual([]);
});
