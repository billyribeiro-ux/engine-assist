// The P1 loop, keyboard only (EX-M2-03, AC-F028-01, A.7): create a project with ⌘N, copy and open
// chat steps with ⌘↵, mark them done with ⌘D, paste the Stage 1 packet back with ⌘⇧V, then in a
// Stage 2 project acknowledge the changed Claude Code config, run a CLI step with ⌘↵, move into the
// terminal with ⌘⇧T and back with ⌘⇧E. No mouse events; the native dialogs are the mock's script.
import { expect, type Page, test } from '@playwright/test';

const TRADELOG = '7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11';
const PACKET = [
	'=== HANDOFF PACKET · Stage 1 → Stage 2 ===',
	'PROJECT: Ledger',
	'APPROVED BY: ',
	'OPEN ITEMS: none',
	'PRESS RELEASE',
	'Ledger keeps every receipt.',
	'FAQ',
	'Q: Which shops? A: Any shop that prints a receipt.'
].join('\n');

const focused = (page: Page) =>
	page.evaluate(() => (document.activeElement?.textContent ?? '').replace(/\s+/g, ' ').trim());
const clipboard = (page: Page) =>
	page.evaluate(() =>
		(
			window as unknown as { __foremanMock: { getClipboard(): string } }
		).__foremanMock.getClipboard()
	);
const toast = (page: Page, text: string | RegExp) =>
	expect(page.locator('.toast').filter({ hasText: text }).first()).toBeVisible();

/** Moves the selection with J until the focused step row matches. */
async function selectStep(page: Page, re: RegExp): Promise<void> {
	for (let i = 0; i < 12 && !re.test(await focused(page)); i++) await page.keyboard.press('j');
	expect(await focused(page)).toMatch(re);
}
/** Tabs (or Shift+Tabs) until the focused control matches. */
async function tabTo(page: Page, re: RegExp, key: 'Tab' | 'Shift+Tab' = 'Tab'): Promise<void> {
	for (let i = 0; i < 80 && !re.test(await focused(page)); i++) await page.keyboard.press(key);
	expect(await focused(page)).toMatch(re);
}

test('P1 loop, keyboard only: create, chat steps, paste back', async ({ page }) => {
	await page.goto('/?scenario=empty');
	await expect(page.getByRole('link', { name: /Start your first project/ })).toBeFocused();

	await page.keyboard.press('Meta+n');
	await expect(page).toHaveURL(/\/setup$/);
	await expect(page.locator('#f-name')).toBeFocused();
	await page.keyboard.type('Ledger');
	await tabTo(page, /^Choose…$/);
	await page.keyboard.press('Enter');
	await tabTo(page, /^$/); // the claude.ai link field
	await page.keyboard.press('Tab');
	await expect(page.locator('#v-ROUGH_IDEA')).toBeFocused();
	await page.keyboard.type('A ledger for my receipts.');
	await page.keyboard.press('Tab');
	await page.keyboard.type('Me and my accountant.');
	await expect(page.getByText(/Fix \d+ fields? first/)).toHaveCount(0);
	await page.keyboard.press('Meta+Enter');
	await expect(page).toHaveURL(/\/projects\/[^/]+\/stages\/1$/);
	await toast(page, 'Project created: Ledger');

	// 1.1: open it, then copy and open; mark it done.
	await page.keyboard.press('Meta+Enter');
	await page.keyboard.press('Meta+Enter');
	await expect.poll(() => clipboard(page)).toContain('I want to build Ledger');
	await page.keyboard.press('Meta+d');
	await toast(page, '1.1 marked done');

	// 1.2 the same way.
	await selectStep(page, /^1\.2/);
	await page.keyboard.press('Meta+Enter');
	await page.keyboard.press('Meta+Enter');
	await page.keyboard.press('Meta+d');
	await toast(page, '1.2 marked done');

	// 1.4 writes the Stage 1 packet: copy it, then paste the packet back.
	await selectStep(page, /^1\.4/);
	await page.keyboard.press('Meta+Enter');
	await page.keyboard.press('Meta+Enter');
	await page.evaluate((text) => {
		(
			window as unknown as { __foremanMock: { setClipboard(t: string): void } }
		).__foremanMock.setClipboard(text);
	}, PACKET);
	await page.keyboard.press('Meta+Shift+v');
	await expect(page.getByText('Packet found on the clipboard: Stage 1 → Stage 2')).toBeVisible();
	await expect(page.getByRole('button', { name: /^Write \d+ files$/ })).toBeFocused();
	await page.keyboard.press('Enter');
	await toast(page, /^Wrote \d+ files; step done\./);
});

test('P1 loop, keyboard only: acknowledge config, run a CLI step, terminal focus', async ({
	page
}) => {
	await page.goto(`/projects/${TRADELOG}/stages/2`);
	await expect(
		page.getByRole('heading', { level: 1, name: "Architect's Design Studio" })
	).toBeVisible();

	// Run is refused until the changed config is acknowledged, and says so.
	await page.keyboard.press('Tab');
	await selectStep(page, /^x.2.1/);
	await page.keyboard.press('Meta+Enter');
	await page.keyboard.press('Meta+Enter');
	await toast(page, /acknowledge it first/);

	await tabTo(page, /^Acknowledge$/, 'Shift+Tab');
	await page.keyboard.press('Enter');
	await expect(page.getByRole('button', { name: 'Acknowledge' })).toHaveCount(0);

	await selectStep(page, /^x.2.1/);
	await page.keyboard.press('Meta+Enter');
	await page.keyboard.press('Meta+Enter');
	await expect(page.getByRole('tab', { name: /x-2-1|x.2.1/ })).toBeVisible();

	await page.keyboard.press('Meta+Shift+t');
	await expect(page.locator('.xterm-helper-textarea').first()).toBeFocused();
	await page.keyboard.press('Meta+Shift+e');
	await expect(page.locator('.xterm-helper-textarea').first()).not.toBeFocused();
	expect(await page.evaluate(() => document.activeElement?.closest('main') !== null)).toBe(true);
});
