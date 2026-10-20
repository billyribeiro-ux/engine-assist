/// <reference types="node" />
// M5-01 (AC-XC-24, AC-XC-26 lint part): one token source and no hard-coded design values.
// - src/lib/styles/tokens.css is a byte copy of docs/design/tokens.css (the design is the source).
// - Outside tokens.css, styles use tokens: no color literals, no font families or sizes, no
//   durations, no radii, and no spacing that has a token (2/4/8/12/16/24/32/48 px).
// - Every text/background pair in docs/design/contrast-check.py meets its WCAG 2.2 AA minimum in
//   Night and Vellum, computed from the shipped tokens.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const root = join(import.meta.dirname, '../../..');
const tokensPath = join(root, 'src/lib/styles/tokens.css');

function files(dir: string, out: string[] = []): string[] {
	for (const n of readdirSync(dir)) {
		const p = join(dir, n);
		if (statSync(p).isDirectory()) files(p, out);
		else if (/\.(svelte|css)$/.test(n) && p !== tokensPath) out.push(p);
	}
	return out;
}

/** The CSS a file contributes: whole .css files, or the <style> blocks of a component. */
function css(path: string): string {
	const text = readFileSync(path, 'utf8');
	if (path.endsWith('.css')) return text;
	return [...text.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((m) => m[1] ?? '').join('\n');
}

const RULES: { name: string; re: RegExp }[] = [
	{ name: 'color literal', re: /#[0-9a-fA-F]{3,8}\b|\b(?:rgba?|hsla?|oklch|lab|lch)\(/ },
	{ name: 'named color', re: /:\s*(?:white|black|red|green|blue|gray|grey|orange)\s*[;}]/ },
	{ name: 'font family', re: /font-family:(?!\s*(?:var\(|inherit))/ },
	{ name: 'font size', re: /font-size:(?!\s*(?:var\(|inherit))/ },
	{ name: 'duration', re: /(?:transition|animation)(?:-duration)?:[^;]*\b\d+m?s\b/ },
	{ name: 'radius', re: /border-radius:(?!\s*(?:var\(|0\b|50%|inherit))/ },
	{
		name: 'spacing that has a token',
		re: /(?:^|[\s;{])(?:margin|padding|gap)(?:-[a-z]+)?:[^;]*?(?<![\d.-])(?:2|4|8|12|16|24|32|48)px\b/m
	}
];

describe('design tokens', () => {
	it('src/lib/styles/tokens.css is the design token file, byte for byte', () => {
		expect(readFileSync(tokensPath, 'utf8')).toBe(
			readFileSync(join(root, 'docs/design/tokens.css'), 'utf8')
		);
	});

	it('no hard-coded design values outside tokens.css (AC-XC-26 lint)', () => {
		const problems: string[] = [];
		for (const f of files(join(root, 'src'))) {
			const lines = css(f).split('\n');
			lines.forEach((line, i) => {
				for (const r of RULES)
					if (r.re.test(line))
						problems.push(`${relative(root, f)}:${i + 1} ${r.name}: ${line.trim()}`);
			});
		}
		expect(problems).toEqual([]);
	});

	it('the lint catches planted violations', () => {
		const planted = [
			'color: #ff0000;',
			'background: rgba(0, 0, 0, 0.5);',
			'font-size: 13px;',
			'font-family: Arial;',
			'transition: opacity 200ms;',
			'border-radius: 6px;',
			'padding: 8px 12px;'
		];
		for (const p of planted)
			expect(
				RULES.some((r) => r.re.test(p)),
				p
			).toBe(true);
		for (const ok of [
			'color: var(--ink);',
			'padding: 6px var(--space-3);',
			'border-radius: 50%;',
			'gap: 7px;'
		])
			expect(
				RULES.some((r) => r.re.test(ok)),
				ok
			).toBe(false);
	});
});

// ---------------------------------------------------------------- contrast (AC-XC-24)
type Theme = Record<string, string>;
function block(text: string, selector: RegExp): Theme {
	const out: Theme = {};
	const m = selector.exec(text);
	if (!m) return out;
	const start = text.indexOf('{', m.index) + 1;
	const end = text.indexOf('}', start);
	for (const d of text.slice(start, end).matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g))
		if (d[1] && d[2]) out[d[1]] = d[2].trim();
	return out;
}
function themes(): { night: Theme; vellum: Theme } {
	const text = readFileSync(tokensPath, 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
	const night = block(text, /:root,\s*\[data-theme="night"\]\s*\{/);
	const vellum = { ...night, ...block(text, /\[data-theme="vellum"\]\s*\{/) };
	return { night, vellum };
}
function resolve(t: Theme, v: string, depth = 0): string {
	const m = /^var\((--[\w-]+)\)$/.exec(v.trim());
	if (!m?.[1] || depth > 5) return v.trim();
	return resolve(t, t[m[1]] ?? '', depth + 1);
}
function lum(hex: string): number {
	const h = hex.replace('#', '');
	const c = [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
	const [r, g, b] = c.map((x) => (x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4)) as [
		number,
		number,
		number
	];
	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function ratio(a: string, b: string): number {
	const [l1, l2] = [lum(a), lum(b)].sort((x, y) => y - x) as [number, number];
	return (l1 + 0.05) / (l2 + 0.05);
}

describe('contrast (WCAG 2.2 AA, both themes)', () => {
	const py = readFileSync(join(root, 'docs/design/contrast-check.py'), 'utf8');
	const MIN: Record<string, number> = { TEXT: 4.5, LARGE: 3, UI: 3 };
	const pairs = [...py.matchAll(/\("(--[\w-]+)",\s*"(--[\w-]+)",\s*(TEXT|LARGE|UI)/g)].map((m) => ({
		fg: m[1] ?? '',
		bg: m[2] ?? '',
		min: MIN[m[3] ?? 'TEXT'] ?? 4.5
	}));

	it('reads the pair list', () => {
		expect(pairs.length).toBeGreaterThan(30);
	});

	it('every listed pair passes in Night and Vellum', () => {
		const t = themes();
		const fails: string[] = [];
		let checked = 0;
		for (const [name, theme] of Object.entries(t))
			for (const p of pairs) {
				const fg = resolve(theme, theme[p.fg] ?? '');
				const bg = resolve(theme, theme[p.bg] ?? '');
				if (!/^#[0-9a-fA-F]{6}$/.test(fg) || !/^#[0-9a-fA-F]{6}$/.test(bg)) continue;
				checked++;
				const r = ratio(fg, bg);
				if (r < p.min) fails.push(`${name} ${p.fg} on ${p.bg}: ${r.toFixed(2)} < ${p.min}`);
			}
		expect(fails).toEqual([]);
		expect(checked).toBeGreaterThan(pairs.length);
	});

	it('Increase Contrast: every text pair reaches 7:1 in Night and Vellum (AC-XC-23)', () => {
		const text = readFileSync(tokensPath, 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
		const { night, vellum } = themes();
		const hcNight = block(text, /@media \(prefers-contrast: more\) \{\s*:root:root \{/);
		const hcVellum = block(text, /:root:root\[data-theme="vellum"\] \{/);
		const hcLight = block(
			text,
			/@media \(prefers-contrast: more\) and \(prefers-color-scheme: light\) \{\s*:root:root:not\(\[data-theme="night"\]\) \{/
		);
		expect(Object.keys(hcNight).length).toBeGreaterThan(5);
		expect(hcLight).toEqual(hcVellum);
		const sets = {
			Night: { ...night, ...hcNight },
			Vellum: { ...vellum, ...hcNight, ...hcVellum }
		};
		const fails: string[] = [];
		for (const [name, theme] of Object.entries(sets))
			for (const p of pairs.filter((x) => x.min === 4.5)) {
				const fg = resolve(theme, theme[p.fg] ?? '');
				const bg = resolve(theme, theme[p.bg] ?? '');
				if (!/^#[0-9a-fA-F]{6}$/.test(fg) || !/^#[0-9a-fA-F]{6}$/.test(bg)) continue;
				const r = ratio(fg, bg);
				if (r < 7) fails.push(`${name} ${p.fg} on ${p.bg}: ${r.toFixed(2)} < 7`);
			}
		expect(fails).toEqual([]);
	});
});
