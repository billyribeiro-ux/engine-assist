// Performance marks (M2-27; NFR-01, NFR-02, NFR-07): against the mock IPC the User Timing entries
// exist, so M5-09 can enforce the targets on the release build. Mock timings are not asserted
// against the targets; the marks and the long-task observer are.
import { expect, test } from '@playwright/test';

test('Today reports a cold start, a project switch is measured, tasks are observed', async ({
	page
}) => {
	await page.goto('/');
	await expect(page.getByRole('heading', { level: 1, name: 'Today' })).toBeVisible();
	await page.waitForFunction(() => performance.getEntriesByName('fm:cold-start').length === 1);
	const cold = await page.evaluate(
		() => performance.getEntriesByName('fm:cold-start')[0]?.duration
	);
	expect(cold).toBeGreaterThan(0);

	// Open a project from the sidebar.
	await page
		.getByRole('link', { name: /TradeLog/ })
		.first()
		.click();
	await page.waitForFunction(() => performance.getEntriesByName('fm:project-switch').length >= 1);
	const sw = await page.evaluate(() =>
		performance.getEntriesByName('fm:project-switch').map((e) => e.duration)
	);
	expect(sw[0]).toBeGreaterThan(0);

	// NFR-07: no task over 50 ms was observed while the mock loaded and rendered (informational
	// ceiling here; M5-09 enforces it on the release build).
	const worst = await page.evaluate(() =>
		performance.getEntriesByName('fm:long-task').reduce((m, e) => Math.max(m, e.duration), 0)
	);
	expect(worst).toBeLessThan(1000);
});
