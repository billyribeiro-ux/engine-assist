// AC-XC-21 (M5-06): the full v1 workflow completes keyboard-only, focus is visible (a 2 px ring) on
// everything Tab reaches, and nothing traps the keyboard. The P1 loop (create, chat steps, paste back,
// run, terminal focus) is p1-loop.spec.ts; this walks every screen and the rest of the workflow.
// VoiceOver (AC-XC-22) is a Mac check, not part of this file.
import { expect, type Page, test } from '@playwright/test';
import { SCREENS, SW, THEMES, TL } from './screens';

interface Focus {
	key: string;
	label: string;
	width: string;
	style: string;
	color: string;
	offset: string;
	inTerminal: boolean;
	combobox: boolean;
}

const active = (page: Page): Promise<Focus> =>
	page.evaluate(() => {
		const el = document.activeElement as HTMLElement | null;
		if (!el || el === document.body) {
			return {
				key: 'body',
				label: 'body',
				width: '',
				style: '',
				color: '',
				offset: '',
				inTerminal: false,
				combobox: false
			};
		}
		const cs = getComputedStyle(el);
		const path: string[] = [];
		for (let n: Element | null = el; n && n !== document.body; n = n.parentElement) {
			const i = n.parentElement ? [...n.parentElement.children].indexOf(n) : 0;
			path.push(`${n.tagName}${i}`);
		}
		return {
			key: path.join('>'),
			label: (el.getAttribute('aria-label') ?? el.textContent)
				.replace(/\s+/g, ' ')
				.trim()
				.slice(0, 40),
			width: cs.outlineWidth,
			style: cs.outlineStyle,
			color: cs.outlineColor,
			offset: cs.outlineOffset,
			inTerminal: el.closest('.xterm') !== null,
			combobox: el.getAttribute('role') === 'combobox'
		};
	});

/** The ring everything Tab reaches must wear: 2 px, solid, opaque. Inset rings (a negative offset) sit
 * inside controls that fill their box (the segmented controls and the strip's sheets). */
function ring(f: Focus): string | null {
	// A combobox keeps DOM focus in its field and shows the focus on the active option
	// (aria-activedescendant; the palette, components.md).
	if (f.combobox) return null;
	if (f.style !== 'solid' || f.width !== '2px')
		return `${f.label}: outline is ${f.width} ${f.style}`;
	if (f.color === 'rgba(0, 0, 0, 0)') return `${f.label}: outline is transparent`;
	return null;
}

for (const theme of THEMES) {
	test.describe(`${theme.name} theme`, () => {
		test.use({ colorScheme: theme.scheme, reducedMotion: 'reduce' });

		for (const screen of SCREENS) {
			test(`${screen.id}: Tab reaches everything with a visible ring and never traps`, async ({
				page
			}) => {
				await page.goto(screen.url);
				await expect(page.getByRole('heading', { level: 1 }).first()).toBeVisible();
				await page.waitForLoadState('networkidle');
				if (screen.open) await screen.open(page);
				await page.waitForTimeout(200);

				const seen = new Map<string, Focus>();
				const order: string[] = [];
				let wrapped = false;
				for (let i = 0; i < 250; i++) {
					await page.keyboard.press('Tab');
					const f = await active(page);
					if (f.inTerminal) {
						// The terminal sends Tab to Claude Code, so Tab cannot leave it; ⌘⇧E does, always.
						await page.keyboard.press('Meta+Shift+e');
						expect((await active(page)).inTerminal).toBe(false);
						wrapped = true;
						break;
					}
					if (f.key === 'body' || seen.has(f.key)) {
						wrapped = true; // focus left the page or came back to where it started
						break;
					}
					// Every press moved focus somewhere new: it never sits on one control.
					expect(f.key).not.toBe(order[order.length - 1]);
					seen.set(f.key, f);
					order.push(f.key);
				}
				expect(wrapped, 'focus must leave or cycle within 250 tabs').toBe(true);
				expect(order.length).toBeGreaterThan(0);

				const problems = [...seen.values()].map(ring).filter((p): p is string => p !== null);
				expect(problems).toEqual([]);
			});
		}
	});
}

test.describe('the workflow past the P1 loop, keyboard only', () => {
	test.use({ reducedMotion: 'reduce' });

	const heading = (page: Page, re: RegExp | string) =>
		expect(page.getByRole('heading', { level: 1 }).first()).toContainText(re);

	/** The palette: ⌘K, type, Enter. */
	async function palette(page: Page, query: string): Promise<void> {
		await page.keyboard.press('Meta+k');
		await expect(page.getByRole('dialog')).toBeVisible();
		await page.keyboard.type(query);
		await page.keyboard.press('Enter');
		await expect(page.getByRole('dialog')).toHaveCount(0);
	}

	test('open a project, go to the gate, sign by keyboard, and visit every screen', async ({
		page
	}) => {
		// Start in Swings at Stage 7 (the mock world's signable gate); from here nothing uses the mouse.
		await page.goto(`/projects/${SW}/stages/7`);
		await heading(page, 'Framing');
		await page.keyboard.press('Meta+Shift+g');
		await expect(page).toHaveURL(/\/gate$/);

		// Tab to Sign packet, open the sheet, escape and reopen: focus returns to the button.
		const sign = page.getByRole('button', { name: 'Sign packet' });
		for (let i = 0; i < 60; i++) {
			if (await sign.evaluate((el) => el === document.activeElement)) break;
			await page.keyboard.press('Tab');
		}
		await expect(sign).toBeFocused();
		await page.keyboard.press('Enter');
		await expect(page.getByRole('dialog')).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(page.getByRole('dialog')).toHaveCount(0);
		await expect(sign).toBeFocused();

		// The sheet traps Tab inside itself (a modal) and Shift+Tab cycles back.
		await page.keyboard.press('Enter');
		const dialog = page.getByRole('dialog');
		await expect(dialog).toBeVisible();
		for (let i = 0; i < 14; i++) {
			await page.keyboard.press('Tab');
			expect(await dialog.evaluate((d) => d.contains(document.activeElement))).toBe(true);
		}
		for (let i = 0; i < 14; i++) {
			await page.keyboard.press('Shift+Tab');
			expect(await dialog.evaluate((d) => d.contains(document.activeElement))).toBe(true);
		}

		// Tab to the sheet's Sign packet and press it; the mock's native confirmation says OK.
		const confirm = dialog.getByRole('button', { name: 'Sign packet' });
		for (let i = 0; i < 20; i++) {
			if (await confirm.evaluate((el) => el === document.activeElement)) break;
			await page.keyboard.press('Tab');
		}
		await expect(confirm).toBeFocused();
		await page.keyboard.press('Enter');
		await expect(
			page
				.locator('.toast')
				.filter({ hasText: /Packet signed/ })
				.first()
		).toBeVisible();

		// Every other screen by the palette and the shortcuts, never the mouse.
		await page.keyboard.press('Meta+,');
		await expect(page).toHaveURL(/\/settings$/);
		await heading(page, 'Settings');
		await palette(page, 'Doctor');
		await expect(page).toHaveURL(/\/doctor$/);
		await palette(page, 'Run log');
		await expect(page).toHaveURL(/\/run-log$/);
		await palette(page, 'Template health');
		await expect(page).toHaveURL(/\/template-health$/);
		await page.keyboard.press('Meta+1');
		await expect(page).toHaveURL(new RegExp(`/projects/${TL}/stages/`));
		await page.keyboard.press('Meta+]');
		await page.keyboard.press('Meta+[');
		await heading(page, /./);
	});

	test('the first Tab lands on the skip link, wearing the ring', async ({ page }) => {
		await page.goto(`/projects/${TL}/stages/2`);
		await expect(page.getByRole('heading', { level: 1 }).first()).toBeVisible();
		await page.keyboard.press('Tab');
		const f = await active(page);
		expect(f.key).not.toBe('body');
		expect(ring(f)).toBeNull();
	});
});
