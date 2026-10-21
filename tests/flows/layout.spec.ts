// AC-F030-10 / AC-XC-26: no screen scrolls sideways at the two window sizes the design is drawn for
// (1280 and 1440 wide), and nothing in the main column sticks out of it.
import { expect, test } from '@playwright/test';
import { SCREENS, VIEWPORTS } from './screens';

for (const vp of VIEWPORTS) {
	test.describe(`${vp.width} px wide`, () => {
		test.use({ viewport: vp, reducedMotion: 'reduce' });
		for (const screen of SCREENS) {
			test(`${screen.id}: no sideways scroll`, async ({ page }) => {
				await page.goto(screen.url);
				await expect(page.getByRole('heading', { level: 1 }).first()).toBeVisible();
				await page.waitForLoadState('networkidle');
				if (screen.open) await screen.open(page);
				await page.waitForTimeout(300);
				const problems = await page.evaluate(() => {
					const out: string[] = [];
					const doc = document.documentElement;
					if (doc.scrollWidth > doc.clientWidth) {
						out.push(`page is ${doc.scrollWidth} wide in a ${doc.clientWidth} window`);
					}
					const main = document.querySelector('main');
					if (main) {
						const edge = main.getBoundingClientRect().right;
						for (const el of main.querySelectorAll('*')) {
							if (el.closest('.xterm, svg') || getComputedStyle(el).position === 'fixed') continue;
							const r = el.getBoundingClientRect();
							if (r.width > 0 && r.right > edge + 1) {
								out.push(
									`${el.tagName.toLowerCase()}.${el.className.slice(0, 30)} ends at ${Math.round(r.right)}, main at ${Math.round(edge)}`
								);
								if (out.length > 6) break;
							}
						}
					}
					return out;
				});
				expect(problems).toEqual([]);
			});
		}
	});
}
