// AC-XC-24 / WCAG 2.2 AA: axe finds no violations on any route, in Night (dark) and Vellum (light),
// with the mock world that fills each screen.
import AxeBuilder from '@axe-core/playwright';
import { expect, test } from '@playwright/test';

const TL = '7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11';
const SW = '5e1f2a3b-4c5d-4e6f-8a7b-9c0d1e2f3a4b';
const FM = 'f0e1d2c3-b4a5-4697-8879-6a5b4c3d2e1f';

const ROUTES: [string, string, string][] = [
	['Today', '/', 'Today'],
	['Today, empty', '/?scenario=empty', 'Today'],
	['First run', '/first-run?scenario=firstRun', 'Set up the site office'],
	['Setup', '/setup', 'New project'],
	['Setup wizard', '/setup/wizard', 'Name and folder'],
	['Stage workspace (CLI)', `/projects/${TL}/stages/2`, "Architect's Design Studio"],
	['Stage workspace (chat)', `/projects/${TL}/stages/1?scenario=stage1`, 'Meet the Architect'],
	['Step', `/projects/${TL}/stages/2/steps/2.4`, "Architect's Design Studio"],
	['Stage workspace (close-out checklist)', `/projects/${SW}/stages/7`, ''],
	['Gate', `/projects/${TL}/stages/5/gate?scenario=stage5`, ''],
	['Gate, signable', `/projects/${SW}/stages/7/gate`, ''],
	['Packet, invalid', `/projects/${TL}/stages/2/packet?scenario=invalidPacket`, 'Stage 2 packet'],
	['Packet, none', `/projects/${FM}/stages/4/packet`, 'Stage 4 packet'],
	['Outside change', `/projects/${TL}/changes?scenario=outsideChange`, ''],
	['Assembler', `/projects/${TL}/assembler?scenario=stage4`, 'Build package'],
	['Doctor', '/doctor', 'Doctor'],
	['Run log', '/run-log', 'Run log'],
	['Settings', '/settings', 'Settings'],
	['Template health', '/template-health', 'Template health']
];

for (const scheme of ['dark', 'light'] as const) {
	test.describe(`${scheme === 'dark' ? 'Night' : 'Vellum'} theme`, () => {
		test.use({ colorScheme: scheme });
		for (const [name, url, heading] of ROUTES) {
			test(`${name}: no axe violations`, async ({ page }) => {
				await page.goto(url);
				const h1 = page.getByRole('heading', { level: 1 }).first();
				await expect(h1).toBeVisible();
				if (heading) await expect(h1).toContainText(heading);
				await page.waitForLoadState('networkidle');
				await page.waitForTimeout(300);
				const axe = await new AxeBuilder({ page })
					.withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
					.analyze();
				expect(
					axe.violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`)
				).toEqual([]);
			});
		}
		test('Command palette open: no axe violations', async ({ page }) => {
			await page.goto(`/projects/${TL}/stages/2`);
			await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
			await page.keyboard.press('Meta+k');
			await expect(page.getByRole('dialog')).toBeVisible();
			await page.keyboard.type('stack');
			const axe = await new AxeBuilder({ page }).analyze();
			expect(axe.violations.map((v) => v.id)).toEqual([]);
		});
	});
}
