// Code review C1: a signed packet whose working file differs from its commit made project_open
// emit tamper:detected on every open, the bus mapped it to a project refetch, and the refetch
// opened again: an endless loop (3 git spawns and an inbox sync per pass in the real app). The mock
// emits the event the way Rust does; this flow counts the project_open calls.
import { expect, test, type Page } from '@playwright/test';

import { TL } from './screens';

function opens(page: Page): Promise<number> {
	return page.evaluate(() => {
		const m = (window as unknown as { __foremanMock?: { calls: Map<string, number> } })
			.__foremanMock;
		return m?.calls.get('project_open') ?? 0;
	});
}

test('tamper::no_refetch_loop', async ({ page }) => {
	await page.goto(`/projects/${TL}/stages/2?scenario=tamper`);
	await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
	// The first open and the one refetch the event asks for.
	await expect.poll(() => opens(page)).toBeGreaterThanOrEqual(1);
	// The window coming back refetches the project once the bus listens (re-stat on focus).
	await page.waitForTimeout(200);
	await page.evaluate(() => window.dispatchEvent(new Event('focus')));
	await page.waitForTimeout(200);
	const settled = await opens(page);
	// A loop refetches in microtasks; half a second is thousands of passes.
	await page.waitForTimeout(500);
	const after = await opens(page);
	expect(after).toBe(settled);
	expect(after).toBeLessThanOrEqual(4);
});
