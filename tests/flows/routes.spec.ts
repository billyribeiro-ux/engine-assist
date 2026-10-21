// M2-04 (EX-M2-01): every P1 route renders under the root layout in mock mode: the sidebar, the
// skip link, the polite live region, a single main landmark, and (on the stage workspace) the
// terminal drawer host. A route that throws or fails to load fails here before it fails axe.
import { expect, test } from '@playwright/test';

const TL = '7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11';

const ROUTES: [string, string][] = [
	['Today', '/'],
	['First run', '/first-run?scenario=firstRun'],
	['Setup', '/setup'],
	['Setup wizard', '/setup/wizard'],
	['Stage workspace', `/projects/${TL}/stages/2`],
	['Step', `/projects/${TL}/stages/2/steps/2.4`],
	['Gate', `/projects/${TL}/stages/5/gate?scenario=stage5`],
	['Packet', `/projects/${TL}/stages/2/packet?scenario=invalidPacket`],
	['Outside change', `/projects/${TL}/changes?scenario=outsideChange`],
	['Assembler', `/projects/${TL}/assembler?scenario=stage4`],
	['Doctor', '/doctor'],
	['Run log', '/run-log'],
	['Settings', '/settings'],
	['Template health', '/template-health']
];

for (const [name, url] of ROUTES) {
	test(`${name} renders under the root layout`, async ({ page }) => {
		const errors: string[] = [];
		page.on('pageerror', (e) => errors.push(e.message));
		await page.goto(url);
		await expect(page.getByRole('heading', { level: 1 }).first()).toBeVisible();
		await expect(page.getByRole('navigation', { name: 'Foreman' })).toBeVisible();
		await expect(page.getByRole('link', { name: 'Skip to content' })).toHaveCount(1);
		await expect(page.locator('[role="status"][aria-live="polite"]').first()).toBeAttached();
		await expect(page.locator('main')).toHaveCount(1);
		expect(errors).toEqual([]);
	});
}

test('the stage workspace hosts the terminal drawer', async ({ page }) => {
	await page.goto(`/projects/${TL}/stages/2`);
	await expect(page.getByRole('region', { name: 'Terminal drawer' })).toBeAttached();
});
