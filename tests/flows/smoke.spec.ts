// Flow smoke (M0-05 example; ADR-015 layer 4): the Today route renders from MockIpcClient and axe
// finds no violations.
import AxeBuilder from '@axe-core/playwright';
import { expect, test } from '@playwright/test';

test('Today renders against the mock with no axe violations', async ({ page }) => {
	await page.goto('/');
	await expect(page.getByRole('heading', { level: 1, name: 'Today' })).toBeVisible();
	await expect(page.getByText('Welcome to Foreman.')).toBeVisible();

	const axe = await new AxeBuilder({ page }).analyze();
	expect(axe.violations).toEqual([]);
});
