<script lang="ts">
	// One run's terminal (F-009): xterm 6 fed with raw PTY bytes. With the terminal focused every key
	// goes to Claude Code except ⌘-chords and ⌃` (components.md); ⌘V pastes through Rust (pty_paste:
	// clipboard reads happen only in Rust, AC-XC-29) and links open only through terminal_link_open,
	// which asks in a native dialog (SR-29). Colors come from the --term-* tokens.
	import '@xterm/xterm/css/xterm.css';

	import { Terminal } from '@xterm/xterm';
	import { onMount } from 'svelte';

	import { failureText } from '#lib/actions/words.ts';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import { terminal } from '#lib/stores/terminal.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';

	let {
		runId,
		live,
		label,
		fontSize = 12,
		scrollback = 10000,
		screenReaderMode = false
	}: {
		runId: string;
		live: boolean;
		label: string;
		fontSize?: number;
		scrollback?: number;
		screenReaderMode?: boolean;
	} = $props();

	let host: HTMLDivElement;
	let term: Terminal | undefined;

	function css(name: string): string {
		return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
	}
	function theme() {
		const ink = css('--term-ink');
		return {
			background: css('--term-bg'),
			foreground: ink,
			cursor: ink,
			cursorAccent: css('--term-bg'),
			selectionBackground: css('--selected'),
			black: css('--term-muted'),
			red: css('--fail'),
			green: css('--pass'),
			yellow: css('--attention'),
			blue: css('--accent-ink'),
			magenta: css('--phase-finish-ink'),
			cyan: css('--phase-systems-ink'),
			white: ink,
			brightBlack: css('--term-muted'),
			brightRed: css('--fail'),
			brightGreen: css('--pass'),
			brightYellow: css('--attention'),
			brightBlue: css('--accent-ink'),
			brightMagenta: css('--phase-finish-ink'),
			brightCyan: css('--phase-systems-ink'),
			brightWhite: ink
		};
	}

	function fit(): void {
		if (!term || !host.clientWidth) return;
		// Cell size from xterm's own screen element (its size is cols × cell width, rows × cell height).
		const screen = host.querySelector<HTMLElement>('.xterm-screen');
		if (!screen) return;
		const box = screen.getBoundingClientRect();
		const cw = box.width / term.cols;
		const ch = box.height / term.rows;
		if (!cw || !ch) return;
		const cols = Math.max(2, Math.min(1000, Math.floor((host.clientWidth - 16) / cw)));
		const rows = Math.max(2, Math.min(500, Math.floor(host.clientHeight / ch)));
		if (cols !== term.cols || rows !== term.rows) {
			term.resize(cols, rows);
			if (live) void ipc().then((c) => c.ptyResize({ runId, cols, rows }).catch(() => undefined));
		}
	}

	async function paste(): Promise<void> {
		const client = await ipc();
		try {
			const r = await client.ptyPaste({ runId });
			term?.paste(r.text);
			if (r.removedControls) ui.announce(`Pasted; removed ${r.removedControls} control characters`);
		} catch (e: unknown) {
			ui.say(failureText(toIpcFailure(e)));
		}
	}

	onMount(() => {
		const t = new Terminal({
			fontFamily: css('--font-mono'),
			fontSize,
			lineHeight: 1.25,
			scrollback,
			screenReaderMode,
			macOptionIsMeta: true,
			disableStdin: !live,
			cursorBlink: false,
			// WCAG 2.2 AA for terminal text too, dim (SGR 2) included: xterm adjusts the foreground.
			minimumContrastRatio: 4.5,
			allowProposedApi: false,
			theme: theme(),
			linkHandler: {
				activate: (_e, text) => {
					void ipc().then(async (c) => {
						const r = await c.terminalLinkOpen({ runId, url: text });
						if (r === 'refused') ui.say('Foreman opens only https links.');
					});
				}
			}
		});
		term = t;
		t.attachCustomKeyEventHandler((ev) => {
			if (ev.type !== 'keydown') return !ev.metaKey;
			if (ev.metaKey && ev.key.toLowerCase() === 'v') {
				void paste();
				return false;
			}
			if (ev.metaKey) return false;
			if (ev.ctrlKey && (ev.key === '`' || ev.code === 'Backquote')) return false;
			return true;
		});
		t.open(host);
		fit();
		const data = t.onData((d) => {
			if (live)
				void terminal.write(runId, d).catch((e: unknown) => ui.say(failureText(toIpcFailure(e))));
		});
		const off = terminal.connect(runId, (bytes) => {
			void terminal.track(runId, bytes.length);
			t.write(bytes, () => void terminal.track(runId, -bytes.length));
		});
		const ro = new ResizeObserver(() => {
			fit();
		});
		ro.observe(host);
		const block = (e: ClipboardEvent) => {
			e.preventDefault();
		};
		host.addEventListener('paste', block, true);
		const media = matchMedia('(prefers-color-scheme: dark)');
		const retheme = () => {
			t.options.theme = theme();
		};
		media.addEventListener('change', retheme);
		const mo = new MutationObserver(retheme);
		mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
		return () => {
			off();
			data.dispose();
			ro.disconnect();
			mo.disconnect();
			media.removeEventListener('change', retheme);
			host.removeEventListener('paste', block, true);
			t.dispose();
		};
	});

	$effect(() => {
		if (term) term.options.disableStdin = !live;
	});
	let seenTick = terminal.focusTick;
	$effect(() => {
		if (terminal.focusTick !== seenTick) {
			seenTick = terminal.focusTick;
			term?.focus();
		}
	});
</script>

<div class="term-host" data-terminal bind:this={host} aria-label={label} role="group"></div>

<style>
	.term-host {
		height: 100%;
		min-height: 0;
		min-width: 0;
		overflow: hidden;
		padding: 0 var(--space-3);
	}
	/* SGR 2 (dim) text: xterm's DOM renderer draws it at 50% alpha, which fails AA on --term-bg.
	   Dim maps to the muted terminal ink instead, which passes in both themes (AC-XC-24). */
	.term-host :global(.xterm .xterm-rows .xterm-dim) {
		color: var(--term-muted);
	}
	.term-host :global(.xterm) {
		height: 100%;
	}
</style>
