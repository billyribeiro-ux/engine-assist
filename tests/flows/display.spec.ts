// M5-03, M5-04 (AC-F030-09, AC-F030-10, AC-XC-23): the stamp, Reduce Motion, Increase Contrast and
// the responsive layout, in a real browser against the mock world.
import { expect, test, type Page } from '@playwright/test';

const SW = '5e1f2a3b-4c5d-4e6f-8a7b-9c0d1e2f3a4b';
const TL = '7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11';

async function sign(page: Page): Promise<void> {
	await page.goto(`/projects/${SW}/stages/7/gate`);
	await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
	await page.getByRole('button', { name: 'Sign packet' }).click();
	const dialog = page.getByRole('dialog');
	await expect(dialog).toBeVisible();
	await page.getByRole('button', { name: 'Sign packet' }).last().click();
	await expect(page.getByText(/Packet signed\. Committed/).first()).toBeVisible();
}

const stampOn = (page: Page) =>
	page.evaluate(() => {
		const g = document.querySelector('.sheet .stamp.is-landing');
		if (!g) return null;
		const cs = getComputedStyle(g);
		return { name: cs.animationName, duration: cs.animationDuration };
	});

test.describe('the stamp (AC-F030-09)', () => {
	test('plays for 220 ms on the signed sheet when motion is allowed', async ({ page }) => {
		await page.emulateMedia({ reducedMotion: 'no-preference' });
		await sign(page);
		await expect
			.poll(async () => {
				const a = await stampOn(page);
				return a && { animated: a.name.endsWith('stamp-in'), duration: a.duration };
			})
			.toEqual({ animated: true, duration: '0.22s' });
	});

	test('does not animate under Reduce Motion', async ({ page }) => {
		await page.emulateMedia({ reducedMotion: 'reduce' });
		await sign(page);
		await expect.poll(() => stampOn(page)).toEqual({ name: 'none', duration: '0s' });
	});
});

test.describe('Increase Contrast (AC-XC-23)', () => {
	for (const scheme of ['dark', 'light'] as const) {
		test(`${scheme}: secondary text becomes primary text and the status colors step up`, async ({
			page
		}) => {
			await page.emulateMedia({ colorScheme: scheme, contrast: 'no-preference' });
			await page.goto(`/projects/${TL}/stages/2`);
			await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
			const read = () =>
				page.evaluate(() => {
					const cs = getComputedStyle(document.documentElement);
					const px = (n: string) => cs.getPropertyValue(n).trim();
					return {
						muted: px('--ink-muted'),
						ink: px('--ink'),
						pass: px('--pass'),
						fail: px('--fail'),
						accent: px('--accent-ink')
					};
				});
			const before = await read();
			expect(before.muted).not.toBe(before.ink);
			await page.emulateMedia({ colorScheme: scheme, contrast: 'more' });
			const after = await read();
			expect(after.muted).toBe(after.ink);
			expect(after.pass).not.toBe(before.pass);
			expect(after.fail).not.toBe(before.fail);
			expect(after.accent).not.toBe(before.accent);
		});
	}
});

test.describe('layout from 1280 × 800 down (AC-F030-10)', () => {
	const open = async (page: Page, w: number, h: number) => {
		await page.setViewportSize({ width: w, height: h });
		await page.goto(`/projects/${TL}/stages/2`);
		await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
	};
	const overflow = (page: Page) =>
		page.evaluate(() => document.documentElement.scrollWidth - innerWidth);
	const sidebarWidth = (page: Page) =>
		page.locator('nav.sidebar').evaluate((e) => e.getBoundingClientRect().width);

	test('wide: the sidebar is full and the inspector is beside the workspace', async ({ page }) => {
		await open(page, 1440, 900);
		await expect(page.getByRole('complementary', { name: 'Inspector' })).toBeVisible();
		expect(await sidebarWidth(page)).toBeGreaterThan(220);
		expect(await overflow(page)).toBeLessThanOrEqual(0);
	});

	test('1280 × 800 works: the inspector collapses first and ⌘⌥I brings it back as an overlay', async ({
		page
	}) => {
		await open(page, 1280, 800);
		await expect(page.getByRole('complementary', { name: 'Inspector' })).toBeHidden();
		expect(await sidebarWidth(page)).toBeGreaterThan(150);
		await expect(page.getByRole('navigation', { name: /drawing set/ })).toBeVisible();
		expect(await overflow(page)).toBeLessThanOrEqual(0);
		await page.keyboard.press('Meta+Alt+i');
		await expect(page.getByRole('complementary', { name: 'Inspector' })).toBeVisible();
		await page.keyboard.press('Meta+Alt+i');
		await expect(page.getByRole('complementary', { name: 'Inspector' })).toBeHidden();
	});

	test('narrower: the sidebar collapses to icons, nothing scrolls sideways', async ({ page }) => {
		await open(page, 1000, 700);
		expect(await sidebarWidth(page)).toBeLessThan(80);
		await expect(page.getByRole('link', { name: /Run log/ })).toBeVisible();
		await expect(page.getByRole('navigation', { name: /drawing set/ })).toBeVisible();
		expect(await overflow(page)).toBeLessThanOrEqual(0);
	});
});
