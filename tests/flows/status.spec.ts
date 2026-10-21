// AC-XC-25: any status is shown by shape plus color, never color alone. On every screen, in both themes:
// text painted in a status color (pass, fail, attention) has a glyph (an svg) beside it (a button's
// label names an action, and a diff's + and - are signs, so neither counts), and a small
// solid mark in a status color is itself a glyph or carries words. The same audit must catch a planted
// bare colored dot, so a passing run means something.
import { expect, type Page, test } from '@playwright/test';
import { SCREENS, THEMES } from './screens';

/** Describes every status color mark that has no shape beside it. */
function colorOnlyMarks(page: Page): Promise<string[]> {
	return page.evaluate(() => {
		const probe = document.createElement('span');
		document.body.append(probe);
		const resolve = (name: string): string => {
			probe.style.color = `var(${name})`;
			return getComputedStyle(probe).color;
		};
		const colors = new Map<string, string>(
			['--pass', '--fail', '--attention'].map((n) => [resolve(n), n.slice(2)])
		);
		probe.remove();
		const found: string[] = [];
		const shown = (el: Element): boolean => {
			const r = el.getBoundingClientRect();
			return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== 'hidden';
		};
		const hasShape = (el: Element): boolean => {
			let scope: Element | null = el;
			for (let i = 0; i < 4 && scope && scope !== document.body; i++) {
				if (scope.querySelector('svg')) return true;
				scope = scope.parentElement;
			}
			return false;
		};
		const where = (el: Element) =>
			`${el.tagName.toLowerCase()}${el.className && typeof el.className === 'string' ? '.' + el.className.trim().split(/\s+/).join('.') : ''} "${el.textContent.replace(/\s+/g, ' ').trim().slice(0, 30)}"`;
		for (const el of document.querySelectorAll('body *')) {
			if (el.closest('svg, .xterm, [aria-hidden="true"]') || !shown(el)) continue;
			const cs = getComputedStyle(el);
			// Words in a status color.
			const own = [...el.childNodes].some(
				(n) => n.nodeType === Node.TEXT_NODE && (n.textContent ?? '').trim() !== ''
			);
			const tone = colors.get(cs.color);
			// A button's label names an action, not a state; a lone sign (the + and - of a diff) is a shape.
			const sign = /^[^\p{L}\p{N}]{1,2}$/u.test(el.textContent.trim());
			if (own && tone && !sign && !el.closest('button') && !hasShape(el))
				found.push(`${tone} text without a shape: ${where(el)}`);
			// A small solid mark: a dot or a bar that is only a colored box.
			const fill = colors.get(cs.backgroundColor);
			if (fill) {
				const r = el.getBoundingClientRect();
				const small = r.width <= 16 && r.height <= 16;
				const words = el.textContent.trim() !== '';
				if (small && !words && !el.querySelector('svg')) {
					found.push(`${fill} mark without a shape: ${where(el)}`);
				}
			}
		}
		return found;
	});
}

for (const theme of THEMES) {
	test.describe(`${theme.name} theme`, () => {
		test.use({ colorScheme: theme.scheme });

		for (const screen of SCREENS) {
			test(`${screen.id}: no status is shown by color alone`, async ({ page }) => {
				await page.goto(screen.url);
				await expect(page.getByRole('heading', { level: 1 }).first()).toBeVisible();
				await page.waitForLoadState('networkidle');
				if (screen.open) await screen.open(page);
				await page.waitForTimeout(300);
				expect(await colorOnlyMarks(page)).toEqual([]);
			});
		}

		test('the audit catches a planted color-only dot and bare colored words', async ({ page }) => {
			await page.goto('/');
			await expect(page.getByRole('heading', { level: 1 }).first()).toBeVisible();
			await page.evaluate(() => {
				const dot = document.createElement('span');
				dot.id = 'planted-dot';
				dot.style.cssText =
					'display:inline-block;width:8px;height:8px;border-radius:50%;background:var(--pass)';
				const words = document.createElement('p');
				words.textContent = 'Planted';
				words.style.color = 'var(--fail)';
				document.body.prepend(dot, words);
			});
			const found = await colorOnlyMarks(page);
			expect(found.some((f) => f.startsWith('pass mark without a shape'))).toBe(true);
			expect(found.some((f) => f.startsWith('fail text without a shape'))).toBe(true);
		});
	});
}
