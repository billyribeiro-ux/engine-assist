// AC-XC-26 / EX-M5-02: every screen matches its mockup in both themes. The mockups are static HTML
// that share the app's token sheet, so a screen "matches" when the app's shell sits where the mockup's
// does (to 2 px) and every element both draw carries the same computed colors, type and radii. The
// content of a screen differs legitimately (mock data, spec additions; see docs/design/parity.md), so
// whole-page pixels are only compared, never asserted, except where the data is the same.
//
// FM_WRITE_SHOTS=1 also writes the app's screenshots to docs/design/screenshots/app/; FM_DIFF_TSV=<file>
// appends each screen's whole-page difference to that file.
import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { expect, type Page, test } from '@playwright/test';
import { SCREENS, THEMES, VIEWPORTS } from './screens';

const here = dirname(fileURLToPath(import.meta.url));
const MOCKUPS = resolve(here, '../../docs/design/mockups');
const SHOTS = resolve(here, '../../docs/design/screenshots/app');

const GEOMETRY = ['.sidebar', '.titlebar', 'main', '.strip', '.title-block'];
const STYLED = [
	'body',
	'h1',
	'.sidebar',
	'.titlebar',
	'main',
	'.strip',
	'.title-block',
	'.btn.primary',
	'.table',
	'.banner',
	'.callout'
];
const PROPS = [
	'backgroundColor',
	'color',
	'fontFamily',
	'fontSize',
	'fontWeight',
	'borderRadius',
	'borderTopColor',
	'borderTopWidth'
] as const;

interface Seen {
	rect: [number, number, number, number];
	style: Record<(typeof PROPS)[number], string>;
	text: string;
}
type Sample = Record<string, Seen | null>;

/** Where the shared elements are and how they are painted. */
function sample(page: Page): Promise<Sample> {
	return page.evaluate(
		([selectors, props]) => {
			const out: Record<string, unknown> = {};
			for (const sel of selectors) {
				const el = document.querySelector(sel);
				if (!el) {
					out[sel] = null;
					continue;
				}
				const r = el.getBoundingClientRect();
				const cs = getComputedStyle(el);
				out[sel] = {
					rect: [Math.round(r.x), Math.round(r.y), Math.round(r.width), Math.round(r.height)],
					style: Object.fromEntries(props.map((p) => [p, cs[p as never] as string])),
					text: (el.textContent ?? '').replace(/\s+/g, ' ').trim().slice(0, 40)
				};
			}
			return out;
		},
		[[...new Set([...GEOMETRY, ...STYLED])], [...PROPS]] as [string[], string[]]
	) as Promise<Sample>;
}

/** Mean absolute difference of two PNGs, 0 (same) to 1 (opposite), drawn in the browser. */
async function pixelDiff(page: Page, a: Buffer, b: Buffer): Promise<number> {
	return page.evaluate(
		async ([x, y]) => {
			const load = (b64: string) =>
				new Promise<HTMLImageElement>((ok, no) => {
					const img = new Image();
					img.onload = () => ok(img);
					img.onerror = no;
					img.src = `data:image/png;base64,${b64}`;
				});
			const [ia, ib] = await Promise.all([load(x), load(y)]);
			const w = Math.min(ia.width, ib.width);
			const h = Math.min(ia.height, ib.height);
			const pixels = (img: HTMLImageElement) => {
				const c = document.createElement('canvas');
				c.width = w;
				c.height = h;
				const g = c.getContext('2d');
				g?.drawImage(img, 0, 0);
				return g?.getImageData(0, 0, w, h).data ?? new Uint8ClampedArray();
			};
			const pa = pixels(ia);
			const pb = pixels(ib);
			let sum = 0;
			for (let i = 0; i < pa.length; i += 4) {
				sum += Math.abs(pa[i] - pb[i]) + Math.abs(pa[i + 1] - pb[i + 1]) + Math.abs(pa[i + 2] - pb[i + 2]);
			}
			return sum / ((pa.length / 4) * 3 * 255);
		},
		[a.toString('base64'), b.toString('base64')]
	);
}

// Documented differences from the mockup (docs/design/parity.md), by `<screen>@<width>`: the geometry of
// these elements is not compared. The title block sits 8 px lower at 1440 on the workspace because the app
// adds an "Export run sheet…" link under the subtitle (F-027); the assembler's mockup reserves an empty
// 300 px inspector column that the app does not draw, as the assembler has nothing to inspect.
const EXEMPT: Record<string, string[]> = {
	'workspace@1440': ['.title-block'],
	'palette@1440': ['.title-block'],
	'assembler@1440': ['main']
};

// Where the data is the same, the whole page is within this much of the mockup.
const SAME_DATA: Record<string, number> = { gate: 0.02, assembler: 0.02 };

for (const theme of THEMES) {
	for (const vp of VIEWPORTS) {
		test.describe(`${theme.name} ${vp.width}`, () => {
			test.use({ colorScheme: theme.scheme, viewport: vp, reducedMotion: 'reduce' });
			for (const screen of SCREENS) {
				test(`${screen.id}: matches ${screen.mock}.html`, async ({ page, browser }) => {
					// The mockup.
					const mockUrl = `${pathToFileURL(`${MOCKUPS}/${screen.mock}.html`).href}${screen.hash ? `#${screen.hash}` : ''}`;
					await page.goto(mockUrl);
					await page.addStyleTag({ content: '.mock-legend{display:none !important}' });
					await page.waitForTimeout(100);
					const mockShot = await page.screenshot();
					const mock = await sample(page);

					// The app.
					await page.goto(screen.url);
					await expect(page.getByRole('heading', { level: 1 }).first()).toBeVisible();
					await page.waitForLoadState('networkidle');
					if (screen.open) await screen.open(page);
					await page.waitForTimeout(300);
					const appShot = await page.screenshot();
					const app = await sample(page);
					if (process.env.FM_WRITE_SHOTS) {
						mkdirSync(SHOTS, { recursive: true });
						writeFileSync(`${SHOTS}/${screen.id}-${theme.name}-${vp.width}.png`, appShot);
					}

					// Shell geometry: where the sidebar, titlebar, main column, strip and title block sit.
					const problems: string[] = [];
					for (const sel of GEOMETRY) {
						if (EXEMPT[`${screen.id}@${vp.width}`]?.includes(sel)) continue;
						const a = mock[sel];
						const b = app[sel];
						if (!a || !b) continue;
						if (a.rect.some((v, i) => Math.abs(v - b.rect[i]) > 2)) {
							problems.push(`${sel} sits at ${b.rect.join(',')}, the mockup has ${a.rect.join(',')}`);
						}
					}
					// The shell exists in the app wherever the mockup draws it (the strip only on a project).
					for (const sel of ['.sidebar', '.titlebar', 'main']) {
						if (mock[sel] && !app[sel]) problems.push(`${sel} is missing`);
					}
					// Paint: every element both draw is colored, set and rounded the same.
					for (const sel of STYLED) {
						const a = mock[sel];
						const b = app[sel];
						if (!a || !b) continue;
						for (const p of PROPS) {
							if (a.style[p] !== b.style[p]) {
								problems.push(`${sel} ${p} is ${b.style[p]}, the mockup has ${a.style[p]}`);
							}
						}
					}
					expect(problems).toEqual([]);

					// Whole page.
					const scratch = await browser.newPage();
					const score = await pixelDiff(scratch, mockShot, appShot);
					await scratch.close();
					if (process.env.FM_DIFF_TSV) {
						appendFileSync(
							process.env.FM_DIFF_TSV,
							`${screen.id}\t${theme.name}\t${vp.width}\t${score.toFixed(4)}\n`
						);
					}
					test.info().annotations.push({ type: 'pixel diff', description: score.toFixed(4) });
					const limit = SAME_DATA[screen.id];
					if (limit !== undefined && !EXEMPT[`${screen.id}@${vp.width}`]) {
						expect(score).toBeLessThan(limit);
					}
				});
			}
		});
	}
}
