// M4-31 (AC-F025-01, AC-F025-03, AC-F025-05): add a prompt for this project through the dialog,
// by keyboard, in a real browser (the dialog must be operable while the app behind it is inert).
import { expect, test } from '@playwright/test';

const TL = '7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11';

test('add a prompt to Stage 2 by keyboard; it says it was added for this project', async ({
	page
}) => {
	await page.goto(`/projects/${TL}/stages/2`);
	await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
	await page.getByRole('button', { name: /Add to Stage 2 after 2\.4/ }).focus();
	await page.keyboard.press('Enter');
	await page.keyboard.press('Enter');
	const dialog = page.getByRole('dialog');
	await expect(dialog).toBeVisible();
	await dialog.getByRole('textbox', { name: 'Title' }).fill('Name check');
	await dialog.getByRole('textbox', { name: 'Prompt' }).fill('Check the names.');
	await dialog.getByRole('button', { name: 'Add' }).click();
	await expect(dialog).toBeHidden();
	await expect(page.getByRole('heading', { name: /Name check/ })).toBeVisible();
	await expect(page.getByText('Added for this project').first()).toBeVisible();
});

test('bypassPermissions is refused and the sentence stays in the dialog', async ({ page }) => {
	await page.goto(`/projects/${TL}/stages/2`);
	await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
	await page.getByRole('button', { name: /Add to Stage 2 after 2\.4/ }).click();
	await page.getByRole('menuitem', { name: /Add prompt/ }).click();
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('textbox', { name: 'Title' }).fill('Fast');
	await dialog.getByRole('textbox', { name: 'Prompt' }).fill('Run with bypassPermissions.');
	await dialog.getByRole('button', { name: 'Add' }).click();
	await expect(dialog.getByRole('alert')).toContainText('bypassPermissions is never allowed');
	await expect(dialog).toBeVisible();
});
