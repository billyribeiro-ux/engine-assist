<script lang="ts">
	// One run's terminal (F-009, ADR-006): xterm 6 fed with raw PTY bytes. Options come from
	// `#lib/terminal/options.ts` (window options off, no clipboard addon, no title subscription,
	// SR-29) and keys are routed by `#lib/terminal/keys.ts` (AC-F009-08): with the terminal focused
	// every key goes to Claude Code except ⌘-chords and ⌃`; ⌘C copies the selection, ⌘V pastes
	// through Rust (pty_paste: clipboard reads happen only in Rust, AC-XC-29), ⌘A selects the
	// buffer, ⌘F finds in it, ⌘⇧E returns to the workspace. Links open only through
	// terminal_link_open, which asks in a native dialog (SR-29). Colors come from the --term-* tokens.
	import '@xterm/xterm/css/xterm.css';

	import { Terminal } from '@xterm/xterm';
	import { onMount, tick } from 'svelte';

	import { failureText } from '#lib/actions/words.ts';
	import { toIpcFailure } from '#lib/ipc/errors.ts';
	import { ipc } from '#lib/ipc/index.ts';
	import { terminal } from '#lib/stores/terminal.svelte.ts';
	import { ui } from '#lib/stores/ui.svelte.ts';
	import { bufferText, findNext, type Match } from '#lib/terminal/archive.ts';
	import { routeKey, xtermHandles } from '#lib/terminal/keys.ts';
	import { urlLinks } from '#lib/terminal/links.ts';
	import { SCROLLBACK_DEFAULT, terminalOptions } from '#lib/terminal/options.ts';
	import { withStyleNonce } from '#lib/terminal/style-nonce.ts';

	let {
		runId,
		live,
		label,
		fontSize = 12,
		scrollback = SCROLLBACK_DEFAULT,
		macOptionIsMeta = true,
		screenReaderMode = false
	}: {
		runId: string;
		live: boolean;
		label: string;
		fontSize?: number;
		scrollback?: number;
		macOptionIsMeta?: boolean;
		screenReaderMode?: boolean;
	} = $props();

	let host: HTMLDivElement;
	let term: Terminal | undefined;
	let finding = $state(false);
	let query = $state('');
	let found = $state<Match | null>(null);
	let findStatus = $state('');
	let findInput = $state<HTMLInputElement>();

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

	// C-1 (SR-28): ⌘V, Edit → Paste and the context menu's Paste can arrive as two signals for one
	// gesture (the key and the DOM paste event); one Rust paste answers both.
	const PASTE_ONCE_MS = 300;
	let lastPaste = -Infinity;
	function requestPaste(): void {
		const now = performance.now();
		if (now - lastPaste < PASTE_ONCE_MS) return;
		lastPaste = now;
		void paste();
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

	/** Every terminal link goes to Rust: it opens only allowed URLs, after a native dialog. */
	function openLink(url: string): void {
		void ipc().then(async (c) => {
			const r = await c.terminalLinkOpen({ runId, url });
			if (r === 'refused')
				ui.say('Foreman opens only https links (or http to localhost) without a user name.');
		});
	}

	/** ⌘C: the selection only; with nothing selected nothing happens (no interrupt). */
	function copy(): void {
		const text = term?.getSelection() ?? '';
		if (!text) return;
		void navigator.clipboard.writeText(text).then(
			() => ui.announce('Copied the selection'),
			() => ui.say("The selection couldn't be copied.")
		);
	}

	async function openFind(): Promise<void> {
		finding = true;
		findStatus = '';
		await tick();
		findInput?.focus();
		findInput?.select();
	}

	function closeFind(): void {
		finding = false;
		found = null;
		findStatus = '';
		term?.clearSelection();
		term?.focus();
	}

	function findAgain(): void {
		if (!term) return;
		const m = findNext(bufferText(term.buffer.active), query, found);
		found = m;
		if (!m) {
			term.clearSelection();
			findStatus = query ? 'No match' : '';
			return;
		}
		findStatus = `Line ${(m.row + 1).toLocaleString('en-US')}`;
		term.select(m.col, m.row, m.length);
		term.scrollToLine(Math.max(0, m.row - Math.floor(term.rows / 2)));
	}

	function onFindKey(e: KeyboardEvent): void {
		if (e.key === 'Enter') {
			e.preventDefault();
			findAgain();
		} else if (e.key === 'Escape') {
			e.preventDefault();
			closeFind();
		}
	}

	onMount(() => {
		const t = new Terminal(
			terminalOptions({
				fontFamily: css('--font-mono'),
				fontSize,
				scrollback,
				macOptionIsMeta,
				screenReaderMode,
				live,
				theme: theme(),
				// OSC 8 links (SR-29): the URL goes to Rust, which checks it and asks first.
				linkHandler: {
					activate: (_e, text) => {
						openLink(text);
					}
				}
			})
		);
		term = t;
		// Plain URLs in the output take the same path (AC-F009-17).
		const links = t.registerLinkProvider(urlLinks(t, openLink));
		t.attachCustomKeyEventHandler((ev) => {
			const route = routeKey(ev);
			if (ev.type === 'keydown') {
				if (route === 'paste') requestPaste();
				else if (route === 'copy') copy();
				else if (route === 'selectAll') t.selectAll();
				else if (route === 'find') void openFind();
				if (route !== 'pty' && route !== 'app') ev.preventDefault();
			}
			return xtermHandles(route);
		});
		// xterm's runtime style elements carry the page's style nonce (style-src 'self', ADR-014).
		withStyleNonce(() => {
			t.open(host);
		});
		fit();
		const data = t.onData((d) => {
			if (live)
				void terminal.write(runId, d).catch((e: unknown) => ui.say(failureText(toIpcFailure(e))));
		});
		// The write callback fires after the tab may be gone, so it keeps the id it was made for.
		const id = runId;
		const off = terminal.connect(id, (bytes) => {
			void terminal.track(id, bytes.length);
			t.write(bytes, () => void terminal.track(id, -bytes.length));
		});
		terminal.setCols(id, t.cols);
		const resized = t.onResize((s) => {
			terminal.setCols(id, s.cols);
		});
		const ro = new ResizeObserver(() => {
			fit();
		});
		ro.observe(host);
		// xterm's own paste handling is off (SR-28, C-1): its listeners read the event's text and
		// send it unsanitized, so every paste, drop and insertFrom* input is stopped here, in the
		// capture phase, before xterm sees it. A paste (Edit → Paste, the context menu) becomes a
		// Rust paste (pty_paste reads and sanitizes the clipboard); dropped text is refused.
		const stop = (e: Event) => {
			e.preventDefault();
			e.stopImmediatePropagation();
		};
		const onPaste = (e: ClipboardEvent) => {
			stop(e);
			requestPaste();
		};
		const onDrag = (e: DragEvent) => {
			stop(e);
			if (e.dataTransfer) e.dataTransfer.dropEffect = 'none';
		};
		const onDrop = (e: DragEvent) => {
			stop(e);
			ui.say("Foreman doesn't take dropped text in the terminal. Copy it and press ⌘V.");
		};
		const onBeforeInput = (e: InputEvent) => {
			if (!e.inputType.startsWith('insertFrom')) return;
			stop(e);
			if (e.inputType.startsWith('insertFromPaste')) requestPaste();
		};
		const onInput = (e: Event) => {
			if (!(e instanceof InputEvent) || !e.inputType.startsWith('insertFrom')) return;
			e.stopImmediatePropagation();
			if (e.target instanceof HTMLTextAreaElement) e.target.value = '';
		};
		const guards: [string, (e: never) => void][] = [
			['paste', onPaste],
			['dragover', onDrag],
			['drop', onDrop],
			['beforeinput', onBeforeInput],
			['input', onInput]
		];
		for (const [type, fn] of guards) host.addEventListener(type, fn as EventListener, true);
		const media = matchMedia('(prefers-color-scheme: dark)');
		const retheme = () => {
			t.options.theme = theme();
		};
		media.addEventListener('change', retheme);
		const mo = new MutationObserver(retheme);
		mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
		return () => {
			off();
			links.dispose();
			data.dispose();
			resized.dispose();
			ro.disconnect();
			mo.disconnect();
			media.removeEventListener('change', retheme);
			for (const [type, fn] of guards) host.removeEventListener(type, fn as EventListener, true);
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

<div class="term-wrap">
	{#if finding}
		<div class="find" role="search" aria-label="Find in terminal">
			<input
				bind:this={findInput}
				class="input mono"
				type="search"
				aria-label="Find in terminal output"
				placeholder="Find"
				bind:value={query}
				oninput={() => {
					found = null;
					findStatus = '';
				}}
				onkeydown={onFindKey}
			/>
			<button class="btn ghost" type="button" onclick={findAgain}>Next</button>
			<button class="btn ghost" type="button" onclick={closeFind}>Done</button>
			<span class="find-status" role="status">{findStatus}</span>
		</div>
	{/if}
	<div class="term-host" data-terminal bind:this={host} aria-label={label} role="group"></div>
</div>

<style>
	.term-wrap {
		position: relative;
		height: 100%;
		min-height: 0;
		min-width: 0;
	}
	.term-host {
		height: 100%;
		min-height: 0;
		min-width: 0;
		overflow: hidden;
		padding: 0 var(--space-3);
	}
	.find {
		position: absolute;
		top: var(--space-2);
		right: var(--space-4);
		z-index: 5;
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2);
		background: var(--sheet-raised);
		color: var(--ink);
		border-radius: var(--radius-control);
		box-shadow: var(--shadow-overlay);
	}
	.find .input {
		width: 220px;
	}
	.find-status {
		font-size: var(--text-12);
		color: var(--ink-muted);
		min-width: 64px;
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
