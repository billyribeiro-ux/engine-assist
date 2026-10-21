// M3-08 (AC-XC-07, SR-20, ADR-014, ADR-020; T-001, T-007): the production CSP from tauri.conf.json,
// served the way Tauri serves it (a sha256 hash for each inline script, a nonce for each `<style>`
// element in the HTML; nothing else added), holds on every screen of the built app: no CSP
// violation while the screens render (the terminal included), and an injected inline `<script>`
// raises `securitypolicyviolation` and doesn't run. The policy has no 'unsafe-inline' and no
// remote source. The real app (WKWebView, Tauri's own injection) is `e2e/specs/02-csp.e2e.ts` on
// the Mac.
import { createHash, randomInt } from 'node:crypto';
import { readFileSync } from 'node:fs';

import { expect, type Page, test } from '@playwright/test';

import { configuredCsp } from '../../e2e/helpers/csp.ts';

const conf: unknown = JSON.parse(
	readFileSync(new URL('../../src-tauri/tauri.conf.json', import.meta.url), 'utf8')
);
const TL = '7a1c3e52-0b9d-4c1e-9f2a-5d6e8b0c4a11';
const SW = '5e1f2a3b-4c5d-4e6f-8a7b-9c0d1e2f3a4b';

/** What Tauri does to an HTML asset and its policy (tauri 2.12.4 `manager::set_csp`). */
function asTauriServes(html: string): { html: string; policy: string } {
	const policy = new Map(configuredCsp(conf, 'csp'));
	const hashes = [...html.matchAll(/<script(?![^>]*\bsrc=)[^>]*>([\s\S]*?)<\/script>/g)].map(
		(m) =>
			`'sha256-${createHash('sha256')
				.update(m[1] ?? '')
				.digest('base64')}'`
	);
	const nonces: string[] = [];
	const out = html.replace(/<style(?=[\s>])/g, () => {
		const n = String(randomInt(1, 2 ** 47));
		nonces.push(`'nonce-${n}'`);
		return `<style nonce="${n}"`;
	});
	policy.set('script-src', [...(policy.get('script-src') ?? []), ...hashes]);
	if (nonces.length) policy.set('style-src', [...(policy.get('style-src') ?? []), ...nonces]);
	return {
		html: out,
		policy: [...policy].map(([k, v]) => `${k} ${v.join(' ')}`).join('; ')
	};
}

interface Violation {
	directive: string;
	blocked: string;
	sample: string;
	at: string;
}

async function served(page: Page): Promise<string[]> {
	const policies: string[] = [];
	await page.addInitScript(() => {
		const w = window as unknown as { __csp: unknown[] };
		w.__csp = [];
		document.addEventListener('securitypolicyviolation', (e) => {
			const el =
				e.target instanceof Element
					? `<${e.target.tagName.toLowerCase()}> ${e.target.textContent.slice(0, 120)}`
					: '';
			w.__csp.push({
				directive: e.effectiveDirective,
				blocked: e.blockedURI,
				sample: e.sample,
				at: `${e.sourceFile}:${e.lineNumber} ${el}`
			});
		});
	});
	await page.route('**/*', async (route) => {
		const res = await route.fetch();
		if (!(res.headers()['content-type'] ?? '').includes('text/html'))
			return route.fulfill({ response: res });
		const { html, policy } = asTauriServes(await res.text());
		policies.push(policy);
		return route.fulfill({
			response: res,
			body: html,
			headers: { ...res.headers(), 'content-security-policy': policy }
		});
	});
	return policies;
}

const violations = (page: Page) =>
	page.evaluate(() => (window as unknown as { __csp: Violation[] }).__csp);

const SCREENS: [string, string][] = [
	['Today', '/'],
	['First run', '/first-run?scenario=firstRun'],
	['Setup', '/setup'],
	['Setup wizard', '/setup/wizard'],
	['Stage workspace with the terminal', `/projects/${TL}/stages/2`],
	['Stage workspace (chat)', `/projects/${TL}/stages/1?scenario=stage1`],
	['Step', `/projects/${TL}/stages/2/steps/2.4`],
	['Gate', `/projects/${SW}/stages/7/gate`],
	['Packet', `/projects/${TL}/stages/2/packet?scenario=invalidPacket`],
	['Outside change', `/projects/${TL}/changes?scenario=outsideChange`],
	['Assembler', `/projects/${TL}/assembler?scenario=stage4`],
	['Doctor', '/doctor'],
	['Run log', '/run-log'],
	['Settings', '/settings'],
	['Template health', '/template-health']
];

test.describe('csp', () => {
	test('csp::production_policy_holds_on_every_screen', async ({ page }) => {
		const policies = await served(page);
		for (const [name, url] of SCREENS) {
			await page.goto(url);
			await expect(page.getByRole('heading', { level: 1 }).first(), name).toBeVisible();
			await page.waitForLoadState('networkidle');
			await page.waitForTimeout(300);
			expect(await violations(page), name).toEqual([]);
		}
		// The terminal drew its rows under the policy (its runtime styles carry the page's nonce).
		await page.goto(`/projects/${TL}/stages/2`);
		const rows = page.locator('.xterm-rows').first();
		await expect(rows).toBeVisible();
		await expect(rows).not.toHaveText('');
		expect(await violations(page)).toEqual([]);
		const xtermStyles = await page.evaluate(() =>
			[...document.querySelectorAll('style')]
				.filter((s) => s.textContent.includes('.xterm'))
				.map((s) => s.nonce !== '')
		);
		expect(xtermStyles.length).toBeGreaterThan(0);
		expect(xtermStyles.every(Boolean)).toBe(true);
		// The policy served is the production one: no 'unsafe-inline', no remote source.
		const policy = policies.at(-1) ?? '';
		expect(policy).not.toContain("'unsafe-inline'");
		expect(policy).not.toContain("'unsafe-eval'");
		expect(policy).not.toMatch(/https?:\/\/(?!ipc\.localhost)/);
		expect(policy).toMatch(/script-src 'self' 'sha256-[A-Za-z0-9+/=]+'/);
	});

	test('csp::injected_inline_script_blocked', async ({ page }) => {
		await served(page);
		await page.goto('/');
		await expect(page.getByRole('heading', { level: 1, name: 'Today' })).toBeVisible();
		const ran = await page.evaluate(async () => {
			const w = window as unknown as { __pwned?: number };
			const s = document.createElement('script');
			s.textContent = 'window.__pwned = 1';
			document.head.append(s);
			const st = document.createElement('style');
			st.textContent = 'body { display: none }';
			document.head.append(st);
			const div = document.createElement('div');
			div.setAttribute('style', 'display: none');
			document.body.append(div);
			await new Promise((r) => setTimeout(r, 200));
			return { pwned: w.__pwned ?? null, hidden: getComputedStyle(document.body).display };
		});
		expect(ran.pwned).toBeNull();
		expect(ran.hidden).not.toBe('none');
		const v = await violations(page);
		expect(v.some((x) => x.directive === 'script-src-elem' && x.blocked === 'inline')).toBe(true);
		expect(v.some((x) => x.directive === 'style-src-elem' && x.blocked === 'inline')).toBe(true);
		expect(v.some((x) => x.directive === 'style-src-attr' && x.blocked === 'inline')).toBe(true);
	});
});
