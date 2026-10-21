// M3-11 (ADR-006, SR-29; AC-F009-06, AC-F009-08, AC-F009-18, AC-F008-20; T-094, T-095, T-096):
// the terminal host's options, key routing, answerback and re-arm archive, against real xterm 6 in
// the browser and the mock IPC.
import { Terminal } from '@xterm/xterm';
import typings from '@xterm/xterm/typings/xterm.d.ts?raw';
import { describe, expect, it, vi } from 'vitest';
import { userEvent } from 'vitest/browser';
import { render } from 'vitest-browser-svelte';

import { runStep } from '#lib/actions/run.ts';
import TerminalDrawer from '#lib/components/TerminalDrawer.svelte';
import TerminalView from '#lib/components/TerminalView.svelte';
import { requests } from '#lib/ipc/fixtures/requests.ts';
import { terminal } from '#lib/stores/terminal.svelte.ts';
import { IDS, fresh } from '#lib/test/screen.ts';

import packageJson from '../../../package.json';
import { archiveOf, findNext } from './archive.ts';
import { routeKey, xtermHandles } from './keys.ts';
import { findUrls, urlLinks } from './links.ts';
import { styleNonce, withStyleNonce } from './style-nonce.ts';
import {
	SCROLLBACK_DEFAULT,
	WINDOW_OPTIONS,
	scrollbackCap,
	terminalOptions,
	type TerminalPrefs
} from './options.ts';

const sources = import.meta.glob<string>(
	['/src/**/*.ts', '/src/**/*.svelte', '!/src/**/*.test.ts'],
	{
		query: '?raw',
		import: 'default',
		eager: true
	}
);

const prefs: TerminalPrefs = {
	fontFamily: 'SF Mono',
	fontSize: 12,
	scrollback: SCROLLBACK_DEFAULT,
	macOptionIsMeta: true,
	screenReaderMode: false,
	live: true
};

function windowOptionKeys(): string[] {
	const body = /export interface IWindowOptions \{([\s\S]*?)\n {2}\}/.exec(typings)?.[1] ?? '';
	return [...body.matchAll(/^\s+([a-zA-Z]+)\?: boolean;/gm)].map((m) => m[1] ?? '');
}

function mounted(options = terminalOptions(prefs)): { t: Terminal; host: HTMLDivElement } {
	const host = document.createElement('div');
	host.style.setProperty('width', '640px');
	host.style.setProperty('height', '240px');
	document.body.append(host);
	const t = new Terminal(options);
	t.open(host);
	return { t, host };
}

/** Escape and BEL made visible, so patterns don't hold control characters. */
const vis = (s: string) => s.replaceAll('\x1b', '⎋').replaceAll('\x07', '␇');

const write = (t: Terminal, s: string) => new Promise<void>((done) => t.write(s, done));

describe('terminal', () => {
	it('terminal::options_snapshot', () => {
		const o = terminalOptions(prefs);
		expect(o).toEqual({
			fontFamily: 'SF Mono',
			fontSize: 12,
			lineHeight: 1.25,
			scrollback: 10_000,
			screenReaderMode: false,
			macOptionIsMeta: true,
			disableStdin: false,
			cursorBlink: false,
			minimumContrastRatio: 4.5,
			allowProposedApi: false,
			ignoreBracketedPasteMode: false,
			logLevel: 'off',
			windowOptions: WINDOW_OPTIONS,
			linkHandler: null
		});
		// Every window option xterm 6 knows is present and false (SR-29, T-096).
		const keys = windowOptionKeys();
		expect(keys.length).toBeGreaterThanOrEqual(22);
		expect(Object.keys(WINDOW_OPTIONS).sort()).toEqual([...keys].sort());
		expect(Object.values(WINDOW_OPTIONS).every((v) => !v)).toBe(true);
		// A real xterm built with them keeps them.
		const { t, host } = mounted();
		expect(
			Object.values(t.options.windowOptions ?? { missing: true }).every((v) => v === false)
		).toBe(true);
		expect(t.options.scrollback).toBe(10_000);
		t.dispose();
		host.remove();
		// AC-F008-20: Screen reader mode turns xterm's screen-reader mode on; off, it stays off.
		expect(terminalOptions({ ...prefs, screenReaderMode: true }).screenReaderMode).toBe(true);
		// AC-F009-06: the scrollback cap is always finite and within Settings' bounds.
		expect(scrollbackCap(Number.POSITIVE_INFINITY)).toBe(10_000);
		expect(scrollbackCap(5)).toBe(1_000);
		expect(scrollbackCap(1e9)).toBe(100_000);
		// An exited run's buffer is read-only.
		expect(terminalOptions({ ...prefs, live: false }).disableStdin).toBe(true);
		// T-094: no clipboard addon anywhere; T-095: titles never subscribed; every xterm is built
		// from these options.
		const deps = { ...packageJson.dependencies, ...packageJson.devDependencies };
		expect(Object.keys(deps).filter((d) => d.startsWith('@xterm/'))).toEqual(['@xterm/xterm']);
		for (const [path, src] of Object.entries(sources)) {
			expect(src, path).not.toMatch(
				/from\s+['"]@xterm\/addon-clipboard['"]|import\(\s*['"]@xterm\/addon-clipboard/
			);
			expect(src, path).not.toMatch(/\.onTitleChange\s*\(/);
			if (
				src.includes("from '@xterm/xterm'") &&
				src.includes('new Terminal(') &&
				!path.endsWith('/terminal/archive.ts')
			)
				expect(src, path).toMatch(/new Terminal\(\s*terminalOptions\(/);
		}
	});

	it('terminal::no_answerback_injection', async () => {
		const { t, host } = mounted();
		const replies: string[] = [];
		t.onData((d) => replies.push(d));
		t.onBinary((d) => replies.push(d));
		const fixture = [
			'\x1b]2;PWN-title\x07', // set the window title (OSC 2) …
			'\x1b]0;PWN-both\x07',
			'\x1b]1;PWN-icon\x07',
			'\x1b[21t', // … then ask for it back (title report)
			'\x1b[20t', // icon title report
			'\x1b[22;0t\x1b[23;0t', // push and pop the title
			'\x1b[11t\x1b[13t\x1b[14t\x1b[15t\x1b[16t\x1b[18t\x1b[19t', // window state and size reports
			'\x1b]10;?\x07\x1b]11;?\x07\x1b]12;?\x07\x1b]4;1;?\x07', // color queries
			'\x1b]52;c;?\x07', // OSC 52 clipboard read
			'\x1b]52;c;UFdOLWNsaXA=\x07', // OSC 52 clipboard write ("PWN-clip")
			'\x1bP$qPWN\x1b\\', // DECRQSS for a setting that doesn't exist
			'\x1bP$q"p\x1b\\\x1bP$qm\x1b\\\x1bP$qr\x1b\\', // DECRQSS for real settings
			'\x1bP+qPWN\x1b\\', // XTGETTCAP
			'\x1b[c\x1b[>c\x1b[5n\x1b[6n\x1b[?6n', // device attributes and status
			'\x1b[?996n\x1b[?u\x1b[>q' // other queries
		].join('');
		await write(t, fixture);
		await new Promise((r) => setTimeout(r, 50));
		const all = vis(replies.join(''));
		expect(all).not.toMatch(/PWN|UFdO/);
		// What xterm may answer has a fixed shape: nothing in it comes from the output.
		const allowed = [
			/^⎋\[\?[\d;]*c$/, // DA1
			/^⎋\[>[\d;]*c$/, // DA2
			/^⎋\[0n$/, // DSR ok
			/^⎋\[\?*\d+;\d+R$/, // cursor position
			/^⎋P[01]\$r[\d;"pmr ]*⎋\\$/, // DECRQSS: a fixed setting or "invalid"
			/^⎋\]\d+(;\d+)?;rgb:[0-9a-f]{4}\/[0-9a-f]{4}\/[0-9a-f]{4}(␇|⎋\\)$/ // color reports
		];
		for (const r of replies.map(vis))
			expect(
				allowed.some((re) => re.test(r)),
				`unexpected reply ${JSON.stringify(r)}`
			).toBe(true);
		// No window report: titles, positions and sizes stay private (windowOptions all false).
		expect(all).not.toMatch(/⎋\][lL]|⎋\[\d+(;\d+)*t/);
		t.dispose();
		host.remove();
		// Control: with a window report switched on, the same kind of output gets an answer, so the
		// silence above is the options' doing.
		const control = mounted({
			...terminalOptions(prefs),
			windowOptions: { getWinSizeChars: true }
		});
		const echoed: string[] = [];
		control.t.onData((d) => echoed.push(d));
		await write(control.t, '\x1b[18t');
		expect(vis(echoed.join(''))).toMatch(/^⎋\[8;\d+;\d+t$/);
		control.t.dispose();
		control.host.remove();

		// Through the tab: output that asks questions never makes Foreman type attacker bytes.
		fresh('happy');
		const runId = 'a0000000-0000-4000-8000-0000000000c1';
		terminal.add({ runId, chain: 'c1', tabName: 'fm-x-s02-2-2' }, null);
		const sent = vi.spyOn(terminal, 'write').mockResolvedValue();
		await render(TerminalView, { runId, live: true, label: 'Terminal output' });
		await vi.waitFor(() => expect(document.querySelector('.xterm-screen')).not.toBeNull());
		terminal.output(runId)(new TextEncoder().encode(fixture).buffer);
		await new Promise((r) => setTimeout(r, 100));
		const typed = sent.mock.calls.map((c) => c[1]).join('');
		expect(typed).not.toMatch(/PWN|UFdO/);
	});

	it('terminal::key_routing', () => {
		const k = (
			key: string,
			mods: Partial<Record<'meta' | 'ctrl' | 'alt' | 'shift', boolean>> = {}
		) =>
			routeKey({
				type: 'keydown',
				key,
				code: key.length === 1 && /[a-z]/i.test(key) ? `Key${key.toUpperCase()}` : '',
				metaKey: !!mods.meta,
				ctrlKey: !!mods.ctrl,
				altKey: !!mods.alt,
				shiftKey: !!mods.shift
			});
		// ⌘C copies, ⌘V pastes through Rust, ⌘A and ⌘F stay in the terminal.
		expect(k('c', { meta: true })).toBe('copy');
		expect(k('v', { meta: true })).toBe('paste');
		expect(k('a', { meta: true })).toBe('selectAll');
		expect(k('f', { meta: true })).toBe('find');
		// Every other ⌘-chord and ⌃` reach the app (⌘K palette, ⌘⇧E back to the workspace, …).
		for (const [key, mods] of [
			['k', { meta: true }],
			['e', { meta: true, shift: true }],
			['t', { meta: true, shift: true }],
			[',', { meta: true }],
			['Enter', { meta: true }],
			['1', { meta: true }],
			['c', { meta: true, shift: true }],
			['v', { meta: true, shift: true }],
			['`', { ctrl: true }]
		] as const)
			expect(k(key, mods), `${JSON.stringify(mods)} ${key}`).toBe('app');
		// Everything else goes to Claude Code: letters, ⌃C, Escape, Tab, Shift+Tab, Option keys.
		for (const [key, mods] of [
			['x', {}],
			['c', { ctrl: true }],
			['Escape', {}],
			['Tab', {}],
			['Tab', { shift: true }],
			['f', { alt: true }],
			['ArrowUp', {}]
		] as const)
			expect(k(key, mods), `${JSON.stringify(mods)} ${key}`).toBe('pty');
		expect(xtermHandles('pty')).toBe(true);
		for (const r of ['app', 'copy', 'paste', 'selectAll', 'find'] as const)
			expect(xtermHandles(r)).toBe(false);
	});

	it('terminal::keys_in_the_view', async () => {
		const m = fresh('happy');
		m.streamDelayMs = 0;
		const runId = IDS.run23;
		terminal.add({ runId, chain: 'c1', tabName: 'fm-x-s02-2-3' }, null);
		const sent = vi.spyOn(terminal, 'write').mockResolvedValue();
		const reached: string[] = [];
		const onWindow = (e: KeyboardEvent) => {
			if (e.metaKey || e.ctrlKey) reached.push(`${e.metaKey ? '⌘' : '⌃'}${e.key}`);
		};
		window.addEventListener('keydown', onWindow);
		const s = await render(TerminalView, { runId, live: true, label: 'Terminal output' });
		await vi.waitFor(() => expect(document.querySelector('.xterm-helper-textarea')).not.toBeNull());
		terminal.requestFocus();
		await vi.waitFor(() =>
			expect(document.activeElement?.classList.contains('xterm-helper-textarea')).toBe(true)
		);
		await userEvent.keyboard('hi');
		await vi.waitFor(() => expect(sent.mock.calls.map((c) => c[1]).join('')).toBe('hi'));
		await userEvent.keyboard('{Meta>}k{/Meta}');
		await userEvent.keyboard('{Control>}`{/Control}');
		expect(reached).toContain('⌘k');
		expect(reached).toContain('⌃`');
		expect(sent.mock.calls.map((c) => c[1]).join('')).toBe('hi');
		// ⌘F finds in the buffer; Escape returns to the terminal.
		terminal.output(runId)(new TextEncoder().encode('alpha\r\nneedle here\r\n').buffer);
		await userEvent.keyboard('{Meta>}f{/Meta}');
		const find = s.getByRole('searchbox', { name: 'Find in terminal output' });
		await expect.element(find).toHaveFocus();
		await userEvent.keyboard('needle{Enter}');
		await expect.element(s.getByRole('status')).toHaveTextContent('Line 2');
		await userEvent.keyboard('{Escape}');
		await expect.element(find).not.toBeInTheDocument();
		// ⌘V pastes through Rust's sanitized text, bracketed by xterm only if Claude asked for it.
		m.setClipboard('pasted\x1b[201~text');
		document.querySelector<HTMLTextAreaElement>('.xterm-helper-textarea')?.focus();
		await userEvent.keyboard('{Meta>}v{/Meta}');
		await vi.waitFor(() =>
			expect(sent.mock.calls.map((c) => c[1]).join('')).toContain('pasted[201~text')
		);
		expect(sent.mock.calls.map((c) => c[1]).join('')).not.toContain('\x1b[201~text');
		window.removeEventListener('keydown', onWindow);
	});

	it('terminal::rearm_archives_buffer', async () => {
		// The archive is text read from the buffer, capped at the scrollback cap.
		const many = Array.from({ length: 1_200 }, (_, i) => `line ${i + 1}\r\n`).join('');
		const a = await archiveOf(
			'fm-x-s02-2-2',
			[new TextEncoder().encode(`\x1b[32mgreen\x1b[0m\r\n${many}`)],
			80,
			1_000
		);
		expect(a.lines).toBeLessThanOrEqual(1_000);
		expect(a.text.endsWith('line 1200')).toBe(true);
		expect(a.text).not.toContain('\x1b');
		expect(findNext(['one', 'two needle', 'needle'], 'NEEDLE', null)).toEqual({
			row: 1,
			col: 4,
			length: 6
		});
		expect(findNext(['one', 'two needle', 'needle'], 'needle', { row: 2, col: 0 })).toEqual({
			row: 1,
			col: 4,
			length: 6
		});

		// Re-arm through the mock: the old tab's buffer becomes the new tab's collapsed archive.
		const m = fresh('happy');
		m.streamDelayMs = 0;
		await m.configAck(requests.config_ack).catch(() => undefined);
		const live23 = m.world.runs.find((x) => x.summary.runId === IDS.run23);
		if (live23) live23.summary.status = 'idle';
		const prep = await m.launchPrepare(requests.launch_prepare);
		if (prep.kind !== 'ready' || prep.rearm?.kind !== 'idleConfirmation')
			throw new Error(`fixture: 2.4 re-arms its chain: ${JSON.stringify(prep)}`);
		const chain = prep.rearm.chain;
		const old = 'a0000000-0000-4000-8000-0000000000d4';
		terminal.add(
			{ runId: old, chain, tabName: 'fm-tradelog-s02-2-3' },
			requests.launch_prepare.projectId
		);
		terminal.output(old)(new TextEncoder().encode('<script>x</script> earlier output\r\n').buffer);
		terminal.active = old;
		m.launchPrepare = () => Promise.resolve(prep);
		const r = await runStep({
			projectId: requests.launch_prepare.projectId,
			step: '2.4',
			surface: 'cli',
			entry: 'stageView'
		});
		expect(r).toBe(true);
		expect(terminal.tabs.some((t) => t.runId === old)).toBe(false);
		const tab = terminal.tabs.find((t) => t.chain === chain);
		expect(tab?.archived?.label).toBe('fm-tradelog-s02-2-3');
		expect(tab?.archived?.text).toContain('<script>x</script> earlier output');
		const s = await render(TerminalDrawer);
		const summary = s.getByText(
			/Earlier in this session: fm-tradelog-s02-2-3, 1 line \(archived, read-only\)/
		);
		await expect.element(summary).toBeVisible();
		const archived = s
			.getByRole('region', { name: /Archived output/ })
			.or(s.getByLabelText(/Archived output/));
		await expect.element(archived).not.toBeVisible();
		await summary.click();
		await expect.element(archived).toHaveTextContent('<script>x</script> earlier output');
		expect(document.querySelector('.archive-text script')).toBeNull();
	});
	it('terminal::style_nonce_propagation', () => {
		// Without Tauri's carrier nothing changes.
		expect(styleNonce()).toBe('');
		expect(withStyleNonce(() => document.createElement('style').nonce)).toBe('');
		// With it, xterm's runtime <style> elements get the page's nonce while it opens, and only then.
		const carrier = document.createElement('style');
		carrier.id = 'fm-style-nonce';
		carrier.nonce = '8128';
		document.head.append(carrier);
		try {
			expect(styleNonce()).toBe('8128');
			const made = withStyleNonce(() => [
				document.createElement('style').nonce,
				document.createElement('div') instanceof HTMLDivElement
			]);
			expect(made).toEqual(['8128', true]);
			expect(document.createElement('style').nonce).toBe('');
			expect(Object.hasOwn(document, 'createElement')).toBe(false);
			const { t, host } = mounted();
			t.dispose();
			host.remove();
			const opened = document.createElement('div');
			document.body.append(opened);
			const term = new Terminal(terminalOptions(prefs));
			withStyleNonce(() => {
				term.open(opened);
			});
			const styles = [...opened.querySelectorAll('style')];
			expect(styles.length).toBeGreaterThan(0);
			expect(styles.every((x) => x.nonce === '8128')).toBe(true);
			term.dispose();
			opened.remove();
		} finally {
			carrier.remove();
		}
	});
	it('terminal::web_links_route_to_rust', async () => {
		// AC-F009-17: plain URLs in output become links whose click hands the text to Rust.
		expect(findUrls('see https://example.com/docs?q=1. and http://localhost:5173/x,')).toEqual([
			{ start: 4, text: 'https://example.com/docs?q=1' },
			{ start: 38, text: 'http://localhost:5173/x' }
		]);
		expect(findUrls('(https://a.example/b) "https://c.example"')).toEqual([
			{ start: 1, text: 'https://a.example/b' },
			{ start: 23, text: 'https://c.example' }
		]);
		expect(findUrls('javascript:alert(1) file:///etc/passwd ftp://x https://')).toEqual([]);
		const { t, host } = mounted();
		await write(t, 'open https://claude.ai/chat/abc now\r\n');
		const opened: string[] = [];
		const provider = urlLinks(t, (u) => opened.push(u));
		const links = await new Promise<Parameters<Parameters<typeof provider.provideLinks>[1]>[0]>(
			(done) => {
				provider.provideLinks(1, done);
			}
		);
		expect(links?.map((l) => [l.text, l.range.start.x, l.range.end.x])).toEqual([
			['https://claude.ai/chat/abc', 6, 31]
		]);
		links?.[0]?.activate(new MouseEvent('click'), links[0].text);
		expect(opened).toEqual(['https://claude.ai/chat/abc']);
		const none = await new Promise<unknown>((done) => {
			provider.provideLinks(2, done);
		});
		expect(none).toBeUndefined();
		t.dispose();
		host.remove();
	});
	it('terminal::tab_shows_exit_code', async () => {
		// AC-F009-02: when a run ends, its tab says exited and the code from the run record.
		const m = fresh('happy');
		m.streamDelayMs = 0;
		terminal.add({ runId: IDS.run23, chain: 'c23', tabName: 'fm-tradelog-s02-2-3' }, null);
		await m.ptyTerminate({ runId: IDS.run23 });
		await terminal.statusChanged(m, IDS.run23, 'exited');
		const tab = terminal.tabs.find((t) => t.runId === IDS.run23);
		expect(tab?.status).toBe('exited');
		expect(tab?.exitCode).toBe(143);
		const s = await render(TerminalDrawer);
		await expect
			.element(s.getByRole('tab', { name: /fm-tradelog-s02-2-3.*exited 143/ }))
			.toBeVisible();
	});
});
