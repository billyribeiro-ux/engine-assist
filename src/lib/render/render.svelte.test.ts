// M2-05 (AC-F030-04, AC-XC-32; T-007, T-077, T-088, T-089): repo, pasted and terminal text is
// rendered as text, never as HTML, and characters that hide or reorder text show as visible escapes.
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';

import type { Diff as DiffT, DocNode } from '#lib/ipc/types.ts';

import Diff from './Diff.svelte';
import Doc from './Doc.svelte';
import { visibleText } from './escape.ts';
import Text from './Text.svelte';

const cp = (...c: number[]) => String.fromCodePoint(...c);
const HOSTILE =
	'<script>window.__pwned=1</script><img src=x onerror="window.__pwned=2"><b>bold</b>';

describe('render', () => {
	it('render::escapes_html_in_packet', async () => {
		const nodes: DocNode[] = [
			{ kind: 'heading', level: 2, spans: [{ kind: 'plain', text: HOSTILE }] },
			{
				kind: 'paragraph',
				spans: [{ kind: 'strong', children: [{ kind: 'plain', text: HOSTILE }] }]
			},
			{ kind: 'codeBlock', info: 'html', text: HOSTILE },
			{
				kind: 'list',
				ordered: false,
				items: [[{ kind: 'paragraph', spans: [{ kind: 'code', text: HOSTILE }] }]]
			}
		];
		const s = await render(Doc, { nodes });
		const el = s.container;
		expect(el.querySelector('script, img, b, iframe')).toBeNull();
		expect(el.textContent).toContain(HOSTILE);
		expect((window as unknown as { __pwned?: number }).__pwned).toBeUndefined();
	});

	it('render::no_raw_html_passthrough', async () => {
		// A link span is text plus its URL as text: no anchor, no javascript: navigation.
		const s = await render(Text, {
			spans: [{ kind: 'link', text: 'click <i>me</i>', url: 'javascript:alert(1)' }]
		});
		expect(s.container.querySelector('a, i')).toBeNull();
		expect(s.container.textContent).toContain('javascript:alert(1)');
		// And no source file has an HTML path at all.
		const sources = import.meta.glob(['/src/**/*.svelte', '/src/**/*.ts', '!/src/**/*.test.ts'], {
			query: '?raw',
			import: 'default',
			eager: true
		});
		const offenders = Object.entries(sources)
			.filter(([, text]) =>
				/\{@html|\.innerHTML\b|\.outerHTML\b|insertAdjacentHTML/.test(
					text.replace(/\/\/.*$|\/\*[\s\S]*?\*\/|<!--[\s\S]*?-->/gm, '')
				)
			)
			.map(([p]) => p);
		expect(Object.keys(sources).length).toBeGreaterThan(40);
		expect(offenders).toEqual([]);
	});

	it('render::bidi_visible_in_diff', async () => {
		const sneaky = `access = "user${cp(0x202e)} ${cp(0x2066)}// admin${cp(0x2069)} ${cp(0x2066)}"`;
		const diff: DiffT = {
			path: 'src/auth.rs',
			exists: true,
			hunks: [
				{
					oldStart: 1,
					newStart: 1,
					lines: [{ op: 'add', spans: [{ kind: 'plain', text: sneaky }] }]
				}
			]
		};
		const s = await render(Diff, { diff, label: 'Diff of auth.rs' });
		const text = s.container.textContent;
		expect(text).toContain('<RLO U+202E>');
		expect(text).toContain('<LRI U+2066>');
		expect(text).toContain('<PDI U+2069>');
		for (const c of [0x202e, 0x2066, 0x2069]) expect(text.includes(cp(c))).toBe(false);
		await expect
			.element(s.getByRole('img', { name: 'hidden character <RLO U+202E>' }))
			.toBeVisible();
		// The + sign is a character, not just color.
		expect(s.container.querySelector('.add .sign')?.textContent).toBe('+');
	});

	it('render::default_ignorables_visible', async () => {
		const cases: [number, string][] = [
			[0x200b, '<ZWSP U+200B>'],
			[0x200d, '<ZWJ U+200D>'],
			[0xfeff, '<BOM U+FEFF>'],
			[0x00ad, '<SHY U+00AD>'],
			[0x2060, '<WJ U+2060>'],
			[0xfe0f, '<U+FE0F>'],
			[0xe0041, '<U+E0041>'],
			[0x061c, '<ALM U+061C>'],
			[0x1b, '<ESC U+001B>'],
			[0x9b, '<U+009B>']
		];
		const text = cases.map(([c]) => `a${cp(c)}b`).join(' ');
		const s = await render(Text, { text });
		const shown = s.container.textContent;
		for (const [c, label] of cases) {
			expect(shown).toContain(label);
			expect(shown.includes(cp(c))).toBe(false);
		}
		// TAB and LF stay layout in multi-line text, and are escaped in single-line contexts.
		expect(visibleText('a\tb\nc')).toBe('a\tb\nc');
		expect(visibleText('a\tb\nc', false)).toBe('a<TAB U+0009>b<LF U+000A>c');
	});

	it('render::hostile_filename_escaped', async () => {
		const name = `invoice${cp(0x202e)}fdp.exe\n<img src=x onerror=alert(1)>${cp(0x1b)}[31m.md`;
		const s = await render(Text, { text: name, layout: false });
		const shown = s.container.textContent;
		expect(s.container.querySelector('img')).toBeNull();
		expect(shown).toBe(
			'invoice<RLO U+202E>fdp.exe<LF U+000A><img src=x onerror=alert(1)><ESC U+001B>[31m.md'
		);
	});
});
